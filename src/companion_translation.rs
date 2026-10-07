use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const TRANSLATION_PACK_SCHEMA: &str = "altru.companion.translation-pack.v1";
pub const ACIR_SCHEMA: &str = "altru.companion.acir.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumericComparator {
    AtLeast,
    AtMost,
    Equal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributePredicateOperator {
    Equals,
    Prefix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcirNode {
    Tag(String),
    AttributePredicate {
        name: String,
        operator: AttributePredicateOperator,
        value: String,
        case_insensitive: bool,
    },
    SelectorAnd(Vec<AcirNode>),
    SelectorOr(Vec<AcirNode>),
    EnvironmentPredicate {
        key: String,
        comparator: NumericComparator,
        value_milli: u64,
        unit: String,
    },
    Conditional {
        condition: Box<AcirNode>,
        body_ref: String,
    },
    CapabilityRef(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcirProgram {
    pub schema: String,
    pub source_language: String,
    pub source_feature: String,
    pub nodes: Vec<AcirNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotedTranslation {
    pub schema: String,
    pub translation_id: String,
    pub source_signature: String,
    pub acir: AcirProgram,
    pub semantic_fidelity_ppm: u32,
    pub authority_cost_ppm: u32,
    pub evidence_refs: Vec<String>,
    pub verification_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationPack {
    pub schema: String,
    pub pack_id: String,
    pub target: String,
    pub registry_hash: String,
    pub graph_hash: String,
    pub translations: Vec<PromotedTranslation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SourceConstruct<'a> {
    language: &'a str,
    feature: &'a str,
    source: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityPackError {
    Io(String),
    Malformed(String),
    WrongSchema,
    WrongTarget,
    InvalidBinding,
    UnsupportedAcir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEnvironment {
    pub display_resolution_milli_dpi: u64,
}

impl Default for CapabilityEnvironment {
    fn default() -> Self {
        Self {
            display_resolution_milli_dpi: 96_000,
        }
    }
}

impl CapabilityEnvironment {
    pub fn from_process() -> Self {
        let value = std::env::var("ALTRU_DISPLAY_DPI")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| (36..=600).contains(value))
            .map(|dpi| dpi.saturating_mul(1_000))
            .unwrap_or(96_000);
        Self {
            display_resolution_milli_dpi: value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityRuntime {
    pack: TranslationPack,
    environment: CapabilityEnvironment,
}

impl CapabilityRuntime {
    pub fn from_pack(
        pack: TranslationPack,
        environment: CapabilityEnvironment,
    ) -> Result<Self, CapabilityPackError> {
        validate_pack(&pack)?;
        Ok(Self { pack, environment })
    }

    pub fn load(
        path: &Path,
        environment: CapabilityEnvironment,
    ) -> Result<Self, CapabilityPackError> {
        let bytes = fs::read(path).map_err(|error| CapabilityPackError::Io(error.to_string()))?;
        let pack: TranslationPack = serde_json::from_slice(&bytes)
            .map_err(|error| CapabilityPackError::Malformed(error.to_string()))?;
        Self::from_pack(pack, environment)
    }

    pub fn load_default() -> Result<Self, CapabilityPackError> {
        Self::load(&default_pack_path(), CapabilityEnvironment::from_process())
    }

    pub fn evaluate_css_media(&self, source: &str) -> Result<Option<bool>, CapabilityPackError> {
        let signature = source_signature("css", "media:min-resolution", source);
        let Some(translation) = self
            .pack
            .translations
            .iter()
            .find(|translation| translation.source_signature == signature)
        else {
            return Ok(None);
        };

        if translation.acir.schema != ACIR_SCHEMA
            || translation.acir.source_language != "css"
            || translation.acir.source_feature != "media:min-resolution"
        {
            return Err(CapabilityPackError::InvalidBinding);
        }

        if translation.acir.nodes.len() != 1 {
            return Err(CapabilityPackError::UnsupportedAcir);
        }

        match &translation.acir.nodes[0] {
            AcirNode::EnvironmentPredicate {
                key,
                comparator,
                value_milli,
                unit,
            } if key == "display.resolution" && unit == "dpi" => {
                let actual = self.environment.display_resolution_milli_dpi;
                let result = match comparator {
                    NumericComparator::AtLeast => actual >= *value_milli,
                    NumericComparator::AtMost => actual <= *value_milli,
                    NumericComparator::Equal => actual == *value_milli,
                };
                Ok(Some(result))
            }
            _ => Err(CapabilityPackError::UnsupportedAcir),
        }
    }

    pub fn pack_id(&self) -> &str {
        &self.pack.pack_id
    }
}

pub fn default_pack_path() -> PathBuf {
    if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state_home)
            .join("altru-companion")
            .join("packs")
            .join("altru-browser.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("altru-companion")
            .join("packs")
            .join("altru-browser.json");
    }
    std::env::temp_dir()
        .join("altru-companion")
        .join("packs")
        .join("altru-browser.json")
}

fn source_signature(language: &str, feature: &str, source: &str) -> String {
    let value = SourceConstruct {
        language,
        feature,
        source,
    };
    let bytes = serde_json::to_vec(&value).expect("source construct serialization is infallible");
    hex::encode(Sha256::digest(bytes))
}

fn validate_pack(pack: &TranslationPack) -> Result<(), CapabilityPackError> {
    if pack.schema != TRANSLATION_PACK_SCHEMA {
        return Err(CapabilityPackError::WrongSchema);
    }
    if pack.target != "altru-browser" {
        return Err(CapabilityPackError::WrongTarget);
    }
    if pack.registry_hash.len() != 64 || pack.graph_hash.len() != 64 || pack.translations.is_empty() {
        return Err(CapabilityPackError::InvalidBinding);
    }

    let mut seen = std::collections::BTreeSet::new();
    for translation in &pack.translations {
        if translation.source_signature.len() != 64
            || translation.verification_hash.len() != 64
            || translation.evidence_refs.is_empty()
            || !seen.insert(translation.source_signature.clone())
        {
            return Err(CapabilityPackError::InvalidBinding);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media_pack(source: &str) -> TranslationPack {
        let signature = source_signature("css", "media:min-resolution", source);
        TranslationPack {
            schema: TRANSLATION_PACK_SCHEMA.into(),
            pack_id: "browser-css-foundation-v1".into(),
            target: "altru-browser".into(),
            registry_hash: "a".repeat(64),
            graph_hash: "b".repeat(64),
            translations: vec![PromotedTranslation {
                schema: "altru.companion.translation.v1".into(),
                translation_id: "css-media-min-resolution-v1".into(),
                source_signature: signature,
                acir: AcirProgram {
                    schema: ACIR_SCHEMA.into(),
                    source_language: "css".into(),
                    source_feature: "media:min-resolution".into(),
                    nodes: vec![AcirNode::EnvironmentPredicate {
                        key: "display.resolution".into(),
                        comparator: NumericComparator::AtLeast,
                        value_milli: 192_000,
                        unit: "dpi".into(),
                    }],
                },
                semantic_fidelity_ppm: 1_000_000,
                authority_cost_ppm: 0,
                evidence_refs: vec!["frequency:verified".into()],
                verification_hash: "c".repeat(64),
            }],
        }
    }

    #[test]
    fn exact_promoted_media_translation_is_evaluated() {
        let source = "@media (min-resolution:192dpi)";
        let low = CapabilityRuntime::from_pack(
            media_pack(source),
            CapabilityEnvironment {
                display_resolution_milli_dpi: 96_000,
            },
        )
        .unwrap();
        let high = CapabilityRuntime::from_pack(
            media_pack(source),
            CapabilityEnvironment {
                display_resolution_milli_dpi: 220_000,
            },
        )
        .unwrap();

        assert_eq!(low.evaluate_css_media(source).unwrap(), Some(false));
        assert_eq!(high.evaluate_css_media(source).unwrap(), Some(true));
    }

    #[test]
    fn unknown_source_does_not_gain_translation_authority() {
        let runtime = CapabilityRuntime::from_pack(
            media_pack("@media (min-resolution:192dpi)"),
            CapabilityEnvironment::default(),
        )
        .unwrap();

        assert_eq!(
            runtime
                .evaluate_css_media("@media (min-resolution:144dpi)")
                .unwrap(),
            None
        );
    }

    #[test]
    fn wrong_target_pack_is_rejected() {
        let mut pack = media_pack("@media (min-resolution:192dpi)");
        pack.target = "other-product".into();
        assert_eq!(
            CapabilityRuntime::from_pack(pack, CapabilityEnvironment::default()).unwrap_err(),
            CapabilityPackError::WrongTarget
        );
    }
}
