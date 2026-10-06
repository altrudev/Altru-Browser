//! Conservative N2 invalidation contracts.
//!
//! Until selector-dependency indexing exists, mutations prefer redoing too much
//! work over missing a style/layout consequence.

use crate::native_dom::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationScope {
    Node,
    Subtree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalidation {
    pub node: NodeId,
    pub scope: InvalidationScope,
    pub style: bool,
    pub layout: bool,
    pub paint: bool,
}

pub fn for_attribute(node: NodeId, _name: &str) -> Invalidation {
    Invalidation {
        node,
        scope: InvalidationScope::Subtree,
        style: true,
        layout: true,
        paint: true,
    }
}

pub fn for_text(node: NodeId) -> Invalidation {
    Invalidation {
        node,
        scope: InvalidationScope::Node,
        style: false,
        layout: true,
        paint: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_mutation_conservatively_invalidates_subtree() {
        let invalidation = for_attribute(7, "class");
        assert_eq!(invalidation.scope, InvalidationScope::Subtree);
        assert!(invalidation.style);
        assert!(invalidation.layout);
        assert!(invalidation.paint);
    }

    #[test]
    fn text_mutation_skips_style_but_requires_layout_and_paint() {
        let invalidation = for_text(3);
        assert_eq!(invalidation.scope, InvalidationScope::Node);
        assert!(!invalidation.style);
        assert!(invalidation.layout);
        assert!(invalidation.paint);
    }
}
