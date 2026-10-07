//! Deterministic capability compiler.
//!
//! Known source constructs are lowered into ACIR and only transforms present in the
//! verified translation registry are available to the runtime.

use sha2::{Digest, Sha256};

use crate::capability_ir::{
    AcirComparison, AcirEnvironmentCondition, AcirEnvironmentFeature, AcirEnvironmentPredicate,
    AcirMediaType, AcirSelectorChain, AcirSelectorCombinator, CapabilityEnvironment,
};

pub const CSS_MEDIA_ENVIRONMENT_V1: &str = "css.media-environment.v1";
pub const CSS_SELECTOR_DESCENDANT_V1: &str = "css.selector-descendant.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationStatus {
    Verified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationSpec {
    pub id: &'static str,
    pub source_family: &'static str,
    pub target_semantics: &'static str,
    pub status: TranslationStatus,
}

pub const VERIFIED_TRANSLATIONS: &[TranslationSpec] = &[
    TranslationSpec {
        id: CSS_MEDIA_ENVIRONMENT_V1,
        source_family: "css-media-environment",
        target_semantics: "acir.environment-condition",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_SELECTOR_DESCENDANT_V1,
        source_family: "css-selector-descendant",
        target_semantics: "acir.selector-chain",
        status: TranslationStatus::Verified,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationDecision {
    Admitted,
    Elided,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationReceipt {
    pub translation_id: String,
    pub source_sha256: String,
    pub decision: TranslationDecision,
}

impl TranslationReceipt {
    pub fn canonical_line(&self) -> String {
        format!(
            "{}|{}|{:?}",
            self.translation_id, self.source_sha256, self.decision
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledCapabilitySource {
    pub source: String,
    pub receipts: Vec<TranslationReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSelectorCapability {
    pub chain: AcirSelectorChain,
    pub receipt: TranslationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityCompilerError {
    MalformedAtRule,
    UnsupportedMediaQuery(String),
    UnverifiedTranslation(String),
}

fn sha256(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

fn require_verified(id: &str) -> Result<(), CapabilityCompilerError> {
    if VERIFIED_TRANSLATIONS
        .iter()
        .any(|spec| spec.id == id && spec.status == TranslationStatus::Verified)
    {
        Ok(())
    } else {
        Err(CapabilityCompilerError::UnverifiedTranslation(id.into()))
    }
}

fn parse_decimal_milli(raw: &str) -> Result<u32, CapabilityCompilerError> {
    let raw = raw.trim();
    let (whole, fraction) = raw.split_once('.').unwrap_or((raw, ""));
    if whole.is_empty()
        || !whole.chars().all(|ch| ch.is_ascii_digit())
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
        || fraction.len() > 3
    {
        return Err(CapabilityCompilerError::UnsupportedMediaQuery(raw.into()));
    }
    let whole = whole
        .parse::<u32>()
        .map_err(|_| CapabilityCompilerError::UnsupportedMediaQuery(raw.into()))?;
    let mut fraction_milli = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u32>()
            .map_err(|_| CapabilityCompilerError::UnsupportedMediaQuery(raw.into()))?
    };
    for _ in fraction.len()..3 {
        fraction_milli = fraction_milli.saturating_mul(10);
    }
    whole
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(fraction_milli))
        .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(raw.into()))
}

fn parse_resolution_milli_dpi(raw: &str) -> Result<u32, CapabilityCompilerError> {
    let raw = raw.trim().to_ascii_lowercase();
    if let Some(number) = raw.strip_suffix("dpi") {
        return parse_decimal_milli(number);
    }
    if let Some(number) = raw.strip_suffix("dppx") {
        let milli_dppx = parse_decimal_milli(number)?;
        return milli_dppx
            .checked_mul(96)
            .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(raw));
    }
    Err(CapabilityCompilerError::UnsupportedMediaQuery(raw))
}

fn parse_width_milli_px(raw: &str) -> Result<u32, CapabilityCompilerError> {
    let raw = raw.trim().to_ascii_lowercase();
    let Some(number) = raw.strip_suffix("px") else {
        return Err(CapabilityCompilerError::UnsupportedMediaQuery(raw));
    };
    parse_decimal_milli(number)
}

fn parse_parenthesized_predicate(
    raw: &str,
) -> Result<AcirEnvironmentPredicate, CapabilityCompilerError> {
    let raw = raw.trim();
    let inner = raw
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(raw.into()))?
        .trim();

    if inner.eq_ignore_ascii_case("prefers-reduced-motion") {
        return Ok(AcirEnvironmentPredicate {
            feature: AcirEnvironmentFeature::PrefersReducedMotionFlag,
            comparison: AcirComparison::AtLeast,
            value: 1,
        });
    }

    let (feature, value) = inner
        .split_once(':')
        .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(inner.into()))?;
    let feature = feature.trim().to_ascii_lowercase();
    let (environment_feature, comparison, parsed_value) = match feature.as_str() {
        "min-resolution" => (
            AcirEnvironmentFeature::ResolutionMilliDpi,
            AcirComparison::AtLeast,
            parse_resolution_milli_dpi(value)?,
        ),
        "max-resolution" => (
            AcirEnvironmentFeature::ResolutionMilliDpi,
            AcirComparison::AtMost,
            parse_resolution_milli_dpi(value)?,
        ),
        "min-width" => (
            AcirEnvironmentFeature::ViewportWidthMilliPx,
            AcirComparison::AtLeast,
            parse_width_milli_px(value)?,
        ),
        "max-width" => (
            AcirEnvironmentFeature::ViewportWidthMilliPx,
            AcirComparison::AtMost,
            parse_width_milli_px(value)?,
        ),
        _ => return Err(CapabilityCompilerError::UnsupportedMediaQuery(inner.into())),
    };

    Ok(AcirEnvironmentPredicate {
        feature: environment_feature,
        comparison,
        value: parsed_value,
    })
}

fn split_top_level_and(query: &str) -> Result<Vec<&str>, CapabilityCompilerError> {
    let bytes = query.as_bytes();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut parts = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                if depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedMediaQuery(query.into()));
                }
                depth -= 1;
            }
            _ => {}
        }

        if depth == 0
            && index + 5 <= bytes.len()
            && query[index..index + 5].eq_ignore_ascii_case(" and ")
        {
            parts.push(query[start..index].trim());
            start = index + 5;
            index += 5;
            continue;
        }
        index += 1;
    }

    if depth != 0 {
        return Err(CapabilityCompilerError::UnsupportedMediaQuery(query.into()));
    }
    parts.push(query[start..].trim());
    Ok(parts)
}

