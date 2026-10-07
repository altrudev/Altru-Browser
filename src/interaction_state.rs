//! Explicit browser-page interaction state.
//!
//! CSS state pseudo-classes are evaluated against this snapshot. The default
//! snapshot is intentionally empty: syntax support never fabricates focus,
//! hover, or active state.

use crate::capability_ir::AcirInteractionPredicate;
use crate::native_dom::{NativeDocument, NodeId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InteractionSnapshot {
    pub focused_node: Option<NodeId>,
    pub focus_visible_node: Option<NodeId>,
    pub hovered_nodes: Vec<NodeId>,
    pub active_node: Option<NodeId>,
}

impl InteractionSnapshot {
    pub fn matches(
        &self,
        document: &NativeDocument,
        node: NodeId,
        predicate: AcirInteractionPredicate,
    ) -> bool {
        match predicate {
            AcirInteractionPredicate::Focus => self.focused_node == Some(node),
            AcirInteractionPredicate::FocusVisible => self.focus_visible_node == Some(node),
            AcirInteractionPredicate::FocusWithin => {
                self.focused_node.is_some_and(|focused| {
                    focused == node || document.ancestor_elements(focused).contains(&node)
                })
            }
            AcirInteractionPredicate::Hover => self.hovered_nodes.contains(&node),
            AcirInteractionPredicate::Active => self.active_node == Some(node),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_never_invents_interaction_state() {
        let mut document = NativeDocument::new();
        let node = document.append_element(document.root(), "button").unwrap();
        let snapshot = InteractionSnapshot::default();

        for predicate in [
            AcirInteractionPredicate::Focus,
            AcirInteractionPredicate::FocusVisible,
            AcirInteractionPredicate::FocusWithin,
            AcirInteractionPredicate::Hover,
            AcirInteractionPredicate::Active,
        ] {
            assert!(!snapshot.matches(&document, node, predicate));
        }
    }

    #[test]
    fn focus_within_tracks_focused_descendant() {
        let mut document = NativeDocument::new();
        let container = document.append_element(document.root(), "div").unwrap();
        let button = document.append_element(container, "button").unwrap();
        let snapshot = InteractionSnapshot {
            focused_node: Some(button),
            focus_visible_node: Some(button),
            ..InteractionSnapshot::default()
        };

        assert!(snapshot.matches(
            &document,
            button,
            AcirInteractionPredicate::Focus
        ));
        assert!(snapshot.matches(
            &document,
            button,
            AcirInteractionPredicate::FocusVisible
        ));
        assert!(snapshot.matches(
            &document,
            container,
            AcirInteractionPredicate::FocusWithin
        ));
        assert!(!snapshot.matches(
            &document,
            container,
            AcirInteractionPredicate::Focus
        ));
    }
}
