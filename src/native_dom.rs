//! AWEF-owned native document model.
//!
//! This deliberately starts small. The important property is ownership of the
//! semantic model: external parsers may eventually feed this structure, but
//! they do not define it.

pub type NodeId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Document,
    Element { tag: String },
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDocument {
    nodes: Vec<Node>,
    root: NodeId,
    mutation_epoch: u64,
}

impl NativeDocument {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node {
                id: 0,
                parent: None,
                children: Vec::new(),
                kind: NodeKind::Document,
                attributes: Vec::new(),
            }],
            root: 0,
            mutation_epoch: 0,
        }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn mutation_epoch(&self) -> u64 {
        self.mutation_epoch
    }

    pub fn append_element(&mut self, parent: NodeId, tag: impl Into<String>) -> Option<NodeId> {
        self.append_element_with_attributes(parent, tag, Vec::new())
    }

    pub fn append_element_with_attributes(
        &mut self,
        parent: NodeId,
        tag: impl Into<String>,
        attributes: Vec<Attribute>,
    ) -> Option<NodeId> {
        self.append(parent, NodeKind::Element { tag: tag.into() }, attributes)
    }

    pub fn append_text(&mut self, parent: NodeId, text: impl Into<String>) -> Option<NodeId> {
        self.append(parent, NodeKind::Text(text.into()), Vec::new())
    }

    pub fn element_parent(&self, node: NodeId) -> Option<NodeId> {
        let parent = self.node(node)?.parent?;
        matches!(self.node(parent)?.kind, NodeKind::Element { .. }).then_some(parent)
    }

    pub fn element_children(&self, node: NodeId) -> Vec<NodeId> {
        self.node(node)
            .map(|candidate| {
                candidate
                    .children
                    .iter()
                    .copied()
                    .filter(|child| {
                        matches!(
                            self.node(*child).map(|node| &node.kind),
                            Some(NodeKind::Element { .. })
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn previous_element_sibling(&self, node: NodeId) -> Option<NodeId> {
        let parent = self.node(node)?.parent?;
        let siblings = &self.node(parent)?.children;
        let position = siblings.iter().position(|candidate| *candidate == node)?;
        siblings[..position]
            .iter()
            .rev()
            .copied()
            .find(|candidate| {
                matches!(
                    self.node(*candidate).map(|node| &node.kind),
                    Some(NodeKind::Element { .. })
                )
            })
    }

    pub fn next_element_sibling(&self, node: NodeId) -> Option<NodeId> {
        let parent = self.node(node)?.parent?;
        let siblings = &self.node(parent)?.children;
        let position = siblings.iter().position(|candidate| *candidate == node)?;
        siblings[position + 1..]
            .iter()
            .copied()
            .find(|candidate| {
                matches!(
                    self.node(*candidate).map(|node| &node.kind),
                    Some(NodeKind::Element { .. })
                )
            })
    }

    pub fn previous_element_siblings(&self, node: NodeId) -> Vec<NodeId> {
        let Some(parent) = self.node(node).and_then(|candidate| candidate.parent) else {
            return Vec::new();
        };
        let Some(parent_node) = self.node(parent) else {
            return Vec::new();
        };
        let Some(position) = parent_node.children.iter().position(|candidate| *candidate == node)
        else {
            return Vec::new();
        };
        parent_node.children[..position]
            .iter()
            .copied()
            .filter(|candidate| {
                matches!(
                    self.node(*candidate).map(|node| &node.kind),
                    Some(NodeKind::Element { .. })
                )
            })
            .collect()
    }

    pub fn element_index(&self, node: NodeId) -> Option<usize> {
        let parent = self.node(node)?.parent?;
        self.element_children(parent)
            .iter()
            .position(|candidate| *candidate == node)
            .map(|index| index + 1)
    }

    pub fn ancestor_elements(&self, node: NodeId) -> Vec<NodeId> {
        let mut ancestors = Vec::new();
        let mut current = self.node(node).and_then(|candidate| candidate.parent);
        while let Some(id) = current {
            if matches!(self.node(id).map(|node| &node.kind), Some(NodeKind::Element { .. })) {
                ancestors.push(id);
            }
            current = self.node(id).and_then(|candidate| candidate.parent);
        }
        ancestors
    }

    pub fn attribute(&self, node: NodeId, name: &str) -> Option<&str> {
        self.node(node)?
            .attributes
            .iter()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
            .map(|attribute| attribute.value.as_str())
    }

    pub fn set_attribute(
        &mut self,
        node: NodeId,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<crate::native_invalidation::Invalidation> {
        let name = name.into().to_ascii_lowercase();
        let value = value.into();
        let candidate = self.nodes.get_mut(node)?;
        let NodeKind::Element { .. } = candidate.kind else {
            return None;
        };

        if let Some(attribute) = candidate
            .attributes
            .iter_mut()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(&name))
        {
            attribute.value = value;
        } else {
            candidate.attributes.push(Attribute {
                name: name.clone(),
                value,
            });
        }
        self.mutation_epoch = self.mutation_epoch.saturating_add(1);
        Some(crate::native_invalidation::for_attribute(node, &name))
    }

    pub fn replace_text(
        &mut self,
        node: NodeId,
        text: impl Into<String>,
    ) -> Option<crate::native_invalidation::Invalidation> {
        let candidate = self.nodes.get_mut(node)?;
        let NodeKind::Text(current) = &mut candidate.kind else {
            return None;
        };
        *current = text.into();
        self.mutation_epoch = self.mutation_epoch.saturating_add(1);
        Some(crate::native_invalidation::for_text(node))
    }

    fn append(
        &mut self,
        parent: NodeId,
        kind: NodeKind,
        attributes: Vec<Attribute>,
    ) -> Option<NodeId> {
        if parent >= self.nodes.len() {
            return None;
        }

        let id = self.nodes.len();
        self.nodes.push(Node {
            id,
            parent: Some(parent),
            children: Vec::new(),
            kind,
            attributes,
        });
        self.nodes[parent].children.push(id);
        self.mutation_epoch = self.mutation_epoch.saturating_add(1);
        Some(id)
    }
}

impl Default for NativeDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_epoch_tracks_owned_dom_changes() {
        let mut document = NativeDocument::new();
        let body = document.append_element(document.root(), "body").unwrap();
        document.append_text(body, "hello").unwrap();
        assert_eq!(document.mutation_epoch(), 2);
        assert_eq!(document.nodes().len(), 3);
    }

    #[test]
    fn attributes_are_owned_by_awef_dom() {
        let mut document = NativeDocument::new();
        let node = document
            .append_element_with_attributes(
                document.root(),
                "p",
                vec![Attribute {
                    name: "class".into(),
                    value: "lead".into(),
                }],
            )
            .unwrap();

        assert_eq!(document.attribute(node, "CLASS"), Some("lead"));
    }

    #[test]
    fn mutation_methods_return_explicit_invalidation() {
        let mut document = NativeDocument::new();
        let p = document.append_element(document.root(), "p").unwrap();
        let text = document.append_text(p, "old").unwrap();

        let style = document.set_attribute(p, "class", "lead").unwrap();
        assert!(style.style && style.layout && style.paint);

        let text_change = document.replace_text(text, "new").unwrap();
        assert!(!text_change.style);
        assert!(text_change.layout && text_change.paint);
    }

    #[test]
    fn relation_pack_exposes_element_tree_semantics() {
        let mut document = NativeDocument::new();
        let body = document.append_element(document.root(), "body").unwrap();
        let first = document.append_element(body, "p").unwrap();
        document.append_text(body, "text").unwrap();
        let second = document.append_element(body, "section").unwrap();
        let child = document.append_element(second, "span").unwrap();

        assert_eq!(document.element_parent(child), Some(second));
        assert_eq!(document.element_children(body), vec![first, second]);
        assert_eq!(document.previous_element_sibling(second), Some(first));
        assert_eq!(document.next_element_sibling(first), Some(second));
        assert_eq!(document.previous_element_siblings(second), vec![first]);
        assert_eq!(document.element_index(first), Some(1));
        assert_eq!(document.element_index(second), Some(2));
        assert_eq!(document.ancestor_elements(child), vec![second, body]);
    }

    #[test]
    fn invalid_parent_fails_closed() {
        let mut document = NativeDocument::new();
        assert!(document.append_element(999, "body").is_none());
        assert_eq!(document.mutation_epoch(), 0);
    }
}
