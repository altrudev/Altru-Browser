//! AWEF-owned N2 CSS parser and selector model.
//!
//! This is deliberately a bounded CSS subset. Unsupported selectors fail
//! closed; unknown properties follow CSS error handling and are ignored.

use crate::native_dom::{NativeDocument, NodeId, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssError {
    MalformedRule,
    UnsupportedSelector(String),
    MalformedDeclaration(String),
    InvalidValue { property: String, value: String },
    UnresolvedCustomProperty(String),
    UnsupportedLayout(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

impl Selector {
    pub fn specificity(&self) -> (u16, u16, u16) {
        (
            u16::from(self.id.is_some()),
            self.classes.len().min(u16::MAX as usize) as u16,
            u16::from(self.tag.is_some()),
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

fn parse_selector(input: &str) -> Result<Selector, CssError> {
    let input = input.trim();
    if input.is_empty()
        || input.contains(',')
        || input.contains('>')
        || input.contains('+')
        || input.contains('~')
        || input.chars().any(char::is_whitespace)
        || input.contains('[')
        || input.contains(':')
        || input.contains('*')
    {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    let mut selector = Selector {
        tag: None,
        id: None,
        classes: Vec::new(),
    };

    let mut cursor = 0usize;
    let bytes = input.as_bytes();

    if !matches!(bytes.first(), Some(b'.' | b'#')) {
        let end = input.find(['.', '#']).unwrap_or(input.len());
        let tag = &input[..end];
        if tag.is_empty() {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        selector.tag = Some(tag.to_ascii_lowercase());
        cursor = end;
    }

    while cursor < input.len() {
        let marker = input.as_bytes()[cursor];
        if marker != b'.' && marker != b'#' {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        cursor += 1;
        let rest = &input[cursor..];
        let end_rel = rest.find(['.', '#']).unwrap_or(rest.len());
        let value = &rest[..end_rel];
        if value.is_empty() {
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

    if selector.tag.is_none() && selector.id.is_none() && selector.classes.is_empty() {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    Ok(selector)
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

pub fn parse_stylesheet(input: &str) -> Result<StyleSheet, CssError> {
    let mut rules = Vec::new();
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

        let selector = parse_selector(rest[..open].trim())?;
        let declarations = parse_declarations(&rest[open + 1..close])?;
        rules.push(Rule {
            selector,
            declarations,
            order,
        });
        order += 1;
        rest = &rest[close + 1..];
    }

    Ok(StyleSheet { rules })
}

pub fn stylesheet_from_document(document: &NativeDocument) -> Result<StyleSheet, CssError> {
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
    parse_stylesheet(&combined)
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
    fn unsupported_combinator_fails_closed() {
        assert!(matches!(
            parse_stylesheet("main p { font-size: 20px; }"),
            Err(CssError::UnsupportedSelector(_))
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
    fn extracts_style_elements_from_native_document() {
        let document = parse_document(
            "<html><head><style>p.lead { font-size: 22px; }</style></head><body><p class=\"lead\">X</p></body></html>",
        )
        .unwrap();
        let sheet = stylesheet_from_document(&document).unwrap();
        assert_eq!(sheet.rules.len(), 1);
    }
}
