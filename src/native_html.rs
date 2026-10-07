//! AWEF-owned initial HTML tree builder.
//!
//! N1 supports a deliberately bounded but real structural subset. Syntax we do
//! not yet model is rejected rather than approximated.

use crate::native_dom::{Attribute, NativeDocument, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlParseError {
    UnsupportedMarkup(String),
    UnsupportedEntity(String),
    DuplicateAttribute(String),
    MalformedMarkup,
    UnbalancedTag { expected: String, found: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTag {
    name: String,
    closing: bool,
    self_closing: bool,
    attributes: Vec<Attribute>,
}

const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const RAW_TEXT_ELEMENTS: &[&str] = &["script", "style"];

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ if entity.starts_with("#x") || entity.starts_with("#X") => {
            u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32)
        }
        _ if entity.starts_with('#') => entity[1..].parse::<u32>().ok().and_then(char::from_u32),
        _ => None,
    }
}

fn decode_entities(input: &str) -> Result<String, HtmlParseError> {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while cursor < input.len() {
        let rest = &input[cursor..];
        let Some(amp_rel) = rest.find('&') else {
            output.push_str(rest);
            break;
        };

        output.push_str(&rest[..amp_rel]);
        let amp = cursor + amp_rel;
        let after_amp = &input[amp + 1..];

        let Some(semi_rel) = after_amp.find(';') else {
            output.push('&');
            cursor = amp + 1;
            continue;
        };

        let entity = &after_amp[..semi_rel];
        if entity.is_empty() || entity.chars().any(char::is_whitespace) {
            output.push('&');
            cursor = amp + 1;
            continue;
        }

        let Some(decoded) = decode_entity(entity) else {
            return Err(HtmlParseError::UnsupportedEntity(entity.into()));
        };

        output.push(decoded);
        cursor = amp + semi_rel + 2;
    }

    Ok(output)
}

fn parse_tag(raw: &str) -> Result<ParsedTag, HtmlParseError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(HtmlParseError::MalformedMarkup);
    }

    let closing = raw.starts_with('/');
    let mut body = raw.trim_start_matches('/').trim();
    let self_closing = !closing && body.ends_with('/');
    if self_closing {
        body = body[..body.len() - 1].trim_end();
    }

    let tag_end = body.find(char::is_whitespace).unwrap_or(body.len());
    let tag = body[..tag_end].trim();
    if tag.is_empty() {
        return Err(HtmlParseError::MalformedMarkup);
    }

    if closing {
        if body[tag_end..].trim().is_empty() {
            return Ok(ParsedTag {
                name: tag.to_ascii_lowercase(),
                closing: true,
                self_closing: false,
                attributes: Vec::new(),
            });
        }
        return Err(HtmlParseError::MalformedMarkup);
    }

    let mut attributes = Vec::new();
    let mut rest = body[tag_end..].trim();

    while !rest.is_empty() {
        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(rest.len());
        let name = rest[..name_end].trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err(HtmlParseError::MalformedMarkup);
        }

        if attributes
            .iter()
            .any(|attribute: &Attribute| attribute.name == name)
        {
            return Err(HtmlParseError::DuplicateAttribute(name));
        }

        rest = rest[name_end..].trim_start();
        let mut value = String::new();

        if let Some(after_eq) = rest.strip_prefix('=') {
            rest = after_eq.trim_start();
            let Some(first) = rest.chars().next() else {
                return Err(HtmlParseError::MalformedMarkup);
            };

            if first == '"' || first == '\'' {
                let quote = first;
                rest = &rest[first.len_utf8()..];
                let Some(end) = rest.find(quote) else {
                    return Err(HtmlParseError::MalformedMarkup);
                };
                value = decode_entities(&rest[..end])?;
                rest = rest[end + quote.len_utf8()..].trim_start();
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                value = decode_entities(&rest[..end])?;
                rest = rest[end..].trim_start();
            }
        }

        attributes.push(Attribute { name, value });
    }

    Ok(ParsedTag {
        name: tag.to_ascii_lowercase(),
        closing: false,
        self_closing,
        attributes,
    })
}

