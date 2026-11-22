//! Feature-gated Taffy geometry adapter.
//!
//! Altru Browser owns the layout contract. Taffy receives only normalized
//! geometry inputs after DOM, CSS parsing/cascade and formatting-context
//! selection have already been decided by the native engine.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryMode {
    FlexRow,
    FlexColumn,
    Grid { columns: u16 },
}

/// Main-axis distribution of free space in a flex line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GeometryJustify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Cross-axis alignment of flex items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GeometryAlign {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

/// One item. All sizes are already resolved to CSS pixels by the native engine;
/// Taffy never sees a percentage, a font or a DOM node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryChild {
    pub min_height: f32,
    pub width: Option<f32>,
    pub min_width: f32,
    pub max_width: Option<f32>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    /// `None` = `auto`.
    pub flex_basis: Option<f32>,
}

impl GeometryChild {
    /// An item the engine has no author sizing or measurement for: it shares the
    /// free space equally with its siblings (the engine's original behavior).
    pub const fn shared(min_height: f32) -> Self {
        Self {
            min_height,
            width: None,
            min_width: 0.0,
            max_width: None,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            flex_basis: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryRequest {
    pub mode: GeometryMode,
    pub width: f32,
    pub gap: f32,
    pub justify: GeometryJustify,
    pub align: GeometryAlign,
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
            let finite = |v: f32| v.is_finite() && v >= 0.0;
            if !finite(child.min_width)
                || !finite(child.flex_grow)
                || !finite(child.flex_shrink)
                || child.width.is_some_and(|v| !finite(v))
                || child.max_width.is_some_and(|v| !finite(v))
                || child.flex_basis.is_some_and(|v| !finite(v))
            {
                return Err("invalid owned geometry child".into());
            }
            let node = tree
                .new_leaf(Style {
                    size: Size {
                        width: child.width.map_or(Dimension::AUTO, Dimension::from_length),
                        height: Dimension::AUTO,
                    },
                    min_size: Size {
                        width: LengthPercentageAuto::length(child.min_width),
                        height: LengthPercentageAuto::length(child.min_height.max(1.0)),
                    },
                    max_size: Size {
                        width: child
                            .max_width
                            .map_or_else(LengthPercentageAuto::auto, LengthPercentageAuto::length),
                        height: LengthPercentageAuto::auto(),
                    },
                    flex_grow: child.flex_grow,
                    flex_shrink: child.flex_shrink,
                    flex_basis: child
                        .flex_basis
                        .map_or(Dimension::AUTO, Dimension::from_length),
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

        if !matches!(request.mode, GeometryMode::Grid { .. }) {
            style.justify_content = Some(match request.justify {
                GeometryJustify::Start => taffy::style::JustifyContent::FLEX_START,
                GeometryJustify::Center => taffy::style::JustifyContent::CENTER,
                GeometryJustify::End => taffy::style::JustifyContent::FLEX_END,
                GeometryJustify::SpaceBetween => taffy::style::JustifyContent::SPACE_BETWEEN,
            });
            style.align_items = Some(match request.align {
                GeometryAlign::Stretch => taffy::style::AlignItems::STRETCH,
                GeometryAlign::Start => taffy::style::AlignItems::FLEX_START,
                GeometryAlign::Center => taffy::style::AlignItems::CENTER,
                GeometryAlign::End => taffy::style::AlignItems::FLEX_END,
            });
        }

        match request.mode {
            GeometryMode::FlexRow => {
                style.display = taffy::style::Display::Flex;
                style.flex_direction = taffy::style::FlexDirection::Row;
            }
            GeometryMode::FlexColumn => {
                style.display = taffy::style::Display::Flex;
                style.flex_direction = taffy::style::FlexDirection::Column;
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
            if (layout.location.x - expected).abs() > 0.05
                || (layout.size.width - candidate.widths_px[index]).abs() > 0.05
            {
                return Err("candidate/Taffy geometry disagrees".into());
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
                justify: GeometryJustify::default(),
                align: GeometryAlign::default(),
                children: vec![GeometryChild::shared(40.0), GeometryChild::shared(40.0)],
            })
            .unwrap();
            assert_eq!(result.width, 300.0);
            assert_eq!(result.children.len(), 2);
            assert!(result.children[1].x > result.children[0].x);
            assert!(result.height >= 40.0);
        }

        #[test]
        fn sized_and_growing_items_share_a_row_like_css() {
            // A 34%-wide sidebar capped at 280px next to a `flex: 1` editor.
            let sidebar = GeometryChild {
                width: Some(340.0),
                min_width: 160.0,
                max_width: Some(280.0),
                flex_grow: 0.0,
                ..GeometryChild::shared(40.0)
            };
            let editor = GeometryChild {
                flex_grow: 1.0,
                flex_basis: Some(0.0),
                ..GeometryChild::shared(40.0)
            };
            let result = solve_geometry(&GeometryRequest {
                mode: GeometryMode::FlexRow,
                width: 1000.0,
                gap: 0.0,
                justify: GeometryJustify::Start,
                align: GeometryAlign::Stretch,
                children: vec![sidebar, editor],
            })
            .unwrap();
            assert_eq!(result.children[0].width, 280.0);
            assert_eq!(result.children[1].x, 280.0);
            assert_eq!(result.children[1].width, 720.0);
        }

        #[test]
        fn space_between_pushes_fixed_items_to_the_edges() {
            let fixed = |w| GeometryChild {
                flex_grow: 0.0,
                flex_basis: Some(w),
                ..GeometryChild::shared(20.0)
            };
            let result = solve_geometry(&GeometryRequest {
                mode: GeometryMode::FlexRow,
                width: 1000.0,
                gap: 12.0,
                justify: GeometryJustify::SpaceBetween,
                align: GeometryAlign::Center,
                children: vec![fixed(50.0), fixed(170.0), fixed(90.0)],
            })
            .unwrap();
            assert_eq!(result.children[0].x, 0.0);
            assert_eq!(result.children[2].x + result.children[2].width, 1000.0);
            let left_space = result.children[1].x - 50.0;
            let right_space = result.children[2].x - (result.children[1].x + 170.0);
            assert!(
                (left_space - right_space).abs() < 0.01,
                "{left_space} vs {right_space}"
            );
        }

        #[test]
        fn invalid_child_sizes_fail_closed() {
            let request = GeometryRequest {
                mode: GeometryMode::FlexRow,
                width: 100.0,
                gap: 0.0,
                justify: GeometryJustify::Start,
                align: GeometryAlign::Stretch,
                children: vec![GeometryChild {
                    max_width: Some(f32::NAN),
                    ..GeometryChild::shared(10.0)
                }],
            };
            assert!(solve_geometry(&request).is_err());
        }

        #[test]
        fn owned_grid_contract_produces_two_columns() {
            let result = solve_geometry(&GeometryRequest {
                mode: GeometryMode::Grid { columns: 2 },
                width: 300.0,
                gap: 10.0,
                justify: GeometryJustify::default(),
                align: GeometryAlign::default(),
                children: vec![
                    GeometryChild::shared(30.0),
                    GeometryChild::shared(30.0),
                    GeometryChild::shared(30.0),
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
pub use enabled::solve_geometry;

#[cfg(not(feature = "taffy-layout"))]
pub fn solve_geometry(_: &GeometryRequest) -> Result<GeometryResult, String> {
    Err("flex/grid geometry adapter is not enabled".into())
}
