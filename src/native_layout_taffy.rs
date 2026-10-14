//! Feature-gated Taffy geometry adapter.
//!
//! Altru Browser owns the layout contract. Taffy receives only normalized
//! geometry inputs after DOM, CSS parsing/cascade and formatting-context
//! selection have already been decided by the native engine.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryMode {
    FlexRow,
    FlexColumn,
    FlexRowReverse,
    FlexColumnReverse,
    Grid { columns: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryChild {
    pub min_height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryRequest {
    pub mode: GeometryMode,
    pub width: f32,
    pub gap: f32,
    pub children: Vec<GeometryChild>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryResult {
    pub width: f32,
    pub height: f32,
    pub children: Vec<GeometryBox>,
}

#[cfg(feature = "taffy-layout")]
mod enabled {
    use super::*;
    use taffy::prelude::*;

    pub fn solve_geometry(request: &GeometryRequest) -> Result<GeometryResult, String> {
        if !request.width.is_finite()
            || request.width <= 0.0
            || !request.gap.is_finite()
            || request.gap < 0.0
        {
            return Err("invalid owned geometry request".into());
        }
        if request.children.is_empty() {
            return Ok(GeometryResult {
                width: request.width,
                height: 0.0,
                children: Vec::new(),
            });
        }

        let mut tree: TaffyTree<()> = TaffyTree::new();
        let mut children = Vec::with_capacity(request.children.len());
        for child in &request.children {
            let node = tree
                .new_leaf(Style {
                    min_size: Size {
                        width: LengthPercentageAuto::length(0.0),
                        height: LengthPercentageAuto::length(child.min_height.max(1.0)),
                    },
                    flex_grow: 1.0,
                    ..Default::default()
                })
                .map_err(|error| error.to_string())?;
            children.push(node);
        }

        let mut style = Style {
            size: Size {
                width: Dimension::from_length(request.width),
                height: Dimension::AUTO,
            },
            gap: Size {
                width: LengthPercentage::length(request.gap),
                height: LengthPercentage::length(request.gap),
            },
            ..Default::default()
        };

        match request.mode {
            GeometryMode::FlexRow => {
                style.display = taffy::style::Display::Flex;
                style.flex_direction = taffy::style::FlexDirection::Row;
            }
            GeometryMode::FlexColumn => {
                style.display = taffy::style::Display::Flex;
                style.flex_direction = taffy::style::FlexDirection::Column;
            }
            GeometryMode::FlexRowReverse => {
                style.display = taffy::style::Display::Flex;
                style.flex_direction = taffy::style::FlexDirection::RowReverse;
            }
            GeometryMode::FlexColumnReverse => {
                style.display = taffy::style::Display::Flex;
                style.flex_direction = taffy::style::FlexDirection::ColumnReverse;
            }
            GeometryMode::Grid { columns } => {
                if columns == 0 || columns > 12 {
                    return Err("owned grid column count outside bounded range".into());
                }
                style.display = taffy::style::Display::Grid;
                style.grid_template_columns = (0..columns).map(|_| fr(1.0)).collect();
            }
        }

        let root = tree
            .new_with_children(style, &children)
            .map_err(|error| error.to_string())?;
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(request.width),
                height: AvailableSpace::MaxContent,
            },
        )
        .map_err(|error| error.to_string())?;

        let root_layout = tree.layout(root).map_err(|error| error.to_string())?;
        let mut boxes = Vec::with_capacity(children.len());
        for child in children {
            let layout = tree.layout(child).map_err(|error| error.to_string())?;
            boxes.push(GeometryBox {
                x: layout.location.x,
                y: layout.location.y,
                width: layout.size.width,
                height: layout.size.height,
            });
        }
        Ok(GeometryResult {
            width: root_layout.size.width,
            height: root_layout.size.height,
            children: boxes,
        })
    }

    /// Isolated H3 adapter proof. Only a measured single-row candidate may
    /// enter Taffy. No live DOM/style wiring or capability promotion.
    pub fn solve_candidate_mixed_grid(
        tracks: &crate::capability_ir::AcirGridTrackList,
        measured: &[crate::intrinsic_measurement::IntrinsicMeasurement],
        width: f32,
        gap: f32,
        child_height: f32,
    ) -> Result<GeometryResult, String> {
        if !child_height.is_finite() || child_height <= 0.0 {
            return Err("invalid candidate child height".into());
        }
        let candidate =
            crate::intrinsic_grid::candidate_single_row_tracks(tracks, measured, width, gap)
                .map_err(str::to_owned)?;
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let mut children = Vec::with_capacity(candidate.widths_px.len());
        for &track_width in &candidate.widths_px {
            let node = tree
                .new_leaf(Style {
                    size: Size {
                        width: Dimension::from_length(track_width),
                        height: Dimension::from_length(child_height),
                    },
                    min_size: Size {
                        width: LengthPercentageAuto::length(track_width),
                        height: LengthPercentageAuto::length(child_height),
                    },
                    max_size: Size {
                        width: LengthPercentageAuto::length(track_width),
                        height: LengthPercentageAuto::length(child_height),
                    },
                    ..Default::default()
                })
                .map_err(|error| error.to_string())?;
            children.push(node);
        }
        let style = Style {
            display: taffy::style::Display::Grid,
            size: Size {
                width: Dimension::from_length(width),
                height: Dimension::AUTO,
            },
            grid_template_columns: candidate.widths_px.iter().copied().map(length).collect(),
            gap: Size {
                width: LengthPercentage::length(gap),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };
        let root = tree
            .new_with_children(style, &children)
            .map_err(|error| error.to_string())?;
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::MaxContent,
            },
        )
        .map_err(|error| error.to_string())?;
        let actual = tree.layout(root).map_err(|error| error.to_string())?;
        let mut boxes = Vec::with_capacity(children.len());
        for (index, node) in children.into_iter().enumerate() {
            let layout = tree.layout(node).map_err(|error| error.to_string())?;
            let expected = candidate.offsets_px[index];
            // Taffy snaps independently calculated CSS-pixel track edges to
            // physical pixel boundaries. Compare those same snapped edges;
            // do not relax the tolerance for genuinely incorrect geometry.
            let snapped_start = expected.round();
            let snapped_end = (expected + candidate.widths_px[index]).round();
            if (layout.location.x - snapped_start).abs() > 0.05
                || (layout.size.width - (snapped_end - snapped_start)).abs() > 0.05
            {
                return Err(format!("candidate/Taffy geometry disagrees: index={index}, expected_x={expected}, actual_x={}, expected_w={}, actual_w={}", layout.location.x, candidate.widths_px[index], layout.size.width));
            }
            boxes.push(GeometryBox {
                x: layout.location.x,
                y: layout.location.y,
                width: layout.size.width,
                height: layout.size.height,
            });
        }
        Ok(GeometryResult {
            width: actual.size.width,
            height: actual.size.height,
            children: boxes,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reverse_flex_changes_visual_order_without_reordering_children() {
            for (normal, reverse, horizontal) in [
                (GeometryMode::FlexRow, GeometryMode::FlexRowReverse, true),
                (GeometryMode::FlexColumn, GeometryMode::FlexColumnReverse, false),
            ] {
                let mk = |mode| GeometryRequest {
                    mode, width: 300.0, gap: 10.0,
                    children: vec![
                        GeometryChild { min_height: 18.0 },
                        GeometryChild { min_height: 35.0 },
                        GeometryChild { min_height: 24.0 },
                    ],
                };
                let a = solve_geometry(&mk(normal)).unwrap();
                let b = solve_geometry(&mk(reverse)).unwrap();
                assert_eq!(a.children.len(), 3);
                assert_eq!(b.children.len(), 3);
                let pos = |r: &GeometryResult, i: usize| if horizontal { r.children[i].x } else { r.children[i].y };
                assert!(pos(&a, 0) < pos(&a, 2));
                assert!(pos(&b, 0) > pos(&b, 2));
            }
        }

        #[test]
        fn candidate_track_order_survives_taffy() {
            use crate::capability_ir::{AcirGridTrack, AcirGridTrackList};
            use crate::intrinsic_measurement::IntrinsicMeasurement;
            let measured = [
                IntrinsicMeasurement::measured(20.0, 40.0),
                IntrinsicMeasurement::measured(30.0, 60.0),
            ];
            let a = AcirGridTrackList {
                columns: vec![AcirGridTrack::FractionMilli(1000), AcirGridTrack::Auto],
            };
            let b = AcirGridTrackList {
                columns: vec![AcirGridTrack::Auto, AcirGridTrack::FractionMilli(1000)],
            };
            let result_a = solve_candidate_mixed_grid(&a, &measured, 300.0, 10.0, 20.0).unwrap();
            let result_b = solve_candidate_mixed_grid(&b, &measured, 300.0, 10.0, 20.0).unwrap();
            assert_eq!(result_a.children[0].width, 230.0);
            assert_eq!(result_a.children[1].x, 240.0);
            assert_eq!(result_b.children[0].width, 40.0);
            assert_eq!(result_b.children[1].x, 50.0);
        }
        #[test]
        fn h4_emit_resolved_font_track_geometry() {
            use crate::capability_ir::{AcirGridTrack, AcirGridTrackList};
            let Ok(path) = std::env::var("ALTRU_TEST_FONT") else { return };
            let font = std::fs::read(path).unwrap();
            let measured = ["aaa", "bbbbbbbb"].map(|value|
                crate::intrinsic_text::measure_ascii_text(&font, 16.0, value));
            for (name, columns) in [
                ("fr-auto", vec![AcirGridTrack::FractionMilli(1000), AcirGridTrack::Auto]),
                ("auto-fr", vec![AcirGridTrack::Auto, AcirGridTrack::FractionMilli(1000)]),
                ("fr-fr", vec![AcirGridTrack::FractionMilli(1000), AcirGridTrack::FractionMilli(1000)]),
            ] {
                let result = solve_candidate_mixed_grid(&AcirGridTrackList { columns }, &measured, 300.0, 10.0, 24.0).unwrap();
                println!("H4_NATIVE {name} {} {} {} {}", result.children[0].x, result.children[0].width, result.children[1].x, result.children[1].width);
            }
        }
        #[test]
        fn candidate_unknown_measurement_fails_closed() {
            use crate::capability_ir::{AcirGridTrack, AcirGridTrackList};
            use crate::intrinsic_measurement::{IntrinsicMeasurement, UnknownMeasurement};
            let a = AcirGridTrackList {
                columns: vec![AcirGridTrack::FractionMilli(1000), AcirGridTrack::Auto],
            };
            let measured = [
                IntrinsicMeasurement::measured(20.0, 40.0),
                IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable),
            ];
            assert!(solve_candidate_mixed_grid(&a, &measured, 300.0, 10.0, 20.0).is_err());
        }

        #[test]
        fn owned_flex_contract_produces_geometry() {
            let result = solve_geometry(&GeometryRequest {
                mode: GeometryMode::FlexRow,
                width: 300.0,
                gap: 10.0,
                children: vec![
                    GeometryChild { min_height: 40.0 },
                    GeometryChild { min_height: 40.0 },
                ],
            })
            .unwrap();
            assert_eq!(result.width, 300.0);
            assert_eq!(result.children.len(), 2);
            assert!(result.children[1].x > result.children[0].x);
            assert!(result.height >= 40.0);
        }

        #[test]
        fn owned_grid_contract_produces_two_columns() {
            let result = solve_geometry(&GeometryRequest {
                mode: GeometryMode::Grid { columns: 2 },
                width: 300.0,
                gap: 10.0,
                children: vec![
                    GeometryChild { min_height: 30.0 },
                    GeometryChild { min_height: 30.0 },
                    GeometryChild { min_height: 30.0 },
                ],
            })
            .unwrap();
            assert_eq!(result.children.len(), 3);
            assert!(result.children[1].x > result.children[0].x);
            assert!(result.children[2].y > result.children[0].y);
        }
    }
}

#[cfg(feature = "taffy-layout")]
pub use enabled::{solve_geometry, solve_candidate_mixed_grid};

#[cfg(not(feature = "taffy-layout"))]
pub fn solve_geometry(_: &GeometryRequest) -> Result<GeometryResult, String> {
    Err("flex/grid geometry adapter is not enabled".into())
}

#[cfg(not(feature = "taffy-layout"))]
pub fn solve_candidate_mixed_grid(_: &crate::capability_ir::AcirGridTrackList, _: &[crate::intrinsic_measurement::IntrinsicMeasurement], _: f32, _: f32, _: f32) -> Result<GeometryResult, String> {
    Err("mixed-grid geometry adapter is not enabled".into())
}