fn lower_media_query(raw: &str) -> Result<AcirEnvironmentCondition, CapabilityCompilerError> {
    require_verified(CSS_MEDIA_ENVIRONMENT_V1)?;
    let query = raw.trim();
    if query.is_empty() {
        return Err(CapabilityCompilerError::UnsupportedMediaQuery(query.into()));
    }

    if let Some(predicate) = query
        .strip_prefix("not ")
        .or_else(|| query.strip_prefix("NOT "))
    {
        let predicate = parse_parenthesized_predicate(predicate)?;
        if predicate.feature != AcirEnvironmentFeature::PrefersReducedMotionFlag {
            return Err(CapabilityCompilerError::UnsupportedMediaQuery(query.into()));
        }
        return Ok(AcirEnvironmentCondition {
            media_type: None,
            predicates: vec![AcirEnvironmentPredicate {
                comparison: AcirComparison::AtMost,
                value: 0,
                ..predicate
            }],
        });
    }

    let mut media_type = None;
    let mut predicates = Vec::new();
    for part in split_top_level_and(query)? {
        if part.eq_ignore_ascii_case("screen") {
            media_type = Some(AcirMediaType::Screen);
        } else {
            predicates.push(parse_parenthesized_predicate(part)?);
        }
    }

    if media_type.is_none() && predicates.is_empty() {
        return Err(CapabilityCompilerError::UnsupportedMediaQuery(query.into()));
    }

    Ok(AcirEnvironmentCondition {
        media_type,
        predicates,
    })
}

fn matching_brace(input: &str, open: usize) -> Result<usize, CapabilityCompilerError> {
    let mut depth = 0usize;
    let mut quote = None;
    for (offset, ch) in input[open..].char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return Err(CapabilityCompilerError::MalformedAtRule);
                }
                depth -= 1;
                if depth == 0 {
                    return Ok(open + offset);
                }
            }
            _ => {}
        }
    }
    Err(CapabilityCompilerError::MalformedAtRule)
}


fn split_top_level_descendants(input: &str) -> Result<Vec<&str>, CapabilityCompilerError> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    let mut in_separator = false;

    for (index, ch) in input.char_indices() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => {
                if bracket_depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedMediaQuery(input.into()));
                }
                bracket_depth -= 1;
            }
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => {
                if paren_depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedMediaQuery(input.into()));
                }
                paren_depth -= 1;
            }
            '>' | '+' | '~' if bracket_depth == 0 && paren_depth == 0 => {
                return Err(CapabilityCompilerError::UnsupportedMediaQuery(input.into()));
            }
            ch if ch.is_whitespace() && bracket_depth == 0 && paren_depth == 0 => {
                if !in_separator {
                    let part = input[start..index].trim();
                    if !part.is_empty() {
                        parts.push(part);
                    }
                    in_separator = true;
                }
            }
            _ if in_separator && bracket_depth == 0 && paren_depth == 0 => {
                start = index;
                in_separator = false;
            }
            _ => {}
        }
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return Err(CapabilityCompilerError::UnsupportedMediaQuery(input.into()));
    }

    if !in_separator {
        let tail = input[start..].trim();
        if !tail.is_empty() {
            parts.push(tail);
        }
    }

    Ok(parts)
}

