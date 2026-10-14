//! Source-order resource manifest for the native rendering dependency graph.
//! Discovery grants no network capability. The host broker must authorize every fetch.
use crate::native_dom::{NativeDocument, NodeId, NodeKind};
use crate::resource_api::ResourceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDependency {
    pub node: NodeId,
    pub kind: ResourceKind,
    pub reference: String,
    pub cors_required: bool,
}

pub fn discover_dependencies(document: &NativeDocument) -> Vec<NativeDependency> {
    let mut result = Vec::new();
    for node in document.nodes() {
        let NodeKind::Element { tag } = &node.kind else { continue; };
        let attr = |name: &str| node.attributes.iter()
            .find(|attribute| attribute.name == name)
            .map(|attribute| attribute.value.as_str());
        let candidate = match tag.as_str() {
            "link" => {
                let rel = attr("rel").unwrap_or_default();
                let tokens = rel.split_ascii_whitespace();
                if tokens.into_iter().any(|token| token.eq_ignore_ascii_case("stylesheet")) {
                    attr("href").map(|v| (ResourceKind::StyleSheet,v))
                } else if rel.split_ascii_whitespace().any(|t| t.eq_ignore_ascii_case("preload")) {
                    let kind = match attr("as").unwrap_or_default() {
                        "style" => ResourceKind::StyleSheet,
                        "font" => ResourceKind::Font,
                        "image" => ResourceKind::Image,
                        "script" => ResourceKind::Script,
                        _ => ResourceKind::Other,
                    };
                    attr("href").map(|v| (kind,v))
                } else { None }
            }
            "img" => attr("src").map(|v| (ResourceKind::Image,v)),
            "script" => attr("src").map(|v| (ResourceKind::Script,v)),
            "source" => attr("src").map(|v| (ResourceKind::Media,v)),
            _ => None,
        };
        if let Some((kind, reference)) = candidate {
            let reference = reference.trim();
            if !reference.is_empty() && result.len() < 100_000 {
                result.push(NativeDependency {
                    node: node.id,
                    kind,
                    reference: reference.to_owned(),
                    cors_required: tag == "link" && attr("crossorigin").is_some(),
                });
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_html::parse_document;
    #[test]
    fn resource_manifest_is_explicit_ordered_and_authority_free() {
        let html = "<html><head><link rel='preload stylesheet' href='/site.css'><link rel='icon' href='/icon.ico'><link rel='preload' as='font' href='/font.woff2'></head><body><img src='/hero.png'><script src='/main.js'></script><img src=''></body></html>";
        let document = parse_document(html).unwrap();
        let deps = discover_dependencies(&document);
        assert_eq!(deps.len(),4);
        assert_eq!(deps.iter().map(|d| d.kind).collect::<Vec<_>>(),
            vec![ResourceKind::StyleSheet,ResourceKind::Font,ResourceKind::Image,ResourceKind::Script]);
        assert_eq!(deps[0].reference,"/site.css");
        assert!(!deps[0].cors_required);
        assert_eq!(deps[2].reference,"/hero.png");
    }
    #[test]
    fn malformed_or_inert_sources_do_not_gain_authority() {
        let document = parse_document("<html><body><a href='https://x.invalid'>x</a><img><link href='/raw.css'><script>hi</script></body></html>").unwrap();
        assert!(discover_dependencies(&document).is_empty());
    }
}
