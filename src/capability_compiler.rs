//! Deterministic capability compiler.
//!
//! Known source constructs are lowered into ACIR and only transforms present in the
//! verified translation registry are available to the runtime.

use sha2::{Digest, Sha256};

use crate::capability_ir::{
    AcirComparison, AcirEnvironmentFeature, AcirEnvironmentPredicate, CapabilityEnvironment,
};

pub const CSS_MEDIA_RESOLUTION_V1: &str = "css.media-resolution.v1";

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

pub const VERIFIED_TRANSLATIONS: &[TranslationSpec] = &[TranslationSpec {
    id: CSS_MEDIA_RESOLUTION_V1,
    source_family: "css-media-resolution",
    target_semantics: "acir.environment-predicate",
    status: TranslationStatus::Verified,
}];

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

fn parse_resolution_milli_dpi(raw: &str) -> Result<u32, CapabilityCompilerError> {
    let raw = raw.trim().to_ascii_lowercase();
    if let Some(number) = raw.strip_suffix("dpi") {
        let value = number
            .trim()
            .parse::<u32>()
            .map_err(|_| CapabilityCompilerError::UnsupportedMediaQuery(raw.clone()))?;
        return value
            .checked_mul(1_000)
            .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(raw));
    }
    if let Some(number) = raw.strip_suffix("dppx") {
        let value = number
            .trim()
            .parse::<u32>()
            .map_err(|_| CapabilityCompilerError::UnsupportedMediaQuery(raw.clone()))?;
        return value
            .checked_mul(96_000)
            .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(raw));
    }
    Err(CapabilityCompilerError::UnsupportedMediaQuery(raw))
}

fn lower_media_query(raw: &str) -> Result<AcirEnvironmentPredicate, CapabilityCompilerError> {
    require_verified(CSS_MEDIA_RESOLUTION_V1)?;
    let query = raw.trim();
    let query = query
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(query.into()))?;
    let (feature, value) = query
        .split_once(':')
        .ok_or_else(|| CapabilityCompilerError::UnsupportedMediaQuery(query.into()))?;
    let comparison = match feature.trim().to_ascii_lowercase().as_str() {
        "min-resolution" => AcirComparison::AtLeast,
        "max-resolution" => AcirComparison::AtMost,
        _ => return Err(CapabilityCompilerError::UnsupportedMediaQuery(query.into())),
    };
    Ok(AcirEnvironmentPredicate {
        feature: AcirEnvironmentFeature::ResolutionMilliDpi,
        comparison,
        value: parse_resolution_milli_dpi(value)?,
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
        let predicate = lower_media_query(query)?;
        let source_construct = &input[media_start..=close];
        let decision = if environment.evaluate(predicate) {
            output.push_str(&input[open + 1..close]);
            TranslationDecision::Admitted
        } else {
            TranslationDecision::Elided
        };
        receipts.push(TranslationReceipt {
            translation_id: CSS_MEDIA_RESOLUTION_V1.into(),
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
    fn min_resolution_media_query_lowers_and_elides() {
        let compiled = compile_stylesheet_capabilities(
            "@media (min-resolution:192dpi) { p { font-size: 20px; } } h1 { font-size: 24px; }",
            CapabilityEnvironment::new(96_000),
        )
        .unwrap();
        assert!(!compiled.source.contains("p {"));
        assert!(compiled.source.contains("h1 {"));
        assert_eq!(compiled.receipts.len(), 1);
        assert_eq!(compiled.receipts[0].decision, TranslationDecision::Elided);
    }

    #[test]
    fn min_resolution_media_query_admits_body_when_true() {
        let compiled = compile_stylesheet_capabilities(
            "@media (min-resolution:192dpi) { p { font-size: 20px; } }",
            CapabilityEnvironment::new(192_000),
        )
        .unwrap();
        assert!(compiled.source.contains("p { font-size: 20px; }"));
        assert_eq!(compiled.receipts[0].decision, TranslationDecision::Admitted);
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
