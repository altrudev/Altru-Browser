//! Bounded semantic translation into ACIR.
//!
//! The compiler does not invent behavior. It recognizes a small, explicit
//! subset and lowers it into deterministic ACIR conditions.

use crate::acir::{
    Comparison, Condition, EnvironmentMetric, EnvironmentPredicate, EnvironmentSnapshot,
    TranslationRecord,
};

pub const CSS_MEDIA_TRANSLATION: TranslationRecord = TranslationRecord {
    translator_id: "acir.css.media.environment.v2",
    source_domain: "css-media-query",
    target_domain: "acir-condition",
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
    pub condition: Condition,
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

fn parse_width_px(value: &str, comparison: Comparison) -> Result<u32, CapabilityCompileError> {
    let value = value.trim().to_ascii_lowercase();
    let Some(raw) = value.strip_suffix("px") else {
        return Err(CapabilityCompileError::UnsupportedMediaCondition(value));
    };
    let px = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| CapabilityCompileError::InvalidValue(value.clone()))?;
    if !px.is_finite() || px < 0.0 {
        return Err(CapabilityCompileError::InvalidValue(value));
    }

    // The native environment currently reports integral CSS pixels.
    // Rounding toward the truth boundary preserves comparison semantics.
    let threshold = match comparison {
        Comparison::AtLeast => px.ceil(),
        Comparison::AtMost => px.floor(),
    };
    if threshold > u32::MAX as f64 {
        return Err(CapabilityCompileError::InvalidValue(value));
    }
    Ok(threshold as u32)
}

fn split_top_level_and(input: &str) -> Result<Vec<&str>, CapabilityCompileError> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let bytes = input.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                if depth == 0 {
                    return Err(CapabilityCompileError::MalformedConditional(input.into()));
                }
                depth -= 1;
            }
            _ => {}
        }

        if depth == 0 && input[index..].starts_with(" and ") {
            let part = input[start..index].trim();
            if part.is_empty() {
                return Err(CapabilityCompileError::MalformedConditional(input.into()));
            }
            parts.push(part);
            index += " and ".len();
            start = index;
            continue;
        }
        index += 1;
    }

    if depth != 0 {
        return Err(CapabilityCompileError::MalformedConditional(input.into()));
    }

    let tail = input[start..].trim();
    if tail.is_empty() {
        return Err(CapabilityCompileError::MalformedConditional(input.into()));
    }
    parts.push(tail);
    Ok(parts)
}

fn compile_media_feature(input: &str) -> Result<Condition, CapabilityCompileError> {
    let Some(inner) = input
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return Err(CapabilityCompileError::UnsupportedMediaCondition(
            input.trim().into(),
        ));
    };
    let inner = inner.trim();

    if inner.eq_ignore_ascii_case("prefers-reduced-motion") {
        return Ok(Condition::PrefersReducedMotion);
    }

    let Some((feature, value)) = inner.split_once(':') else {
        return Err(CapabilityCompileError::MalformedConditional(input.into()));
    };
    let feature = feature.trim().to_ascii_lowercase();

    let predicate = match feature.as_str() {
        "min-resolution" => EnvironmentPredicate {
            metric: EnvironmentMetric::ResolutionDpi,
            comparison: Comparison::AtLeast,
            threshold: parse_resolution_dpi(value)?,
        },
        "max-resolution" => EnvironmentPredicate {
            metric: EnvironmentMetric::ResolutionDpi,
            comparison: Comparison::AtMost,
            threshold: parse_resolution_dpi(value)?,
        },
        "min-width" => EnvironmentPredicate {
            metric: EnvironmentMetric::ViewportWidthPx,
            comparison: Comparison::AtLeast,
            threshold: parse_width_px(value, Comparison::AtLeast)?,
        },
        "max-width" => EnvironmentPredicate {
            metric: EnvironmentMetric::ViewportWidthPx,
            comparison: Comparison::AtMost,
            threshold: parse_width_px(value, Comparison::AtMost)?,
        },
        "prefers-reduced-motion" => {
            let value = value.trim().to_ascii_lowercase();
            return match value.as_str() {
                "reduce" => Ok(Condition::PrefersReducedMotion),
                "no-preference" => Ok(Condition::Not(Box::new(Condition::PrefersReducedMotion))),
                _ => Err(CapabilityCompileError::UnsupportedMediaCondition(format!(
                    "{feature}:{value}"
                ))),
            };
        }
        _ => {
            return Err(CapabilityCompileError::UnsupportedMediaCondition(feature));
        }
    };

    Ok(Condition::Environment(predicate))
}

