//! Bounded semantic translation into ACIR.
//!
//! The compiler does not invent behavior. It recognizes a small, explicit
//! subset and lowers it into deterministic ACIR predicates.

use crate::acir::{
    Comparison, EnvironmentMetric, EnvironmentPredicate, EnvironmentSnapshot, TranslationRecord,
};

pub const CSS_MEDIA_TRANSLATION: TranslationRecord = TranslationRecord {
    translator_id: "acir.css.media.environment.v1",
    source_domain: "css-media-query",
    target_domain: "acir-environment-predicate",
    exact_for_bounded_subset: true,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityCompileError {
    MalformedConditional(String),
    UnsupportedMediaCondition(String),
    InvalidValue(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledMediaCondition {
    pub predicate: EnvironmentPredicate,
    pub translation: TranslationRecord,
}

fn parse_positive_u32(value: &str) -> Result<u32, CapabilityCompileError> {
    let parsed = value
        .trim()
        .parse::<u32>()
        .map_err(|_| CapabilityCompileError::InvalidValue(value.trim().into()))?;
    if parsed == 0 {
        return Err(CapabilityCompileError::InvalidValue(value.trim().into()));
    }
    Ok(parsed)
}

fn parse_resolution_dpi(value: &str) -> Result<u32, CapabilityCompileError> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(raw) = value.strip_suffix("dpi") {
        return parse_positive_u32(raw);
    }
    if let Some(raw) = value.strip_suffix("dppx") {
        let dppx = raw
            .trim()
            .parse::<f64>()
            .map_err(|_| CapabilityCompileError::InvalidValue(value.clone()))?;
        if !dppx.is_finite() || dppx <= 0.0 {
            return Err(CapabilityCompileError::InvalidValue(value));
        }
        return Ok((dppx * 96.0).round().max(1.0) as u32);
    }
    Err(CapabilityCompileError::UnsupportedMediaCondition(value))
}

pub fn compile_css_media_condition(
    input: &str,
) -> Result<CompiledMediaCondition, CapabilityCompileError> {
    let mut input = input.trim();
    if let Some(rest) = input.strip_prefix("screen and") {
        input = rest.trim();
    } else if let Some(rest) = input.strip_prefix("all and") {
        input = rest.trim();
    }

    let Some(inner) = input.strip_prefix('(').and_then(|v| v.strip_suffix(')')) else {
        return Err(CapabilityCompileError::UnsupportedMediaCondition(
            input.into(),
        ));
    };
    let Some((feature, value)) = inner.split_once(':') else {
        return Err(CapabilityCompileError::MalformedConditional(input.into()));
    };

    let (metric, comparison, threshold) = match feature.trim().to_ascii_lowercase().as_str() {
        "min-resolution" => (
            EnvironmentMetric::ResolutionDpi,
            Comparison::AtLeast,
            parse_resolution_dpi(value)?,
        ),
        "max-resolution" => (
            EnvironmentMetric::ResolutionDpi,
            Comparison::AtMost,
            parse_resolution_dpi(value)?,
        ),
        "min-width" => {
            let value = value.trim().to_ascii_lowercase();
            let Some(raw) = value.strip_suffix("px") else {
                return Err(CapabilityCompileError::UnsupportedMediaCondition(value));
            };
            (
                EnvironmentMetric::ViewportWidthPx,
                Comparison::AtLeast,
                parse_positive_u32(raw)?,
            )
        }
        "max-width" => {
            let value = value.trim().to_ascii_lowercase();
            let Some(raw) = value.strip_suffix("px") else {
                return Err(CapabilityCompileError::UnsupportedMediaCondition(value));
            };
            (
                EnvironmentMetric::ViewportWidthPx,
                Comparison::AtMost,
                parse_positive_u32(raw)?,
            )
        }
        _ => {
            return Err(CapabilityCompileError::UnsupportedMediaCondition(
                feature.trim().into(),
            ));
        }
    };

    Ok(CompiledMediaCondition {
        predicate: EnvironmentPredicate {
            metric,
            comparison,
            threshold,
        },
        translation: CSS_MEDIA_TRANSLATION.clone(),
    })
}

fn find_top_level_media(input: &str, from: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let mut index = from;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }

        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'@' if depth == 0 && input[index..].starts_with("@media") => {
                let after = index + "@media".len();
                if after == bytes.len()
                    || bytes[after].is_ascii_whitespace()
                    || bytes[after] == b'('
                {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn find_matching_brace(input: &str, open: usize) -> Result<usize, CapabilityCompileError> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut escaped = false;

    for index in open..bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
            continue;
        }

        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }

    Err(CapabilityCompileError::MalformedConditional(
        "unterminated @media block".into(),
    ))
}

pub fn lower_css_media_blocks(
    input: &str,
    environment: EnvironmentSnapshot,
) -> Result<String, CapabilityCompileError> {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while let Some(media_start) = find_top_level_media(input, cursor) {
        output.push_str(&input[cursor..media_start]);

        let condition_start = media_start + "@media".len();
        let Some(open_relative) = input[condition_start..].find('{') else {
            return Err(CapabilityCompileError::MalformedConditional(
                "missing @media body".into(),
            ));
        };
        let open = condition_start + open_relative;
        let close = find_matching_brace(input, open)?;
        let condition = input[condition_start..open].trim();
        let compiled = compile_css_media_condition(condition)?;

        if compiled.predicate.evaluate(environment) {
            let inner = lower_css_media_blocks(&input[open + 1..close], environment)?;
            output.push_str(&inner);
        }

        cursor = close + 1;
    }

    output.push_str(&input[cursor..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_resolution_condition_to_acir() {
        let compiled = compile_css_media_condition("(min-resolution:192dpi)").unwrap();
        assert_eq!(compiled.predicate.metric, EnvironmentMetric::ResolutionDpi);
        assert_eq!(compiled.predicate.comparison, Comparison::AtLeast);
        assert_eq!(compiled.predicate.threshold, 192);
        assert!(compiled.translation.exact_for_bounded_subset);
    }

    #[test]
    fn converts_dppx_to_dpi() {
        let compiled = compile_css_media_condition("(min-resolution:2dppx)").unwrap();
        assert_eq!(compiled.predicate.threshold, 192);
    }

    #[test]
    fn unsupported_media_feature_fails_closed() {
        assert!(matches!(
            compile_css_media_condition("(orientation:landscape)"),
            Err(CapabilityCompileError::UnsupportedMediaCondition(_))
        ));
    }

    #[test]
    fn lowers_only_matching_media_blocks() {
        let environment = EnvironmentSnapshot {
            viewport_width_px: 800,
            resolution_dpi: 96,
        };
        let css = "p { font-size: 16px; } @media (min-resolution:192dpi) { .hi { font-size: 24px; } } @media (max-width:900px) { .narrow { font-size: 18px; } }";
        let lowered = lower_css_media_blocks(css, environment).unwrap();
        assert!(lowered.contains("p { font-size: 16px; }"));
        assert!(!lowered.contains(".hi"));
        assert!(lowered.contains(".narrow"));
        assert!(!lowered.contains("@media"));
    }
}
