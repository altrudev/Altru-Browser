//! Feature-gated Taffy experiment.
//!
//! Taffy is evaluated as a geometry primitive only. AWEF remains authoritative
//! for DOM, CSS parsing/cascade, formatting-context selection, and evidence.

#[cfg(feature = "taffy-layout")]
mod enabled {
    use taffy::prelude::*;

    #[derive(Debug, Clone, PartialEq)]
    pub struct TaffyFlexEvidence {
        pub root_width: f32,
        pub root_height: f32,
        pub first_x: f32,
        pub second_x: f32,
        pub child_width: f32,
    }

    pub fn flex_probe() -> Result<TaffyFlexEvidence, String> {
        let mut tree: TaffyTree<()> = TaffyTree::new();

        let child_style = Style {
            size: Size {
                width: Dimension::from_length(100.0),
                height: Dimension::from_length(40.0),
            },
            ..Default::default()
        };

        let first = tree
            .new_leaf(child_style.clone())
            .map_err(|error| error.to_string())?;
        let second = tree
            .new_leaf(child_style)
            .map_err(|error| error.to_string())?;

        let root = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: Dimension::from_length(300.0),
                        height: Dimension::from_length(100.0),
                    },
                    ..Default::default()
                },
                &[first, second],
            )
            .map_err(|error| error.to_string())?;

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(100.0),
            },
        )
        .map_err(|error| error.to_string())?;

        let root_layout = tree.layout(root).map_err(|error| error.to_string())?;
        let first_layout = tree.layout(first).map_err(|error| error.to_string())?;
        let second_layout = tree.layout(second).map_err(|error| error.to_string())?;

        Ok(TaffyFlexEvidence {
            root_width: root_layout.size.width,
            root_height: root_layout.size.height,
            first_x: first_layout.location.x,
            second_x: second_layout.location.x,
            child_width: first_layout.size.width,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn taffy_flex_candidate_produces_expected_geometry() {
            let evidence = flex_probe().unwrap();
            assert_eq!(evidence.root_width, 300.0);
            assert_eq!(evidence.root_height, 100.0);
            assert_eq!(evidence.first_x, 0.0);
            assert_eq!(evidence.second_x, 100.0);
            assert_eq!(evidence.child_width, 100.0);
        }
    }
}

#[cfg(feature = "taffy-layout")]
pub use enabled::{TaffyFlexEvidence, flex_probe};