fn find_raw_text_end(input: &str, tag: &str) -> Option<(usize, usize)> {
    let lower = input.to_ascii_lowercase();
    let needle = format!("</{tag}");
    let mut search_from = 0usize;

    while search_from < lower.len() {
        let relative = lower[search_from..].find(&needle)?;
        let start = search_from + relative;
        let after_name = start + needle.len();
        let tail = &lower[after_name..];
        let close_relative = tail.find('>')?;
        if tail[..close_relative].trim().is_empty() {
            return Some((start, after_name + close_relative + 1));
        }
        search_from = after_name;
    }

    None
}

fn append_raw_text_if_present(
    document: &mut NativeDocument,
    parent: usize,
    raw: &str,
) -> Result<(), HtmlParseError> {
    if raw.is_empty() {
        return Ok(());
    }
    document
        .append_text(parent, raw.to_string())
        .ok_or(HtmlParseError::MalformedMarkup)?;
    Ok(())
}

pub fn parse_document(input: &str) -> Result<NativeDocument, HtmlParseError> {
    let mut document = NativeDocument::new();
    let mut stack = vec![document.root()];
    let mut cursor = 0usize;

    while cursor < input.len() {
        let rest = &input[cursor..];
        let Some(open_rel) = rest.find('<') else {
            append_text_if_present(&mut document, &stack, rest)?;
            break;
        };

        if open_rel > 0 {
            append_text_if_present(&mut document, &stack, &rest[..open_rel])?;
        }

        let open = cursor + open_rel;
        let after_open = &input[open..];

        if after_open.starts_with("<!--") {
            let Some(end_rel) = after_open.find("-->") else {
                return Err(HtmlParseError::MalformedMarkup);
            };
            cursor = open + end_rel + 3;
            continue;
        }

        let Some(close_rel) = after_open.find('>') else {
            return Err(HtmlParseError::MalformedMarkup);
        };
        let raw = after_open[1..close_rel].trim();
        cursor = open + close_rel + 1;

        if raw.to_ascii_lowercase().starts_with("!doctype") {
            continue;
        }

        let parsed = parse_tag(raw)?;

        if parsed.closing {
            if stack.len() <= 1 {
                return Err(HtmlParseError::MalformedMarkup);
            }

            let current = *stack.last().unwrap();
            let expected = match &document.node(current).unwrap().kind {
                NodeKind::Element { tag } => tag.clone(),
                _ => return Err(HtmlParseError::MalformedMarkup),
            };

            if expected != parsed.name {
                return Err(HtmlParseError::UnbalancedTag {
                    expected,
                    found: parsed.name,
                });
            }

            stack.pop();
        } else {
            let parent = *stack.last().unwrap();
            let tag_name = parsed.name.clone();
            let is_void = VOID_ELEMENTS.contains(&tag_name.as_str());
            let is_raw_text = RAW_TEXT_ELEMENTS.contains(&tag_name.as_str());
            let id = document
                .append_element_with_attributes(parent, parsed.name, parsed.attributes)
                .ok_or(HtmlParseError::MalformedMarkup)?;

            if parsed.self_closing || is_void {
                continue;
            }

            if is_raw_text {
                let remaining = &input[cursor..];
                let Some((content_end, closing_end)) = find_raw_text_end(remaining, &tag_name) else {
                    return Err(HtmlParseError::UnbalancedTag {
                        expected: tag_name,
                        found: "<eof>".into(),
                    });
                };
                append_raw_text_if_present(&mut document, id, &remaining[..content_end])?;
                cursor += closing_end;
                continue;
            }

            stack.push(id);
        }
    }

    if stack.len() != 1 {
        let current = *stack.last().unwrap();
        let expected = match &document.node(current).unwrap().kind {
            NodeKind::Element { tag } => tag.clone(),
            _ => "<unknown>".into(),
        };
        return Err(HtmlParseError::UnbalancedTag {
            expected,
            found: "<eof>".into(),
        });
    }

    Ok(document)
}

