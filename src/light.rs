use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightError {
    UnsupportedTag(String),
    AttributesNotAllowed(String),
    UnsupportedEntity(String),
    MalformedTag,
    UnbalancedTag { expected: String, found: String },
    UnclosedTag(String),
}

impl fmt::Display for LightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTag(tag) => write!(f, "unsupported tag: {tag}"),
            Self::AttributesNotAllowed(tag) => {
                write!(f, "attributes not allowed on light-plane tag: {tag}")
            }
            Self::UnsupportedEntity(entity) => write!(f, "unsupported entity: &{entity};"),
            Self::MalformedTag => write!(f, "malformed tag"),
            Self::UnbalancedTag { expected, found } => {
                write!(
                    f,
                    "unbalanced tag: expected </{expected}> but found </{found}>"
                )
            }
            Self::UnclosedTag(tag) => write!(f, "unclosed tag: <{tag}>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayCommand {
    Start(String),
    End(String),
    Text(String),
    LineBreak,
    Rule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightDocument {
    pub commands: Vec<DisplayCommand>,
}

const ALLOWED_CONTAINER_TAGS: &[&str] = &[
    "html",
    "body",
    "main",
    "article",
    "section",
    "header",
    "footer",
    "nav",
    "div",
    "p",
    "blockquote",
    "ul",
    "ol",
    "li",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "strong",
    "b",
    "em",
    "i",
    "code",
    "pre",
    "span",
];

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

fn decode_entities(text: &str) -> Result<String, LightError> {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;

    while cursor < text.len() {
        let rest = &text[cursor..];
        let Some(amp_rel) = rest.find('&') else {
            output.push_str(rest);
            break;
        };

        output.push_str(&rest[..amp_rel]);
        let amp = cursor + amp_rel;
        let after_amp = &text[amp + 1..];

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
            return Err(LightError::UnsupportedEntity(entity.into()));
        };
        output.push(decoded);
        cursor = amp + 1 + semi_rel + 1;
    }

    Ok(output)
}

fn normalize_text(text: &str) -> Result<String, LightError> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    decode_entities(&normalized)
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn parse_bounded_html(input: &str) -> Result<LightDocument, LightError> {
    let mut commands = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut cursor = 0usize;

    while cursor < input.len() {
        let rest = &input[cursor..];
        let Some(open_rel) = rest.find('<') else {
            let text = normalize_text(rest)?;
            if !text.is_empty() {
                commands.push(DisplayCommand::Text(text));
            }
            break;
        };

        if open_rel > 0 {
            let text = normalize_text(&rest[..open_rel])?;
            if !text.is_empty() {
                commands.push(DisplayCommand::Text(text));
            }
        }

        let open = cursor + open_rel;
        let after_open = &input[open..];

        if after_open.starts_with("<!--") {
            let Some(end_rel) = after_open.find("-->") else {
                return Err(LightError::MalformedTag);
            };
            cursor = open + end_rel + 3;
            continue;
        }

        let Some(close_rel) = after_open.find('>') else {
            return Err(LightError::MalformedTag);
        };
        let raw = after_open[1..close_rel].trim();
        cursor = open + close_rel + 1;

        if raw.is_empty() {
            return Err(LightError::MalformedTag);
        }

        if raw.to_ascii_lowercase().starts_with("!doctype") {
            continue;
        }

        let is_closing = raw.starts_with('/');
        let inner = raw
            .trim_start_matches('/')
            .trim()
            .trim_end_matches('/')
            .trim();
        let mut parts = inner.split_whitespace();
        let Some(tag_raw) = parts.next() else {
            return Err(LightError::MalformedTag);
        };
        let tag = tag_raw.to_ascii_lowercase();

        if parts.next().is_some() {
            return Err(LightError::AttributesNotAllowed(tag));
        }

        if tag == "br" {
            if is_closing {
                return Err(LightError::MalformedTag);
            }
            commands.push(DisplayCommand::LineBreak);
            continue;
        }

        if tag == "hr" {
            if is_closing {
                return Err(LightError::MalformedTag);
            }
            commands.push(DisplayCommand::Rule);
            continue;
        }

        if !ALLOWED_CONTAINER_TAGS.contains(&tag.as_str()) {
            return Err(LightError::UnsupportedTag(tag));
        }

        if is_closing {
            let Some(expected) = stack.pop() else {
                return Err(LightError::UnbalancedTag {
                    expected: "<none>".into(),
                    found: tag,
                });
            };
            if expected != tag {
                return Err(LightError::UnbalancedTag {
                    expected,
                    found: tag,
                });
            }
            commands.push(DisplayCommand::End(tag));
        } else {
            stack.push(tag.clone());
            commands.push(DisplayCommand::Start(tag));
        }
    }

    if let Some(tag) = stack.pop() {
        return Err(LightError::UnclosedTag(tag));
    }

    Ok(LightDocument { commands })
}

fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "body"
            | "main"
            | "article"
            | "section"
            | "header"
            | "footer"
            | "nav"
            | "div"
            | "p"
            | "blockquote"
            | "ul"
            | "ol"
            | "li"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "pre"
    )
}

fn font_size(stack: &[String]) -> u32 {
    for tag in stack.iter().rev() {
        return match tag.as_str() {
            "h1" => 32,
            "h2" => 28,
            "h3" => 24,
            "h4" => 21,
            "h5" => 19,
            "h6" => 17,
            _ => continue,
        };
    }
    16
}

pub fn render_svg(document: &LightDocument, width: u32) -> String {
    let width = width.max(320);
    let mut y = 32u32;
    let mut stack: Vec<String> = Vec::new();
    let mut body = String::new();

    for command in &document.commands {
        match command {
            DisplayCommand::Start(tag) => {
                if is_block(tag) && !body.is_empty() {
                    y += 8;
                }
                stack.push(tag.clone());
            }
            DisplayCommand::End(tag) => {
                if let Some(position) = stack.iter().rposition(|item| item == tag) {
                    stack.truncate(position);
                }
                if is_block(tag) {
                    y += 10;
                }
            }
            DisplayCommand::Text(text) => {
                let size = font_size(&stack);
                let bold = stack.iter().any(|tag| {
                    matches!(
                        tag.as_str(),
                        "strong" | "b" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                    )
                });
                let italic = stack.iter().any(|tag| matches!(tag.as_str(), "em" | "i"));
                let mono = stack
                    .iter()
                    .any(|tag| matches!(tag.as_str(), "code" | "pre"));
                let family = if mono { "monospace" } else { "sans-serif" };
                body.push_str(&format!(
                    "<text x=\"24\" y=\"{y}\" font-family=\"{family}\" font-size=\"{size}\" font-weight=\"{}\" font-style=\"{}\">{}</text>",
                    if bold { "700" } else { "400" },
                    if italic { "italic" } else { "normal" },
                    escape_xml(text)
                ));
                y += size + 8;
            }
            DisplayCommand::LineBreak => {
                y += 18;
            }
            DisplayCommand::Rule => {
                body.push_str(&format!(
                    "<line x1=\"24\" x2=\"{}\" y1=\"{y}\" y2=\"{y}\" stroke=\"currentColor\" stroke-width=\"1\" />",
                    width.saturating_sub(24)
                ));
                y += 16;
            }
        }
    }

    let height = y.max(96) + 24;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/><g fill=\"#111\" color=\"#888\">{body}</g></svg>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_renders_static_article() {
        let document = parse_bounded_html(
            "<!doctype html><html><body><article><h1>Hello</h1><p>World</p></article></body></html>",
        )
        .unwrap();
        let svg = render_svg(&document, 800);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Hello</text>"));
        assert!(svg.contains(">World</text>"));
    }

    #[test]
    fn decodes_supported_entities_before_svg_escaping() {
        let document =
            parse_bounded_html("<html><body><p>A &amp; B &#x3C; C</p></body></html>").unwrap();
        let svg = render_svg(&document, 800);
        assert!(svg.contains(">A &amp; B &lt; C</text>"));
        assert!(!svg.contains("&amp;amp;"));
    }

    #[test]
    fn rejects_unknown_entities() {
        let error = parse_bounded_html("<html><body><p>&definitelynotanentity;</p></body></html>")
            .unwrap_err();
        assert!(matches!(error, LightError::UnsupportedEntity(_)));
    }

    #[test]
    fn deterministic_render_is_byte_identical() {
        let input = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let document_a = parse_bounded_html(input).unwrap();
        let document_b = parse_bounded_html(input).unwrap();
        assert_eq!(render_svg(&document_a, 800), render_svg(&document_b, 800));
    }

    #[test]
    fn rejects_attributes_in_phase1_subset() {
        let error =
            parse_bounded_html("<html><body><p class=\"x\">Hello</p></body></html>").unwrap_err();
        assert!(matches!(error, LightError::AttributesNotAllowed(_)));
    }

    #[test]
    fn rejects_unknown_tags() {
        let error =
            parse_bounded_html("<html><body><marquee>Hello</marquee></body></html>").unwrap_err();
        assert!(matches!(error, LightError::UnsupportedTag(_)));
    }

    #[test]
    fn rejects_unbalanced_markup() {
        let error = parse_bounded_html("<html><body><p>Hello</body></html>").unwrap_err();
        assert!(matches!(error, LightError::UnbalancedTag { .. }));
    }
}
