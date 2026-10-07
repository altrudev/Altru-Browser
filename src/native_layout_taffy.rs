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

    #[cfg(test)]
    mod tests {
        use super::*;

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
pub use enabled::solve_geometry;

#[cfg(not(feature = "taffy-layout"))]
pub fn solve_geometry(_: &GeometryRequest) -> Result<GeometryResult, String> {
    Err("flex/grid geometry adapter is not enabled".into())
}