pub fn compile_selector_capability(
    input: &str,
) -> Result<Option<CompiledSelectorCapability>, CapabilityCompilerError> {
    let parts = split_top_level_descendants(input)?;
    if parts.len() < 2 {
        return Ok(None);
    }

    require_verified(CSS_SELECTOR_DESCENDANT_V1)?;

    let chain = AcirSelectorChain {
        compounds: parts.into_iter().map(str::to_string).collect(),
        combinators: vec![AcirSelectorCombinator::Descendant; parts.len() - 1],
    };
    if !chain.is_well_formed() {
        return Err(CapabilityCompilerError::MalformedAtRule);
    }

    Ok(Some(CompiledSelectorCapability {
        chain,
        receipt: TranslationReceipt {
            translation_id: CSS_SELECTOR_DESCENDANT_V1.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

pub fn compile_stylesheet_capabilities(
    input: &str,
    environment: CapabilityEnvironment,
) -> Result<CompiledCapabilitySource, CapabilityCompilerError> {
    let mut output = String::with_capacity(input.len());
    let mut receipts = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative) = input[cursor..].find("@media") {
        let media_start = cursor + relative;
        output.push_str(&input[cursor..media_start]);

        let header_start = media_start + "@media".len();
        let open_relative = input[header_start..]
            .find('{')
            .ok_or(CapabilityCompilerError::MalformedAtRule)?;
        let open = header_start + open_relative;
        let close = matching_brace(input, open)?;
        let query = input[header_start..open].trim();
        let condition = lower_media_query(query)?;
        let source_construct = &input[media_start..=close];
        let decision = if environment.evaluate_condition(&condition) {
            output.push_str(&input[open + 1..close]);
            TranslationDecision::Admitted
        } else {
            TranslationDecision::Elided
        };
        receipts.push(TranslationReceipt {
            translation_id: CSS_MEDIA_ENVIRONMENT_V1.into(),
            source_sha256: sha256(source_construct),
            decision,
        });
        cursor = close + 1;
    }

    output.push_str(&input[cursor..]);
    Ok(CompiledCapabilitySource {
        source: output,
        receipts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_cnet_media_vocabulary_lowers_through_one_acir_family() {
        let samples = [
            "( max-width: 767.98px )",
            "( min-width: 768px ) and ( max-width: 991.98px )",
            "( min-width: 992px )",
            "(max-width: 639px)",
            "(max-width:781px)",
            "(min-resolution:192dpi)",
            "(min-width: 640px)",
            "(min-width:782px)",
            "not (prefers-reduced-motion)",
            "screen and (max-width:600px)",
        ];
        for query in samples {
            let css = format!("@media {query} {{ p {{ font-size: 20px; }} }}");
            let compiled =
                compile_stylesheet_capabilities(&css, CapabilityEnvironment::desktop(800)).unwrap();
            assert_eq!(compiled.receipts.len(), 1, "{query}");
        }
    }

    #[test]
    fn width_and_resolution_conditions_admit_or_elide_deterministically() {
        let environment = CapabilityEnvironment::desktop(800);

        let narrow = compile_stylesheet_capabilities(
            "@media (max-width:600px) { p { font-size: 20px; } }",
            environment,
        )
        .unwrap();
        assert_eq!(narrow.receipts[0].decision, TranslationDecision::Elided);

        let desktop = compile_stylesheet_capabilities(
            "@media (min-width:768px) { p { font-size: 20px; } }",
            environment,
        )
        .unwrap();
        assert_eq!(desktop.receipts[0].decision, TranslationDecision::Admitted);

        let hi_dpi = compile_stylesheet_capabilities(
            "@media (min-resolution:192dpi) { p { font-size: 20px; } }",
            environment,
        )
        .unwrap();
        assert_eq!(hi_dpi.receipts[0].decision, TranslationDecision::Elided);
    }

    #[test]
    fn descendant_selector_compiles_without_splitting_attribute_spaces() {
        let compiled = compile_selector_capability(":root .card[data-mode=dark i]")
            .unwrap()
            .unwrap();

        assert_eq!(compiled.chain.compounds, vec![":root", ".card[data-mode=dark i]"]);
        assert_eq!(
            compiled.chain.combinators,
            vec![AcirSelectorCombinator::Descendant]
        );
        assert_eq!(
            compiled.receipt.translation_id,
            CSS_SELECTOR_DESCENDANT_V1
        );
    }

    #[test]
    fn explicit_child_combinator_is_not_silently_lowered() {
        assert!(compile_selector_capability(":root > .card").is_err());
    }

    #[test]
    fn unsupported_media_feature_fails_closed() {
        let result = compile_stylesheet_capabilities(
            "@media (prefers-color-scheme:dark) { p { font-size: 20px; } }",
            CapabilityEnvironment::default(),
        );
        assert!(matches!(
            result,
            Err(CapabilityCompilerError::UnsupportedMediaQuery(_))
        ));
    }
}