pub fn compile_css_media_condition(
    input: &str,
) -> Result<CompiledMediaCondition, CapabilityCompileError> {
    let mut input = input.trim();
    let mut negate = false;

    if let Some(rest) = input.strip_prefix("not ") {
        negate = true;
        input = rest.trim();
    }

    if let Some(rest) = input.strip_prefix("screen and ") {
        input = rest.trim();
    } else if let Some(rest) = input.strip_prefix("all and ") {
        input = rest.trim();
    }

    let terms = split_top_level_and(input)?;
    let mut conditions = terms
        .into_iter()
        .map(compile_media_feature)
        .collect::<Result<Vec<_>, _>>()?;

    let mut condition = if conditions.len() == 1 {
        conditions.remove(0)
    } else {
        Condition::All(conditions)
    };

    if negate {
        condition = Condition::Not(Box::new(condition));
    }

    Ok(CompiledMediaCondition {
        condition,
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

        if compiled.condition.evaluate(environment) {
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
        assert!(matches!(
            compiled.condition,
            Condition::Environment(EnvironmentPredicate {
                metric: EnvironmentMetric::ResolutionDpi,
                comparison: Comparison::AtLeast,
                threshold: 192,
            })
        ));
        assert!(compiled.translation.exact_for_bounded_subset);
    }

    #[test]
    fn converts_dppx_to_dpi() {
        let compiled = compile_css_media_condition("(min-resolution:2dppx)").unwrap();
        assert!(matches!(
            compiled.condition,
            Condition::Environment(EnvironmentPredicate { threshold: 192, .. })
        ));
    }

    #[test]
    fn compiles_cnet_media_query_family() {
        let queries = [
            "(min-resolution:192dpi)",
            "screen and (max-width:600px)",
            "not (prefers-reduced-motion)",
            "(min-width:782px)",
            "(max-width:781px)",
            "(min-width: 640px)",
            "(max-width: 639px)",
            "( min-width: 992px )",
            "( min-width: 768px ) and ( max-width: 991.98px )",
            "( max-width: 767.98px )",
        ];
        for query in queries {
            compile_css_media_condition(query).unwrap_or_else(|error| panic!("{query}: {error:?}"));
        }
    }

    #[test]
    fn fractional_widths_preserve_integer_environment_boundaries() {
        let min = compile_css_media_condition("(min-width:991.98px)").unwrap();
        let max = compile_css_media_condition("(max-width:991.98px)").unwrap();
        let environment = EnvironmentSnapshot {
            viewport_width_px: 992,
            resolution_dpi: 96,
            prefers_reduced_motion: false,
        };
        assert!(min.condition.evaluate(environment));
        assert!(!max.condition.evaluate(environment));
    }

    #[test]
    fn reduced_motion_negation_uses_environment_preference() {
        let compiled = compile_css_media_condition("not (prefers-reduced-motion)").unwrap();
        let normal = EnvironmentSnapshot::desktop_preview();
        let reduced = EnvironmentSnapshot {
            prefers_reduced_motion: true,
            ..normal
        };
        assert!(compiled.condition.evaluate(normal));
        assert!(!compiled.condition.evaluate(reduced));
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
        let environment = EnvironmentSnapshot::desktop_preview();
        let css = "p { font-size: 16px; } @media (min-resolution:192dpi) { .hi { font-size: 24px; } } @media (max-width:900px) { .narrow { font-size: 18px; } }";
        let lowered = lower_css_media_blocks(css, environment).unwrap();
        assert!(lowered.contains("p { font-size: 16px; }"));
        assert!(!lowered.contains(".hi"));
        assert!(lowered.contains(".narrow"));
        assert!(!lowered.contains("@media"));
    }
}
