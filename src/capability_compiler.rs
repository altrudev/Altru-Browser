//! Deterministic capability translation into ACIR and native-ready source.
//!
//! The compiler does not invent semantics. Unsupported syntax fails closed.

use crate::acir::{
    CapabilityEnvironment, CapabilityNode, EnvironmentPredicate, TranslationDecision,
    TranslationReceipt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityCompileError {
    UnsupportedConditional(String),
    MalformedConditional(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledCapabilitySource {
    pub lowered_source: String,
    pub acir: Vec<CapabilityNode>,
    pub receipts: Vec<TranslationReceipt>,
}

pub fn compile_css_capabilities(
    input: &str,
    environment: &CapabilityEnvironment,
) -> Result<CompiledCapabilitySource, CapabilityCompileError> {
    let mut acir = Vec::new();
    let mut receipts = Vec::new();
    let lowered_source =
        lower_css_conditionals(input, environment, &mut acir, &mut receipts)?;

    Ok(CompiledCapabilitySource {
        lowered_source,
        acir,
        receipts,
    })
}

fn lower_css_conditionals(
    input: &str,
    environment: &CapabilityEnvironment,
    acir: &mut Vec<CapabilityNode>,
    receipts: &mut Vec<TranslationReceipt>,
) -> Result<String, CapabilityCompileError> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut cursor = 0usize;
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut escaped = false;

    while cursor < bytes.len() {
        let byte = bytes[cursor];

        if let Some(active_quote) = quote {
            output.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }

        if byte == b'\'' || byte == b'"' {
            quote = Some(byte);
            output.push(byte);
            cursor += 1;
            continue;
        }

        if depth == 0 && starts_media_at(bytes, cursor) {
            let open = find_header_open_brace(input, cursor)?;
            let header = input[cursor..open].trim();
            let predicate = parse_media_predicate(header)?;
            let close = find_matching_brace(input, open)?;
            let body = &input[open + 1..close];
            let source_construct = header.to_string();

            acir.push(CapabilityNode::Conditional {
                predicate: predicate.clone(),
                source_construct: source_construct.clone(),
                body: body.to_string(),
            });

            let include = predicate.evaluate(environment);
            receipts.push(TranslationReceipt {
                source_construct,
                target_primitive: "ACIR::Conditional(EnvironmentPredicate)".into(),
                decision: if include {
                    TranslationDecision::Included
                } else {
                    TranslationDecision::Excluded
                },
                exact: true,
            });

            if include {
                let lowered_body =
                    lower_css_conditionals(body, environment, acir, receipts)?;
                output.extend_from_slice(lowered_body.as_bytes());
            }

            cursor = close + 1;
            continue;
        }

        match byte {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        output.push(byte);
        cursor += 1;
    }

    String::from_utf8(output)
        .map_err(|_| CapabilityCompileError::MalformedConditional("invalid UTF-8".into()))
}

fn starts_media_at(bytes: &[u8], cursor: usize) -> bool {
    let needle = b"@media";
    let Some(candidate) = bytes.get(cursor..cursor.saturating_add(needle.len())) else {
        return false;
    };
    if candidate != needle {
        return false;
    }
    bytes
        .get(cursor + needle.len())
        .is_none_or(|byte| byte.is_ascii_whitespace() || *byte == b'(')
}

fn find_header_open_brace(
    input: &str,
    start: usize,
) -> Result<usize, CapabilityCompileError> {
    let bytes = input.as_bytes();
    let mut cursor = start;
    let mut quote: Option<u8> = None;
    let mut escaped = false;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
        } else if byte == b'\'' || byte == b'"' {
            quote = Some(byte);
        } else if byte == b'{' {
            return Ok(cursor);
        }
        cursor += 1;
    }

    Err(CapabilityCompileError::MalformedConditional(
        input[start..].trim().into(),
    ))
}

fn find_matching_brace(
    input: &str,
    open: usize,
) -> Result<usize, CapabilityCompileError> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut cursor = open;
    let mut quote: Option<u8> = None;
    let mut escaped = false;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
        } else if byte == b'\'' || byte == b'"' {
            quote = Some(byte);
        } else if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            if depth == 0 {
                break;
            }
            depth -= 1;
            if depth == 0 {
                return Ok(cursor);
            }
        }
        cursor += 1;
    }

    Err(CapabilityCompileError::MalformedConditional(
        input[open..].trim().into(),
    ))
}

fn parse_media_predicate(
    header: &str,
) -> Result<EnvironmentPredicate, CapabilityCompileError> {
    let condition = header
        .strip_prefix("@media")
        .ok_or_else(|| CapabilityCompileError::MalformedConditional(header.into()))?
        .trim();

    let inner = condition
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| CapabilityCompileError::UnsupportedConditional(header.into()))?
        .trim();

    let (feature, raw_value) = inner
        .split_once(':')
        .ok_or_else(|| CapabilityCompileError::UnsupportedConditional(header.into()))?;

    if feature.trim() != "min-resolution" {
        return Err(CapabilityCompileError::UnsupportedConditional(
            header.into(),
        ));
    }

    let raw_value = raw_value.trim();
    let dpi = raw_value
        .strip_suffix("dpi")
        .ok_or_else(|| CapabilityCompileError::UnsupportedConditional(header.into()))?
        .trim()
        .parse::<u32>()
        .map_err(|_| CapabilityCompileError::UnsupportedConditional(header.into()))?;

    if dpi == 0 {
        return Err(CapabilityCompileError::UnsupportedConditional(
            header.into(),
        ));
    }

    Ok(EnvironmentPredicate::MinResolutionDpi(dpi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_min_resolution_media_query_through_acir() {
        let source = "p { font-size: 18px; } @media (min-resolution:192dpi) { p { font-size: 22px; } }";
        let compiled =
            compile_css_capabilities(source, &CapabilityEnvironment { resolution_dpi: 96 })
                .unwrap();

        assert!(compiled.lowered_source.contains("font-size: 18px"));
        assert!(!compiled.lowered_source.contains("font-size: 22px"));
        assert_eq!(compiled.acir.len(), 1);
        assert_eq!(compiled.receipts.len(), 1);
        assert_eq!(
            compiled.receipts[0].decision,
            TranslationDecision::Excluded
        );
        assert!(compiled.receipts[0].exact);
    }

    #[test]
    fn includes_media_body_when_environment_satisfies_predicate() {
        let source = "@media (min-resolution: 192dpi) { p { font-size: 22px; } }";
        let compiled =
            compile_css_capabilities(source, &CapabilityEnvironment { resolution_dpi: 192 })
                .unwrap();

        assert!(compiled.lowered_source.contains("font-size: 22px"));
        assert_eq!(
            compiled.receipts[0].decision,
            TranslationDecision::Included
        );
    }

    #[test]
    fn unsupported_media_condition_fails_closed() {
        let result = compile_css_capabilities(
            "@media (prefers-color-scheme: dark) { p { font-size: 22px; } }",
            &CapabilityEnvironment::default(),
        );

        assert!(matches!(
            result,
            Err(CapabilityCompileError::UnsupportedConditional(_))
        ));
    }
}
