//! Deterministic capability compiler.
//!
//! Known source constructs are lowered into ACIR and only transforms present in the
//! verified translation registry are available to the runtime.

use sha2::{Digest, Sha256};

use crate::capability_ir::{
    AcirComparison, AcirEnvironmentCondition, AcirEnvironmentFeature, AcirEnvironmentPredicate,
    AcirInteractionPredicate, AcirLengthBasis, AcirMediaType, AcirRelativeLength,
    AcirSelectorBoolean, AcirSelectorChain, AcirSelectorRelation, AcirStructuralPredicate,
    AcirSupportCondition, CapabilityEnvironment,
};

pub const CSS_MEDIA_ENVIRONMENT_V1: &str = "css.media-environment.v1";
pub const CSS_SELECTOR_DESCENDANT_V1: &str = "css.selector-descendant.v1";
pub const CSS_SELECTOR_RELATIONS_V1: &str = "css.selector-relations.v1";
pub const CSS_SELECTOR_INTERACTION_V1: &str = "css.selector-interaction-state.v1";
pub const CSS_SELECTOR_STRUCTURAL_V1: &str = "css.selector-structural-child.v1";
pub const CSS_SELECTOR_BOOLEAN_V1: &str = "css.selector-boolean.v1";
pub const CSS_SELECTOR_LIST_V1: &str = "css.selector-list.v1";
pub const CSS_FONT_EM_V1: &str = "css.font-em.v1";
pub const CSS_LENGTH_EM_V1: &str = "css.length-em.current-font.v1";
pub const CSS_SUPPORTS_DECLARATION_V1: &str = "css.supports-declaration.v1";
pub const CSS_LENGTH_ZERO_V1: &str = "css.length-zero.v1";

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
    TranslationSpec {
        id: CSS_SELECTOR_RELATIONS_V1,
        source_family: "css-selector-relations",
        target_semantics: "acir.selector-chain",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_SELECTOR_INTERACTION_V1,
        source_family: "css-selector-interaction-state",
        target_semantics: "acir.interaction-predicate",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_SELECTOR_STRUCTURAL_V1,
        source_family: "css-selector-structural-child",
        target_semantics: "acir.structural-predicate",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_SELECTOR_BOOLEAN_V1,
        source_family: "css-selector-boolean",
        target_semantics: "acir.selector-boolean",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_SELECTOR_LIST_V1,
        source_family: "css-selector-list",
        target_semantics: "native.rule-expansion",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_FONT_EM_V1,
        source_family: "css-font-relative-length",
        target_semantics: "acir.relative-length.parent-font",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_LENGTH_EM_V1,
        source_family: "css-length-em",
        target_semantics: "acir.relative-length.current-font",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_SUPPORTS_DECLARATION_V1,
        source_family: "css-supports-declaration",
        target_semantics: "acir.support-condition",
        status: TranslationStatus::Verified,
    },
    TranslationSpec {
        id: CSS_LENGTH_ZERO_V1,
        source_family: "css-unitless-zero-length",
        target_semantics: "native.px-zero",
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
pub struct CompiledSelectorStateCapability {
    pub base_selector: String,
    pub predicates: Vec<AcirInteractionPredicate>,
    pub receipt: TranslationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSelectorStructuralCapability {
    pub base_selector: String,
    pub predicates: Vec<AcirStructuralPredicate>,
    pub receipt: TranslationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSelectorBooleanCapability {
    pub base_selector: String,
    pub alternatives: Vec<String>,
    pub operation: AcirSelectorBoolean,
    pub receipt: TranslationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSelectorListCapability {
    pub selectors: Vec<String>,
    pub receipt: TranslationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledRelativeLengthCapability {
    pub value: AcirRelativeLength,
    pub receipt: TranslationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityCompilerError {
    MalformedAtRule,
    UnsupportedMediaQuery(String),
    UnsupportedSelector(String),
    UnsupportedSupportCondition(String),
    UnsupportedValue(String),
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

fn parse_decimal_milli_value(raw: &str) -> Result<u32, CapabilityCompilerError> {
    let raw = raw.trim();
    let (whole, fraction) = raw.split_once('.').unwrap_or((raw, ""));
    if (whole.is_empty() && fraction.is_empty())
        || (!whole.is_empty() && !whole.chars().all(|ch| ch.is_ascii_digit()))
        || (!fraction.is_empty() && !fraction.chars().all(|ch| ch.is_ascii_digit()))
        || fraction.len() > 3
    {
        return Err(CapabilityCompilerError::UnsupportedValue(raw.into()));
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole
            .parse::<u32>()
            .map_err(|_| CapabilityCompilerError::UnsupportedValue(raw.into()))?
    };
    let mut fraction_milli = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u32>()
            .map_err(|_| CapabilityCompilerError::UnsupportedValue(raw.into()))?
    };
    for _ in fraction.len()..3 {
        fraction_milli = fraction_milli.saturating_mul(10);
    }
    whole
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(fraction_milli))
        .ok_or_else(|| CapabilityCompilerError::UnsupportedValue(raw.into()))
}

pub fn compile_font_size_capability(
    input: &str,
) -> Result<Option<CompiledRelativeLengthCapability>, CapabilityCompilerError> {
    let normalized = input.trim().to_ascii_lowercase();
    if normalized.ends_with("rem") {
        return Ok(None);
    }
    let Some(number) = normalized.strip_suffix("em") else {
        return Ok(None);
    };

    require_verified(CSS_FONT_EM_V1)?;
    let milli_factor = parse_decimal_milli_value(number)?;

    Ok(Some(CompiledRelativeLengthCapability {
        value: AcirRelativeLength {
            milli_factor,
            basis: AcirLengthBasis::ParentFontSize,
        },
        receipt: TranslationReceipt {
            translation_id: CSS_FONT_EM_V1.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

pub fn compile_current_font_length_capability(
    input: &str,
) -> Result<Option<CompiledRelativeLengthCapability>, CapabilityCompilerError> {
    let normalized = input.trim().to_ascii_lowercase();
    if normalized.ends_with("rem") {
        return Ok(None);
    }
    let Some(number) = normalized.strip_suffix("em") else {
        return Ok(None);
    };

    require_verified(CSS_LENGTH_EM_V1)?;
    let milli_factor = parse_decimal_milli_value(number)?;

    Ok(Some(CompiledRelativeLengthCapability {
        value: AcirRelativeLength {
            milli_factor,
            basis: AcirLengthBasis::CurrentFontSize,
        },
        receipt: TranslationReceipt {
            translation_id: CSS_LENGTH_EM_V1.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

pub fn compile_zero_length_capability(
    input: &str,
) -> Result<Option<TranslationReceipt>, CapabilityCompilerError> {
    if input.trim() != "0" {
        return Ok(None);
    }
    require_verified(CSS_LENGTH_ZERO_V1)?;
    Ok(Some(TranslationReceipt {
        translation_id: CSS_LENGTH_ZERO_V1.into(),
        source_sha256: sha256(input),
        decision: TranslationDecision::Admitted,
    }))
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


fn parse_selector_chain(
    input: &str,
) -> Result<Option<AcirSelectorChain>, CapabilityCompilerError> {
    let mut compounds = Vec::new();
    let mut combinators = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    let mut pending_space = false;

    let flush_current = |current: &mut String, compounds: &mut Vec<String>| {
        let value = current.trim();
        if !value.is_empty() {
            compounds.push(value.to_string());
        }
        current.clear();
    };

    for ch in input.chars() {
        if let Some(active_quote) = quote {
            current.push(ch);
            if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                current.push(ch);
            }
            ']' => {
                if bracket_depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
                }
                bracket_depth -= 1;
                current.push(ch);
            }
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                current.push(ch);
            }
            ')' => {
                if paren_depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
                }
                paren_depth -= 1;
                current.push(ch);
            }
            ch if ch.is_whitespace() && bracket_depth == 0 && paren_depth == 0 => {
                if !current.trim().is_empty() {
                    flush_current(&mut current, &mut compounds);
                    pending_space = true;
                }
            }
            '>' | '+' | '~' if bracket_depth == 0 && paren_depth == 0 => {
                if !current.trim().is_empty() {
                    flush_current(&mut current, &mut compounds);
                }
                if compounds.is_empty() || combinators.len() >= compounds.len() {
                    return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
                }
                pending_space = false;
                combinators.push(match ch {
                    '>' => AcirSelectorRelation::Child,
                    '+' => AcirSelectorRelation::AdjacentSibling,
                    '~' => AcirSelectorRelation::GeneralSibling,
                    _ => unreachable!(),
                });
            }
            _ => {
                if pending_space {
                    if combinators.len() < compounds.len() {
                        combinators.push(AcirSelectorRelation::Descendant);
                    }
                    pending_space = false;
                }
                current.push(ch);
            }
        }
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
    }

    if !current.trim().is_empty() {
        flush_current(&mut current, &mut compounds);
    }

    if compounds.len() < 2 {
        return Ok(None);
    }

    let chain = AcirSelectorChain {
        compounds,
        combinators,
    };
    if !chain.is_well_formed() {
        return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
    }
    Ok(Some(chain))
}

fn split_selector_arguments(
    input: &str,
) -> Result<Vec<String>, CapabilityCompilerError> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;

    for ch in input.chars() {
        if let Some(active_quote) = quote {
            current.push(ch);
            if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            '[' => {
                bracket_depth = bracket_depth.saturating_add(1);
                current.push(ch);
            }
            ']' => {
                if bracket_depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
                }
                bracket_depth -= 1;
                current.push(ch);
            }
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                current.push(ch);
            }
            ')' => {
                if paren_depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
                }
                paren_depth -= 1;
                current.push(ch);
            }
            ',' if bracket_depth == 0 && paren_depth == 0 => {
                let value = current.trim();
                if value.is_empty() {
                    return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
                }
                parts.push(value.to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
    }

    let tail = current.trim();
    if tail.is_empty() {
        return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
    }
    parts.push(tail.to_string());
    Ok(parts)
}

pub fn compile_selector_list_capability(
    input: &str,
) -> Result<Option<CompiledSelectorListCapability>, CapabilityCompilerError> {
    let selectors = split_selector_arguments(input)?;
    if selectors.len() < 2 {
        return Ok(None);
    }

    require_verified(CSS_SELECTOR_LIST_V1)?;
    Ok(Some(CompiledSelectorListCapability {
        selectors,
        receipt: TranslationReceipt {
            translation_id: CSS_SELECTOR_LIST_V1.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

pub fn compile_selector_boolean_capability(
    input: &str,
) -> Result<Option<CompiledSelectorBooleanCapability>, CapabilityCompilerError> {
    let input = input.trim();
    let candidates = [
        (":where(", AcirSelectorBoolean::AnyZeroSpecificity),
        (":not(", AcirSelectorBoolean::None),
        (":is(", AcirSelectorBoolean::Any),
    ];

    let Some((start, marker, operation)) = candidates
        .into_iter()
        .filter_map(|(marker, operation)| input.find(marker).map(|start| (start, marker, operation)))
        .min_by_key(|(start, _, _)| *start)
    else {
        return Ok(None);
    };

    let open = start + marker.len() - 1;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut close = None;
    for (offset, ch) in input[open..].char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
                }
                depth -= 1;
                if depth == 0 {
                    close = Some(open + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close.ok_or_else(|| CapabilityCompilerError::UnsupportedSelector(input.into()))?;
    let inner = &input[open + 1..close];
    if inner.is_empty() {
        return Err(CapabilityCompilerError::UnsupportedSelector(input.into()));
    }

    require_verified(CSS_SELECTOR_BOOLEAN_V1)?;
    let alternatives = split_selector_arguments(inner)?;
    let mut base_selector = String::new();
    base_selector.push_str(input[..start].trim());
    base_selector.push_str(input[close + 1..].trim());

    Ok(Some(CompiledSelectorBooleanCapability {
        base_selector,
        alternatives,
        operation,
        receipt: TranslationReceipt {
            translation_id: CSS_SELECTOR_BOOLEAN_V1.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

pub fn compile_selector_structural_capability(
    input: &str,
) -> Result<Option<CompiledSelectorStructuralCapability>, CapabilityCompilerError> {
    let mut base = input.trim().to_string();
    let mut predicates = Vec::new();

    loop {
        let normalized = base.to_ascii_lowercase();
        let matched = [
            (":first-child", AcirStructuralPredicate::FirstChild),
            (":last-child", AcirStructuralPredicate::LastChild),
            (":only-child", AcirStructuralPredicate::OnlyChild),
        ]
        .into_iter()
        .find(|(suffix, _)| normalized.ends_with(suffix));

        let Some((suffix, predicate)) = matched else {
            break;
        };
        let new_len = base.len().saturating_sub(suffix.len());
        base.truncate(new_len);
        base = base.trim_end().to_string();
        predicates.push(predicate);
    }

    if predicates.is_empty() {
        return Ok(None);
    }

    require_verified(CSS_SELECTOR_STRUCTURAL_V1)?;
    predicates.reverse();
    Ok(Some(CompiledSelectorStructuralCapability {
        base_selector: base,
        predicates,
        receipt: TranslationReceipt {
            translation_id: CSS_SELECTOR_STRUCTURAL_V1.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

pub fn compile_selector_state_capability(
    input: &str,
) -> Result<Option<CompiledSelectorStateCapability>, CapabilityCompilerError> {
    let mut base = input.trim().to_string();
    let mut predicates = Vec::new();

    loop {
        let normalized = base.to_ascii_lowercase();
        let matched = [
            (":focus-visible", AcirInteractionPredicate::FocusVisible),
            (":focus-within", AcirInteractionPredicate::FocusWithin),
            (":focus", AcirInteractionPredicate::Focus),
            (":hover", AcirInteractionPredicate::Hover),
            (":active", AcirInteractionPredicate::Active),
        ]
        .into_iter()
        .find(|(suffix, _)| normalized.ends_with(suffix));

        let Some((suffix, predicate)) = matched else {
            break;
        };
        let new_len = base.len().saturating_sub(suffix.len());
        base.truncate(new_len);
        base = base.trim_end().to_string();
        predicates.push(predicate);
    }

    if predicates.is_empty() {
        return Ok(None);
    }

    require_verified(CSS_SELECTOR_INTERACTION_V1)?;
    predicates.reverse();

    Ok(Some(CompiledSelectorStateCapability {
        base_selector: base,
        predicates,
        receipt: TranslationReceipt {
            translation_id: CSS_SELECTOR_INTERACTION_V1.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

pub fn compile_selector_capability(
    input: &str,
) -> Result<Option<CompiledSelectorCapability>, CapabilityCompilerError> {
    let Some(chain) = parse_selector_chain(input)? else {
        return Ok(None);
    };

    let translation_id = if chain
        .combinators
        .iter()
        .all(|relation| *relation == AcirSelectorRelation::Descendant)
    {
        CSS_SELECTOR_DESCENDANT_V1
    } else {
        CSS_SELECTOR_RELATIONS_V1
    };
    require_verified(translation_id)?;

    Ok(Some(CompiledSelectorCapability {
        chain,
        receipt: TranslationReceipt {
            translation_id: translation_id.into(),
            source_sha256: sha256(input),
            decision: TranslationDecision::Admitted,
        },
    }))
}

fn is_css_global_keyword(value: &str) -> bool {
    matches!(value.trim(), "inherit" | "initial" | "unset")
}

fn is_px_value(value: &str) -> bool {
    if value.trim() == "0" {
        return true;
    }
    value
        .trim()
        .strip_suffix("px")
        .is_some_and(|number| number.trim().parse::<f32>().is_ok_and(|v| v.is_finite() && v >= 0.0))
}

fn is_em_value(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.ends_with("rem") {
        return false;
    }
    normalized
        .strip_suffix("em")
        .is_some_and(|number| parse_decimal_milli_value(number).is_ok())
}

fn css_declaration_supported(property: &str, value: &str) -> bool {
    let property = property.trim().to_ascii_lowercase();
    let value = value.trim();

    if property.starts_with("--") {
        return !value.is_empty();
    }

    if is_css_global_keyword(value) {
        return matches!(
            property.as_str(),
            "display"
                | "flex-direction"
                | "grid-template-columns"
                | "font-size"
                | "margin-top"
                | "margin-bottom"
                | "padding-top"
                | "padding-right"
                | "padding-bottom"
                | "padding-left"
                | "gap"
        );
    }

    match property.as_str() {
        "display" => matches!(
            value,
            "none"
                | "block"
                | "inline"
                | "inline-block"
                | "flex"
                | "inline-flex"
                | "grid"
                | "inline-grid"
                | "table"
                | "inline-table"
                | "table-row"
                | "table-cell"
                | "table-row-group"
                | "table-header-group"
                | "table-footer-group"
                | "table-caption"
        ),
        "flex-direction" => matches!(value, "row" | "column"),
        "grid-template-columns" => {
            let tracks = value.split_ascii_whitespace().collect::<Vec<_>>();
            !tracks.is_empty() && tracks.len() <= 12 && tracks.iter().all(|track| *track == "1fr")
        }
        "font-size" => {
            value.starts_with("var(") && value.ends_with(')')
                || is_px_value(value)
                || compile_font_size_capability(value).is_ok_and(|compiled| compiled.is_some())
        }
        "margin-top"
        | "margin-bottom"
        | "padding-top"
        | "padding-right"
        | "padding-bottom"
        | "padding-left"
        | "gap" => {
            value.starts_with("var(") && value.ends_with(')')
                || is_px_value(value)
                || is_em_value(value)
        }
        _ => false,
    }
}

fn matching_outer_parenthesis(input: &str) -> Option<usize> {
    if !input.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (index, ch) in input.char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_support_wrapping_parens(mut input: &str) -> &str {
    loop {
        let trimmed = input.trim();
        let Some(close) = matching_outer_parenthesis(trimmed) else {
            return trimmed;
        };
        if close + 1 != trimmed.len() {
            return trimmed;
        }
        input = &trimmed[1..close];
    }
}

fn split_support_top_level<'a>(
    input: &'a str,
    operator: &str,
) -> Result<Vec<&'a str>, CapabilityCompilerError> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..]
            .chars()
            .next()
            .ok_or_else(|| CapabilityCompilerError::UnsupportedSupportCondition(input.into()))?;
        let width = ch.len_utf8();

        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            index += width;
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Err(CapabilityCompilerError::UnsupportedSupportCondition(input.into()));
                }
                depth -= 1;
            }
            _ => {}
        }

        if depth == 0 && input[index..].starts_with(operator) {
            let part = input[start..index].trim();
            if part.is_empty() {
                return Err(CapabilityCompilerError::UnsupportedSupportCondition(input.into()));
            }
            parts.push(part);
            index += operator.len();
            start = index;
            continue;
        }
        index += width;
    }

    if quote.is_some() || depth != 0 {
        return Err(CapabilityCompilerError::UnsupportedSupportCondition(input.into()));
    }
    let tail = input[start..].trim();
    if tail.is_empty() {
        return Err(CapabilityCompilerError::UnsupportedSupportCondition(input.into()));
    }
    parts.push(tail);
    Ok(parts)
}

fn parse_support_condition(raw: &str) -> Result<AcirSupportCondition, CapabilityCompilerError> {
    require_verified(CSS_SUPPORTS_DECLARATION_V1)?;
    let input = strip_support_wrapping_parens(raw);

    if let Some(rest) = input.strip_prefix("not ") {
        return Ok(AcirSupportCondition::Not(Box::new(parse_support_condition(rest)?)));
    }

    let any = split_support_top_level(input, " or ")?;
    if any.len() > 1 {
        return Ok(AcirSupportCondition::Any(
            any.into_iter()
                .map(parse_support_condition)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    let all = split_support_top_level(input, " and ")?;
    if all.len() > 1 {
        return Ok(AcirSupportCondition::All(
            all.into_iter()
                .map(parse_support_condition)
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    let declaration = strip_support_wrapping_parens(input);
    let (property, value) = declaration
        .split_once(':')
        .ok_or_else(|| CapabilityCompilerError::UnsupportedSupportCondition(raw.into()))?;
    if property.trim().is_empty() || value.trim().is_empty() {
        return Err(CapabilityCompilerError::UnsupportedSupportCondition(raw.into()));
    }

    Ok(AcirSupportCondition::CssDeclaration {
        property: property.trim().to_ascii_lowercase(),
        value: value.trim().to_string(),
    })
}

fn evaluate_support_condition(condition: &AcirSupportCondition) -> bool {
    match condition {
        AcirSupportCondition::CssDeclaration { property, value } => {
            css_declaration_supported(property, value)
        }
        AcirSupportCondition::All(conditions) => {
            conditions.iter().all(evaluate_support_condition)
        }
        AcirSupportCondition::Any(conditions) => {
            conditions.iter().any(evaluate_support_condition)
        }
        AcirSupportCondition::Not(condition) => !evaluate_support_condition(condition),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConditionalAtRuleKind {
    Media,
    Supports,
}

fn next_top_level_conditional(input: &str) -> Option<(usize, ConditionalAtRuleKind)> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut index = 0usize;

    while index < input.len() {
        let ch = input[index..].chars().next()?;
        let width = ch.len_utf8();

        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            index += width;
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '@' if depth == 0 => {
                if input[index..].starts_with("@media") {
                    return Some((index, ConditionalAtRuleKind::Media));
                }
                if input[index..].starts_with("@supports") {
                    return Some((index, ConditionalAtRuleKind::Supports));
                }
            }
            _ => {}
        }
        index += width;
    }
    None
}

fn lower_stylesheet_conditionals(
    input: &str,
    environment: CapabilityEnvironment,
    receipts: &mut Vec<TranslationReceipt>,
) -> Result<String, CapabilityCompilerError> {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while let Some((relative, kind)) = next_top_level_conditional(&input[cursor..]) {
        let start = cursor + relative;
        output.push_str(&input[cursor..start]);

        let marker = match kind {
            ConditionalAtRuleKind::Media => "@media",
            ConditionalAtRuleKind::Supports => "@supports",
        };
        let header_start = start + marker.len();
        let open_relative = input[header_start..]
            .find('{')
            .ok_or(CapabilityCompilerError::MalformedAtRule)?;
        let open = header_start + open_relative;
        let close = matching_brace(input, open)?;
        let condition_source = input[header_start..open].trim();
        let source_construct = &input[start..=close];

        let (translation_id, admitted) = match kind {
            ConditionalAtRuleKind::Media => {
                let condition = lower_media_query(condition_source)?;
                (
                    CSS_MEDIA_ENVIRONMENT_V1,
                    environment.evaluate_condition(&condition),
                )
            }
            ConditionalAtRuleKind::Supports => {
                let condition = parse_support_condition(condition_source)?;
                (
                    CSS_SUPPORTS_DECLARATION_V1,
                    evaluate_support_condition(&condition),
                )
            }
        };

        let decision = if admitted {
            let lowered_body =
                lower_stylesheet_conditionals(&input[open + 1..close], environment, receipts)?;
            output.push_str(&lowered_body);
            TranslationDecision::Admitted
        } else {
            TranslationDecision::Elided
        };
        receipts.push(TranslationReceipt {
            translation_id: translation_id.into(),
            source_sha256: sha256(source_construct),
            decision,
        });
        cursor = close + 1;
    }

    output.push_str(&input[cursor..]);
    Ok(output)
}

pub fn compile_stylesheet_capabilities(
    input: &str,
    environment: CapabilityEnvironment,
) -> Result<CompiledCapabilitySource, CapabilityCompilerError> {
    let mut receipts = Vec::new();
    let source = lower_stylesheet_conditionals(input, environment, &mut receipts)?;
    Ok(CompiledCapabilitySource { source, receipts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unitless_zero_length_compiles_but_nonzero_unitless_does_not() {
        let zero = compile_zero_length_capability("0").unwrap().unwrap();
        assert_eq!(zero.translation_id, CSS_LENGTH_ZERO_V1);
        assert!(compile_zero_length_capability("0px").unwrap().is_none());
        assert!(compile_zero_length_capability("1").unwrap().is_none());
    }

    #[test]
    fn supports_declaration_condition_uses_altru_capability_registry() {
        let unsupported = compile_stylesheet_capabilities(
            "@supports ((-webkit-mask-image:none) or (mask-image:none)) { .x { display: block; } }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(!unsupported.source.contains(".x"));
        assert_eq!(unsupported.receipts.len(), 1);
        assert_eq!(
            unsupported.receipts[0].translation_id,
            CSS_SUPPORTS_DECLARATION_V1
        );
        assert_eq!(
            unsupported.receipts[0].decision,
            TranslationDecision::Elided
        );

        let supported = compile_stylesheet_capabilities(
            "@supports (display: grid) { .x { display: grid; } }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(supported.source.contains("display: grid"));
        assert_eq!(
            supported.receipts[0].decision,
            TranslationDecision::Admitted
        );
    }

    #[test]
    fn nested_media_and_supports_are_lowered_recursively() {
        let compiled = compile_stylesheet_capabilities(
            "@media (min-width:768px) { @supports (display: grid) { .x { display: grid; } } }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(compiled.source.contains(".x"));
        assert_eq!(compiled.receipts.len(), 2);
    }

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
    fn top_level_selector_list_compiles_without_splitting_nested_commas() {
        let compiled = compile_selector_list_capability(
            ".a,.b[data-x=\"x,y\"],:is(.c,.d)",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            compiled.selectors,
            vec![".a", ".b[data-x=\"x,y\"]", ":is(.c,.d)"]
        );
        assert_eq!(compiled.receipt.translation_id, CSS_SELECTOR_LIST_V1);
    }

    #[test]
    fn boolean_selector_can_be_embedded_inside_compound() {
        let compiled = compile_selector_boolean_capability(
            "h1:where(.wp-block-heading).has-background",
        )
        .unwrap()
        .unwrap();
        assert_eq!(compiled.base_selector, "h1.has-background");
        assert_eq!(compiled.alternatives, vec![".wp-block-heading"]);
        assert_eq!(compiled.operation, AcirSelectorBoolean::AnyZeroSpecificity);
    }

    #[test]
    fn descendant_selector_compiles_without_splitting_attribute_spaces() {
        let compiled = compile_selector_capability(":root .card[data-mode=dark i]")
            .unwrap()
            .unwrap();

        assert_eq!(compiled.chain.compounds, vec![":root", ".card[data-mode=dark i]"]);
        assert_eq!(
            compiled.chain.combinators,
            vec![AcirSelectorRelation::Descendant]
        );
        assert_eq!(
            compiled.receipt.translation_id,
            CSS_SELECTOR_DESCENDANT_V1
        );
    }

    #[test]
    fn boolean_selector_family_compiles_is_where_and_not() {
        let where_selector = compile_selector_boolean_capability(":where(.has-border-color)")
            .unwrap()
            .unwrap();
        assert_eq!(where_selector.base_selector, "");
        assert_eq!(where_selector.alternatives, vec![".has-border-color"]);
        assert_eq!(
            where_selector.operation,
            AcirSelectorBoolean::AnyZeroSpecificity
        );

        let not_selector = compile_selector_boolean_capability("p:not(.lead,.summary)")
            .unwrap()
            .unwrap();
        assert_eq!(not_selector.base_selector, "p");
        assert_eq!(not_selector.alternatives, vec![".lead", ".summary"]);
        assert_eq!(not_selector.operation, AcirSelectorBoolean::None);

        let is_selector = compile_selector_boolean_capability("p:is(.lead,.summary)")
            .unwrap()
            .unwrap();
        assert_eq!(is_selector.operation, AcirSelectorBoolean::Any);
        assert_eq!(
            is_selector.receipt.translation_id,
            CSS_SELECTOR_BOOLEAN_V1
        );
    }

    #[test]
    fn boolean_selector_preserves_attribute_commas_and_case_flags() {
        let compiled = compile_selector_boolean_capability(
            "img:is([sizes=auto i],[sizes^=\"auto,\" i])",
        )
        .unwrap()
        .unwrap();
        assert_eq!(compiled.base_selector, "img");
        assert_eq!(
            compiled.alternatives,
            vec!["[sizes=auto i]", "[sizes^=\"auto,\" i]"]
        );
    }

    #[test]
    fn attribute_only_selector_is_not_mistaken_for_relation_or_boolean_syntax() {
        assert!(compile_selector_capability("[sizes^=\"auto,\" i]").unwrap().is_none());
        assert!(compile_selector_boolean_capability("[sizes^=\"auto,\" i]")
            .unwrap()
            .is_none());
        assert!(compile_selector_state_capability("[sizes^=\"auto,\" i]")
            .unwrap()
            .is_none());
    }

    #[test]
    fn interaction_state_pseudos_compile_to_acir_predicates() {
        let compiled = compile_selector_state_capability(".screen-reader-text:focus")
            .unwrap()
            .unwrap();
        assert_eq!(compiled.base_selector, ".screen-reader-text");
        assert_eq!(compiled.predicates, vec![AcirInteractionPredicate::Focus]);
        assert_eq!(
            compiled.receipt.translation_id,
            CSS_SELECTOR_INTERACTION_V1
        );

        let combined = compile_selector_state_capability(".x:hover:active")
            .unwrap()
            .unwrap();
        assert_eq!(
            combined.predicates,
            vec![
                AcirInteractionPredicate::Hover,
                AcirInteractionPredicate::Active
            ]
        );
    }

    #[test]
    fn bare_known_interaction_state_is_bounded_and_unknown_state_fails_closed() {
        let compiled = compile_selector_state_capability(":focus")
            .unwrap()
            .unwrap();
        assert!(compiled.base_selector.is_empty());
        assert_eq!(compiled.predicates, vec![AcirInteractionPredicate::Focus]);

        assert!(compile_selector_state_capability(":visited").unwrap().is_none());
    }

    #[test]
    fn selector_relation_family_compiles_child_and_siblings() {
        let child = compile_selector_capability("main > .card").unwrap().unwrap();
        assert_eq!(child.chain.combinators, vec![AcirSelectorRelation::Child]);
        assert_eq!(child.receipt.translation_id, CSS_SELECTOR_RELATIONS_V1);

        let adjacent = compile_selector_capability("h2 + p").unwrap().unwrap();
        assert_eq!(
            adjacent.chain.combinators,
            vec![AcirSelectorRelation::AdjacentSibling]
        );

        let general = compile_selector_capability("h2 ~ p").unwrap().unwrap();
        assert_eq!(
            general.chain.combinators,
            vec![AcirSelectorRelation::GeneralSibling]
        );
    }

    #[test]
    fn selector_relation_parser_preserves_nested_spaces() {
        let compiled = compile_selector_capability(
            "main > .card[data-mode=dark i] + p:is(.lead,.summary)",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            compiled.chain.compounds,
            vec!["main", ".card[data-mode=dark i]", "p:is(.lead,.summary)"]
        );
        assert_eq!(
            compiled.chain.combinators,
            vec![
                AcirSelectorRelation::Child,
                AcirSelectorRelation::AdjacentSibling
            ]
        );
    }

    #[test]
    fn font_em_compiles_to_parent_font_relative_acir() {
        let one = compile_font_size_capability("1em").unwrap().unwrap();
        assert_eq!(one.value.milli_factor, 1_000);
        assert_eq!(one.value.basis, AcirLengthBasis::ParentFontSize);
        assert_eq!(one.receipt.translation_id, CSS_FONT_EM_V1);

        let fractional = compile_font_size_capability("1.25em").unwrap().unwrap();
        assert_eq!(fractional.value.milli_factor, 1_250);

        assert!(compile_font_size_capability("1rem").unwrap().is_none());
        assert!(compile_font_size_capability("16px").unwrap().is_none());
    }

    #[test]
    fn structural_child_pseudos_compile_as_bounded_dom_predicates() {
        let last = compile_selector_structural_capability(".item:last-child")
            .unwrap()
            .unwrap();
        assert_eq!(last.base_selector, ".item");
        assert_eq!(last.predicates, vec![AcirStructuralPredicate::LastChild]);

        let bare = compile_selector_structural_capability(":only-child")
            .unwrap()
            .unwrap();
        assert!(bare.base_selector.is_empty());
        assert_eq!(bare.predicates, vec![AcirStructuralPredicate::OnlyChild]);
        assert_eq!(bare.receipt.translation_id, CSS_SELECTOR_STRUCTURAL_V1);
    }

    #[test]
    fn bare_interaction_state_compiles_without_fabricating_universal_syntax() {
        let compiled = compile_selector_state_capability(":active")
            .unwrap()
            .unwrap();
        assert!(compiled.base_selector.is_empty());
        assert_eq!(compiled.predicates, vec![AcirInteractionPredicate::Active]);
    }

    #[test]
    fn focus_visible_compiles_as_distinct_interaction_state() {
        let compiled = compile_selector_state_capability("button:focus-visible")
            .unwrap()
            .unwrap();
        assert_eq!(compiled.base_selector, "button");
        assert_eq!(
            compiled.predicates,
            vec![AcirInteractionPredicate::FocusVisible]
        );
        assert_eq!(
            compiled.receipt.translation_id,
            CSS_SELECTOR_INTERACTION_V1
        );
    }

    #[test]
    fn current_font_em_compiles_with_distinct_basis() {
        let compiled = compile_current_font_length_capability("1.5em")
            .unwrap()
            .unwrap();
        assert_eq!(compiled.value.milli_factor, 1_500);
        assert_eq!(compiled.value.basis, AcirLengthBasis::CurrentFontSize);
        assert_eq!(compiled.receipt.translation_id, CSS_LENGTH_EM_V1);
        assert!(compile_current_font_length_capability("1rem").unwrap().is_none());
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
