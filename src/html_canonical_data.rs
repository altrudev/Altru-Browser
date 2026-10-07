//! Canonical HTML data pack.
//!
//! This module contains pinned, table-driven HTML knowledge that should not be
//! rediscovered one site at a time. The named-character dataset is vendored
//! from WHATWG and parsed once per process; no runtime network access occurs.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const WHATWG_ENTITIES_JSON: &str =
    include_str!("../capability-library/data/whatwg-entities.json");

pub const WHATWG_ENTITIES_SOURCE: &str = "https://html.spec.whatwg.org/entities.json";
pub const WHATWG_ENTITIES_SHA256: &str =
    "d741d877ac77c4194c4ad526b5b4a19aef8dfe411ab840a466891cdbb9f362e6";

pub const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
    "source", "track", "wbr",
];

pub const RAW_TEXT_ELEMENTS: &[&str] = &["script", "style"];

/// RCDATA elements are catalogued now even though the tokenizer state is a
/// separate capability from raw-text handling.
pub const RCDATA_ELEMENTS: &[&str] = &["textarea", "title"];

#[derive(Debug, Clone, Deserialize)]
struct NamedEntity {
    characters: String,
}

fn named_entities() -> &'static HashMap<String, NamedEntity> {
    static TABLE: OnceLock<HashMap<String, NamedEntity>> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(WHATWG_ENTITIES_JSON)
            .expect("vendored WHATWG named-character dataset must parse")
    })
}

/// Decode a semicolon-terminated HTML named character reference.
///
/// The input excludes the leading ampersand and trailing semicolon. This v1 API
/// is deliberately strict about the semicolon; legacy semicolon omission
/// remains a separately verified tokenizer capability.
pub fn decode_named_reference(name: &str) -> Option<&'static str> {
    let key = format!("&{name};");
    named_entities()
        .get(&key)
        .map(|entry| entry.characters.as_str())
}

pub fn is_void_element(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
}

pub fn is_raw_text_element(tag: &str) -> bool {
    RAW_TEXT_ELEMENTS.contains(&tag)
}

pub fn is_rcdata_element(tag: &str) -> bool {
    RCDATA_ELEMENTS.contains(&tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendored_named_reference_table_has_expected_shape() {
        assert_eq!(named_entities().len(), 2_231);
        assert_eq!(decode_named_reference("amp"), Some("&"));
        assert_eq!(decode_named_reference("copy"), Some("©"));
        assert_eq!(decode_named_reference("hellip"), Some("…"));
    }

    #[test]
    fn multi_codepoint_named_reference_is_preserved() {
        assert_eq!(decode_named_reference("NotEqualTilde"), Some("≂̸"));
    }

    #[test]
    fn html_element_classes_are_table_driven() {
        assert!(is_void_element("img"));
        assert!(is_raw_text_element("script"));
        assert!(is_rcdata_element("textarea"));
        assert!(!is_void_element("div"));
    }
}