fn append_text_if_present(
    document: &mut NativeDocument,
    stack: &[usize],
    raw: &str,
) -> Result<(), HtmlParseError> {
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Ok(());
    }

    let text = decode_entities(&normalized)?;
    document
        .append_text(*stack.last().unwrap(), text)
        .ok_or(HtmlParseError::MalformedMarkup)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_case_insensitive_tag_names() {
        let document = parse_document("<HTML><BODY><P>Hello</P></BODY></HTML>").unwrap();
        assert_eq!(document.nodes().len(), 5);
    }

    #[test]
    fn parses_attributes_into_owned_dom() {
        let document =
            parse_document("<html><body><p class=\"lead\" data-x=1>Hello</p></body></html>")
                .unwrap();
        let paragraph = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        assert_eq!(document.attribute(paragraph.id, "class"), Some("lead"));
        assert_eq!(document.attribute(paragraph.id, "data-x"), Some("1"));
    }

    #[test]
    fn decodes_entities_in_text_and_attributes() {
        let document =
            parse_document("<html><body><p title=\"A &amp; B\">A &lt; B</p></body></html>")
                .unwrap();
        let paragraph = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        assert_eq!(document.attribute(paragraph.id, "title"), Some("A & B"));
        assert!(
            document
                .nodes()
                .iter()
                .any(|node| matches!(&node.kind, NodeKind::Text(text) if text == "A < B"))
        );
    }

    #[test]
    fn raw_text_elements_do_not_parse_embedded_markup() {
        let document = parse_document(
            r#"<html><head><script>const x = "<div>"; if (a < b) { c(); }</script><style>.x::before { content: "<"; }</style></head><body><p>ok</p></body></html>"#,
        )
        .unwrap();

        let script = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "script"))
            .unwrap();
        let script_text = document
            .node(script.children[0])
            .and_then(|node| match &node.kind {
                NodeKind::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .unwrap();
        assert!(script_text.contains("<div>"));
        assert!(script_text.contains("a < b"));

        let style = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "style"))
            .unwrap();
        assert_eq!(style.children.len(), 1);
    }

    #[test]
    fn raw_text_end_tag_matching_is_case_insensitive() {
        let document =
            parse_document("<html><head><script>let x = 1;</ScRiPt></head><body></body></html>")
                .unwrap();
        assert!(document.nodes().iter().any(
            |node| matches!(&node.kind, NodeKind::Element { tag } if tag == "script")
        ));
    }

    #[test]
    fn accepts_void_elements_without_artificial_end_tags() {
        let document =
            parse_document("<html><body><p>Hello<br>World</p><hr></body></html>").unwrap();
        assert!(
            document
                .nodes()
                .iter()
                .any(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "br"))
        );
        assert!(
            document
                .nodes()
                .iter()
                .any(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "hr"))
        );
    }

    #[test]
    fn ignores_comments_without_creating_dom_nodes() {
        let document =
            parse_document("<html><body><!-- internal --><p>Hello</p></body></html>").unwrap();
        assert_eq!(document.nodes().len(), 5);
    }

    #[test]
    fn rejects_duplicate_attributes_until_html_duplicate_policy_is_modeled() {
        let result = parse_document("<html><body><p class=\"a\" class=\"b\">x</p></body></html>");
        assert!(matches!(result, Err(HtmlParseError::DuplicateAttribute(_))));
    }

    #[test]
    fn reports_unbalanced_tag_identity() {
        let result = parse_document("<html><body><p>Hello</body></html>");
        assert_eq!(
            result,
            Err(HtmlParseError::UnbalancedTag {
                expected: "p".into(),
                found: "body".into(),
            })
        );
    }
}
