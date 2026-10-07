//! AWEF-owned N2 CSS parser and selector model.
//!
//! This is deliberately a bounded CSS subset. Unsupported selectors fail
//! closed; unknown properties follow CSS error handling and are ignored.

use crate::capability_compiler::{
    CapabilityCompilerError, TranslationReceipt, compile_selector_capability,
    compile_stylesheet_capabilities,
};
use crate::capability_ir::{AcirSelectorRelation, CapabilityEnvironment};
use crate::native_dom::{NativeDocument, NodeId, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssError {
    MalformedRule,
    UnsupportedSelector(String),
    MalformedDeclaration(String),
    InvalidValue { property: String, value: String },
    UnresolvedCustomProperty(String),
    UnsupportedLayout(String),
    CapabilityCompiler(CapabilityCompilerError),
}

impl From<CapabilityCompilerError> for CssError {
    fn from(value: CapabilityCompilerError) -> Self {
        Self::CapabilityCompiler(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeOperator {
    Equals,
    Prefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    pub name: String,
    pub operator: AttributeOperator,
    pub value: String,
    pub case_insensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<AttributeSelector>,
    pub root: bool,
    pub any_of: Vec<Selector>,
    pub ancestor: Option<(AcirSelectorRelation, Box<Selector>)>,
}

impl Selector {
    pub fn specificity(&self) -> (u16, u16, u16) {
        let base = (
            u16::from(self.id.is_some()),
            (self.classes.len() + self.attributes.len() + usize::from(self.root))
                .min(u16::MAX as usize) as u16,
            u16::from(self.tag.is_some()),
        );
        let nested = self
            .any_of
            .iter()
            .map(Selector::specificity)
            .max()
            .unwrap_or((0, 0, 0));
        let ancestor = self
            .ancestor
            .as_ref()
            .map(|(_, selector)| selector.specificity())
            .unwrap_or((0, 0, 0));
        (
            base.0
                .saturating_add(nested.0)
                .saturating_add(ancestor.0),
            base.1
                .saturating_add(nested.1)
                .saturating_add(ancestor.1),
            base.2
                .saturating_add(nested.2)
                .saturating_add(ancestor.2),
        )
    }

    pub fn matches(&self, document: &NativeDocument, node: NodeId) -> bool {
        let Some(candidate) = document.node(node) else {
            return false;
        };
        let NodeKind::Element { tag } = &candidate.kind else {
            return false;
        };

        if let Some(expected) = &self.tag
            && tag != expected
        {
            return false;
        }

        if let Some(expected) = &self.id
            && document.attribute(node, "id") != Some(expected.as_str())
        {
            return false;
        }

        if self.root && candidate.parent != Some(document.root()) {
            return false;
        }

        if !self.classes.is_empty() {
            let classes = document
                .attribute(node, "class")
                .unwrap_or_default()
                .split_ascii_whitespace()
                .collect::<Vec<_>>();
            if self
                .classes
                .iter()
                .any(|required| !classes.contains(&required.as_str()))
            {
                return false;
            }
        }

        for attribute in &self.attributes {
            let Some(actual) = document.attribute(node, &attribute.name) else {
                return false;
            };
            let matched = if attribute.case_insensitive {
                let actual = actual.to_ascii_lowercase();
                let expected = attribute.value.to_ascii_lowercase();
                match attribute.operator {
                    AttributeOperator::Equals => actual == expected,
                    AttributeOperator::Prefix => actual.starts_with(&expected),
                }
            } else {
                match attribute.operator {
                    AttributeOperator::Equals => actual == attribute.value,
                    AttributeOperator::Prefix => actual.starts_with(&attribute.value),
                }
            };
            if !matched {
                return false;
            }
        }

        if !self.any_of.is_empty()
            && !self
                .any_of
                .iter()
                .any(|selector| selector.matches(document, node))
        {
            return false;
        }

        if let Some((relation, ancestor)) = &self.ancestor {
            let matched = match relation {
                AcirSelectorRelation::Descendant => {
                    let mut current = candidate.parent;
                    let mut found = false;
                    while let Some(parent_id) = current {
                        if ancestor.matches(document, parent_id) {
                            found = true;
                            break;
                        }
                        current = document.node(parent_id).and_then(|parent| parent.parent);
                    }
                    found
                }
            };
            if !matched {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    Display(String),
    Keyword(String),
    GridColumns(u16),
    Px(f32),
    Var(String),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub property: String,
    pub value: CssValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleSheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleSheet {
    pub stylesheet: StyleSheet,
    pub translations: Vec<TranslationReceipt>,
}

fn parse_attribute_selector(input: &str) -> Result<AttributeSelector, CssError> {
    let input = input.trim();
    let (body, case_insensitive) = if let Some(body) = input.strip_suffix(" i") {
        (body.trim_end(), true)
    } else {
        (input, false)
    };

    let (name, operator, raw_value) = if let Some((name, value)) = body.split_once("^=") {
        (name, AttributeOperator::Prefix, value)
    } else if let Some((name, value)) = body.split_once('=') {
        (name, AttributeOperator::Equals, value)
    } else {
        return Err(CssError::UnsupportedSelector(format!("[{input}]")));
    };

    let name = name.trim().to_ascii_lowercase();
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'))
    {
        return Err(CssError::UnsupportedSelector(format!("[{input}]")));
    }

    let raw_value = raw_value.trim();
    let value = if (raw_value.starts_with('"') && raw_value.ends_with('"'))
        || (raw_value.starts_with('\'') && raw_value.ends_with('\''))
    {
        if raw_value.len() < 2 {
            return Err(CssError::UnsupportedSelector(format!("[{input}]")));
        }
        raw_value[1..raw_value.len() - 1].to_string()
    } else {
        raw_value.to_string()
    };
    if value.is_empty() {
        return Err(CssError::UnsupportedSelector(format!("[{input}]")));
    }

    Ok(AttributeSelector {
        name,
        operator,
        value,
        case_insensitive,
    })
}

fn split_selector_list(input: &str) -> Result<Vec<&str>, CssError> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut bracket_depth = 0usize;
    let mut quote: Option<char> = None;

    for (index, ch) in input.char_indices() {
        if let Some(current_quote) = quote {
            if ch == current_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => {
                if bracket_depth == 0 {
                    return Err(CssError::UnsupportedSelector(input.into()));
                }
                bracket_depth -= 1;
            }
            ',' if bracket_depth == 0 => {
                let part = input[start..index].trim();
                if part.is_empty() {
                    return Err(CssError::UnsupportedSelector(input.into()));
                }
                parts.push(part);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    if quote.is_some() || bracket_depth != 0 {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    let tail = input[start..].trim();
    if tail.is_empty() {
        return Err(CssError::UnsupportedSelector(input.into()));
    }
    parts.push(tail);
    Ok(parts)
}

fn parse_simple_selector(input: &str) -> Result<Selector, CssError> {
    let input = input.trim();
    if input.is_empty()
        || input.contains('>')
        || input.contains('+')
        || input.contains('~')
        || input.contains('*')
        || input.contains(':')
        || input
            .chars()
            .any(|ch| ch.is_whitespace() && !input.contains('['))
    {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    let mut selector = Selector {
        tag: None,
        id: None,
        classes: Vec::new(),
        attributes: Vec::new(),
        root: false,
        any_of: Vec::new(),
        ancestor: None,
    };
    let mut cursor = 0usize;

    if !matches!(input.as_bytes().first(), Some(b'.' | b'#' | b'[')) {
        let end = input.find(['.', '#', '[']).unwrap_or(input.len());
        let tag = input[..end].trim();
        if tag.is_empty() || tag.chars().any(char::is_whitespace) {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        selector.tag = Some(tag.to_ascii_lowercase());
        cursor = end;
    }

    while cursor < input.len() {
        match input.as_bytes()[cursor] {
            b'.' | b'#' => {
                let marker = input.as_bytes()[cursor];
                cursor += 1;
                let rest = &input[cursor..];
                let end_rel = rest.find(['.', '#', '[']).unwrap_or(rest.len());
                let value = &rest[..end_rel];
                if value.is_empty() || value.chars().any(char::is_whitespace) {
                    return Err(CssError::UnsupportedSelector(input.into()));
                }
                if marker == b'#' {
                    if selector.id.is_some() {
                        return Err(CssError::UnsupportedSelector(input.into()));
                    }
                    selector.id = Some(value.into());
                } else {
                    selector.classes.push(value.into());
                }
                cursor += end_rel;
            }
            b'[' => {
                let rest = &input[cursor + 1..];
                let Some(close_rel) = rest.find(']') else {
                    return Err(CssError::UnsupportedSelector(input.into()));
                };
                let body = &rest[..close_rel];
                selector.attributes.push(parse_attribute_selector(body)?);
                cursor += close_rel + 2;
            }
            _ => return Err(CssError::UnsupportedSelector(input.into())),
        }
    }

    if selector.tag.is_none()
        && selector.id.is_none()
        && selector.classes.is_empty()
        && selector.attributes.is_empty()
        && !selector.root
    {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    Ok(selector)
}

fn parse_selector(input: &str) -> Result<Selector, CssError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    let compiled_selector = match compile_selector_capability(input) {
        Ok(compiled) => compiled,
        Err(CapabilityCompilerError::UnsupportedSelector(_)) => {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        Err(error) => return Err(error.into()),
    };

    if let Some(compiled) = compiled_selector {
        let mut current: Option<Selector> = None;
        for (index, compound) in compiled.chain.compounds.iter().enumerate() {
            let mut selector = parse_selector(compound)?;
            if index > 0 {
                let relation = compiled.chain.combinators[index - 1];
                let ancestor = current
                    .take()
                    .ok_or_else(|| CssError::UnsupportedSelector(input.into()))?;
                selector.ancestor = Some((relation, Box::new(ancestor)));
            }
            current = Some(selector);
        }
        return current.ok_or_else(|| CssError::UnsupportedSelector(input.into()));
    }

    if input == ":root" {
        return Ok(Selector {
            tag: None,
            id: None,
            classes: Vec::new(),
            attributes: Vec::new(),
            root: true,
            any_of: Vec::new(),
            ancestor: None,
        });
    }

    if let Some(is_start) = input.find(":is(") {
        if !input.ends_with(')') || input[is_start + 4..input.len() - 1].contains(":is(") {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        let mut selector = parse_simple_selector(&input[..is_start])?;
        let inner = &input[is_start + 4..input.len() - 1];
        selector.any_of = split_selector_list(inner)?
            .into_iter()
            .map(parse_simple_selector)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(selector);
    }

    if input.contains(',') {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    parse_simple_selector(input)
}

fn parse_px(property: &str, value: &str) -> Result<f32, CssError> {
    let Some(number) = value.strip_suffix("px") else {
        return Err(CssError::InvalidValue {
            property: property.into(),
            value: value.into(),
        });
    };
    let parsed = number
        .trim()
        .parse::<f32>()
        .map_err(|_| CssError::InvalidValue {
            property: property.into(),
            value: value.into(),
        })?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(CssError::InvalidValue {
            property: property.into(),
            value: value.into(),
        });
    }
    Ok(parsed)
}

pub fn parse_declarations(input: &str) -> Result<Vec<Declaration>, CssError> {
    let mut declarations = Vec::new();

    for raw in input.split(';') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let Some((property, value)) = raw.split_once(':') else {
            return Err(CssError::MalformedDeclaration(raw.into()));
        };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim();

        let parsed = if property.starts_with("--") {
            Some(CssValue::Raw(value.into()))
        } else {
            match property.as_str() {
                "display" => match value {
                    "none" | "block" | "inline" | "flex" | "grid" => {
                        Some(CssValue::Display(value.into()))
                    }
                    _ => {
                        return Err(CssError::InvalidValue {
                            property,
                            value: value.into(),
                        });
                    }
                },
                "flex-direction" => match value {
                    "row" | "column" => Some(CssValue::Keyword(value.into())),
                    _ => {
                        return Err(CssError::InvalidValue {
                            property,
                            value: value.into(),
                        });
                    }
                },
                "grid-template-columns" => {
                    let tracks = value.split_ascii_whitespace().collect::<Vec<_>>();
                    if tracks.is_empty()
                        || tracks.len() > 12
                        || tracks.iter().any(|track| *track != "1fr")
                    {
                        return Err(CssError::InvalidValue {
                            property,
                            value: value.into(),
                        });
                    }
                    Some(CssValue::GridColumns(tracks.len() as u16))
                }
                "font-size" | "margin-top" | "margin-bottom" | "padding-top" | "padding-right"
                | "padding-bottom" | "padding-left" | "gap" => {
                    if let Some(inner) =
                        value.strip_prefix("var(").and_then(|v| v.strip_suffix(')'))
                    {
                        let name = inner.trim();
                        if !name.starts_with("--") {
                            return Err(CssError::InvalidValue {
                                property,
                                value: value.into(),
                            });
                        }
                        Some(CssValue::Var(name.into()))
                    } else {
                        Some(CssValue::Px(parse_px(&property, value)?))
                    }
                }
                // CSS-compatible recovery for properties N2 does not implement.
                _ => None,
            }
        };

        if let Some(value) = parsed {
            declarations.push(Declaration { property, value });
        }
    }

    Ok(declarations)
}

fn strip_css_comments(input: &str) -> Result<String, CssError> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;

    loop {
        let Some(start) = rest.find("/*") else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("*/") else {
            return Err(CssError::MalformedRule);
        };
        rest = &after_start[end + 2..];
    }

    Ok(output)
}

fn parse_compiled_stylesheet(
    input: &str,
) -> Result<(StyleSheet, Vec<TranslationReceipt>), CssError> {
    let mut rules = Vec::new();
    let mut translations = Vec::new();
    let mut rest = input;
    let mut order = 0usize;

    while !rest.trim().is_empty() {
        let Some(open) = rest.find('{') else {
            return Err(CssError::MalformedRule);
        };
        let Some(close_rel) = rest[open + 1..].find('}') else {
            return Err(CssError::MalformedRule);
        };
        let close = open + 1 + close_rel;

        let selector_source = rest[..open].trim();
        match compile_selector_capability(selector_source) {
            Ok(Some(compiled_selector)) => translations.push(compiled_selector.receipt),
            Ok(None) => {}
            Err(CapabilityCompilerError::UnsupportedSelector(_)) => {
                return Err(CssError::UnsupportedSelector(selector_source.into()));
            }
            Err(error) => return Err(error.into()),
        }
        let selector = parse_selector(selector_source)?;
        let declarations = parse_declarations(&rest[open + 1..close])?;
        rules.push(Rule {
            selector,
            declarations,
            order,
        });
        order += 1;
        rest = &rest[close + 1..];
    }

    Ok((StyleSheet { rules }, translations))
}
pub fn parse_stylesheet_with_environment(
    input: &str,
    environment: CapabilityEnvironment,
) -> Result<CompiledStyleSheet, CssError> {
    let cleaned = strip_css_comments(input)?;
    let compiled = compile_stylesheet_capabilities(&cleaned, environment)?;
    let (stylesheet, selector_translations) = parse_compiled_stylesheet(&compiled.source)?;
    let mut translations = compiled.receipts;
    translations.extend(selector_translations);
    Ok(CompiledStyleSheet {
        stylesheet,
        translations,
    })
}

pub fn parse_stylesheet(input: &str) -> Result<StyleSheet, CssError> {
    Ok(parse_stylesheet_with_environment(input, CapabilityEnvironment::default())?.stylesheet)
}

fn document_style_source(document: &NativeDocument) -> String {
    let mut combined = String::new();
    for node in document.nodes() {
        if matches!(&node.kind, NodeKind::Element { tag } if tag == "style") {
            for child in &node.children {
                if let Some(crate::native_dom::Node {
                    kind: NodeKind::Text(text),
                    ..
                }) = document.node(*child)
                {
                    combined.push_str(text);
                    combined.push('\n');
                }
            }
        }
    }
    combined
}

pub fn stylesheet_from_document_with_environment(
    document: &NativeDocument,
    environment: CapabilityEnvironment,
) -> Result<CompiledStyleSheet, CssError> {
    parse_stylesheet_with_environment(&document_style_source(document), environment)
}

pub fn stylesheet_from_document(document: &NativeDocument) -> Result<StyleSheet, CssError> {
    Ok(
        stylesheet_from_document_with_environment(document, CapabilityEnvironment::default())?
            .stylesheet,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_html::parse_document;

    #[test]
    fn parses_compound_selector_specificity() {
        let sheet = parse_stylesheet("p.lead#hero { font-size: 20px; }").unwrap();
        assert_eq!(sheet.rules[0].selector.specificity(), (1, 1, 1));
    }

    #[test]
    fn selector_matches_owned_attributes() {
        let document =
            parse_document("<html><body><p id=\"hero\" class=\"lead x\">Hi</p></body></html>")
                .unwrap();
        let node = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        let selector = parse_selector("p#hero.lead").unwrap();
        assert!(selector.matches(&document, node.id));
    }

    #[test]
    fn parses_is_with_case_insensitive_attribute_selectors() {
        let document =
            parse_document("<html><body><img sizes=\"AUTO, 100vw\"></body></html>").unwrap();
        let image = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "img"))
            .unwrap();
        let selector = parse_selector("img:is([sizes=auto i],[sizes^=\"auto,\" i])").unwrap();

        assert!(selector.matches(&document, image.id));
        assert_eq!(selector.specificity(), (0, 1, 1));
    }

    #[test]
    fn root_selector_matches_only_document_element() {
        let document =
            parse_document("<html><body><div id=\"child\"></div></body></html>").unwrap();
        let html = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "html"))
            .unwrap();
        let div = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "div"))
            .unwrap();
        let selector = parse_selector(":root").unwrap();

        assert!(selector.matches(&document, html.id));
        assert!(!selector.matches(&document, div.id));
        assert_eq!(selector.specificity(), (0, 1, 0));
    }

    #[test]
    fn descendant_relation_matches_observed_root_class_selector() {
        let document = parse_document(
            "<html><body><section><div class=\"has-very-light-gray-background-color\">X</div></section></body></html>",
        )
        .unwrap();
        let target = document
            .nodes()
            .iter()
            .find(|node| {
                matches!(&node.kind, NodeKind::Element { tag } if tag == "div")
                    && document.attribute(node.id, "class")
                        == Some("has-very-light-gray-background-color")
            })
            .unwrap();
        let compiled = parse_stylesheet_with_environment(
            ":root .has-very-light-gray-background-color { font-size: 20px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        let selector = &compiled.stylesheet.rules[0].selector;

        assert!(selector.matches(&document, target.id));
        assert_eq!(selector.specificity(), (0, 2, 0));
        assert!(compiled
            .translations
            .iter()
            .any(|receipt| receipt.translation_id == "css.selector-descendant.v1"));
    }

    #[test]
    fn descendant_relation_requires_matching_ancestor() {
        let document = parse_document(
            "<html><body><section><p class=\"lead\">X</p></section></body></html>",
        )
        .unwrap();
        let target = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        let selector = parse_selector("article .lead").unwrap();
        assert!(!selector.matches(&document, target.id));
    }

    #[test]
    fn unsupported_pseudo_class_still_fails_closed() {
        assert!(matches!(
            parse_selector("img:not(.x)"),
            Err(CssError::UnsupportedSelector(_))
        ));
    }

    #[test]
    fn unsupported_child_and_sibling_combinators_fail_closed() {
        for selector in ["main > p", "main + p", "main ~ p"] {
            let css = format!("{selector} {{ font-size: 20px; }}");
            assert!(matches!(
                parse_stylesheet(&css),
                Err(CssError::UnsupportedSelector(_))
            ));
        }
    }

    #[test]
    fn strips_css_comments_before_selector_parsing() {
        let sheet =
            parse_stylesheet("/*# sourceURL=inline-css */ p.lead { font-size: 20px; }").unwrap();
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selector.tag.as_deref(), Some("p"));
    }

    #[test]
    fn unterminated_css_comment_fails_closed() {
        assert!(matches!(
            parse_stylesheet("/* broken p { font-size: 20px; }"),
            Err(CssError::MalformedRule)
        ));
    }

    #[test]
    fn unknown_property_is_ignored_not_invented() {
        let declarations = parse_declarations("future-property: 1; font-size: 18px;").unwrap();
        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0].property, "font-size");
    }

    #[test]
    fn parses_custom_property_reference() {
        let declarations =
            parse_declarations("--text-size: 22px; font-size: var(--text-size);").unwrap();
        assert_eq!(declarations.len(), 2);
        assert_eq!(declarations[0].property, "--text-size");
        assert!(matches!(declarations[0].value, CssValue::Raw(_)));
        assert!(matches!(declarations[1].value, CssValue::Var(_)));
    }

    #[test]
    fn parses_bounded_flex_and_grid_declarations() {
        let flex = parse_declarations("display: flex; flex-direction: column; gap: 12px;").unwrap();
        assert!(matches!(flex[0].value, CssValue::Display(ref value) if value == "flex"));
        assert!(matches!(flex[1].value, CssValue::Keyword(ref value) if value == "column"));
        assert!(matches!(flex[2].value, CssValue::Px(value) if value == 12.0));

        let grid =
            parse_declarations("display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px;")
                .unwrap();
        assert!(matches!(grid[1].value, CssValue::GridColumns(3)));
    }

    #[test]
    fn rejects_unbounded_grid_track_syntax() {
        assert!(matches!(
            parse_declarations("grid-template-columns: 100px 1fr;"),
            Err(CssError::InvalidValue { property, .. }) if property == "grid-template-columns"
        ));
    }

    #[test]
    fn media_resolution_is_lowered_before_selector_parsing() {
        let compiled = parse_stylesheet_with_environment(
            "@media (min-resolution:192dpi) { p { font-size: 20px; } } h1 { font-size: 24px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert_eq!(compiled.stylesheet.rules.len(), 1);
        assert_eq!(
            compiled.stylesheet.rules[0].selector.tag.as_deref(),
            Some("h1")
        );
        assert_eq!(compiled.translations.len(), 1);
    }

    #[test]
    fn unsupported_media_query_remains_fail_closed() {
        let result =
            parse_stylesheet("@media (prefers-color-scheme:dark) { p { font-size: 20px; } }");
        assert!(matches!(result, Err(CssError::CapabilityCompiler(_))));
    }

    #[test]
    fn extracts_style_elements_from_native_document() {
        let document = parse_document(
            "<html><head><style>p.lead { font-size: 22px; }</style></head><body><p class=\"lead\">X</p></body></html>",
        )
        .unwrap();
        let sheet = stylesheet_from_document(&document).unwrap();
        assert_eq!(sheet.rules.len(), 1);
    }
}
