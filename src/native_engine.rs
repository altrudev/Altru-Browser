//! Servo-independent AWEF native execution path.
//!
//! N2 adds an AWEF-owned bounded CSS cascade and recursive box-layout path.
//! This remains an experimental compatibility subset.

use sha2::{Digest, Sha256};

use crate::capability_compiler::TranslationReceipt;
use crate::capability_ir::CapabilityEnvironment;
use crate::engine_api::{
    EngineAdapter, EngineCapability, EngineManifest, PlatformClaim, PlatformStatus, PromotionState,
};
use crate::native_css::{CssError, stylesheet_from_document_with_environment};
use crate::native_dom::NativeDocument;
use crate::native_html::{HtmlParseError, parse_document};
use crate::native_layout::{layout_document_with_styles, scene_from_layout};
use crate::native_scene::{DeterministicTextRenderer, Scene, SceneRenderer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEngineError {
    UnsupportedMarkup(String),
    UnsupportedEntity(String),
    DuplicateAttribute(String),
    MalformedMarkup,
    UnbalancedTag { expected: String, found: String },
    Css(CssError),
}

impl From<HtmlParseError> for NativeEngineError {
    fn from(value: HtmlParseError) -> Self {
        match value {
            HtmlParseError::UnsupportedMarkup(markup) => Self::UnsupportedMarkup(markup),
            HtmlParseError::UnsupportedEntity(entity) => Self::UnsupportedEntity(entity),
            HtmlParseError::DuplicateAttribute(attribute) => Self::DuplicateAttribute(attribute),
            HtmlParseError::MalformedMarkup => Self::MalformedMarkup,
            HtmlParseError::UnbalancedTag { expected, found } => {
                Self::UnbalancedTag { expected, found }
            }
        }
    }
}

impl From<CssError> for NativeEngineError {
    fn from(value: CssError) -> Self {
        Self::Css(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionEvidence {
    pub input_sha256: String,
    pub artifact_sha256: String,
    pub execution_sha256: String,
    pub translation_sha256: String,
    pub canonical_css_sha256: String,
    pub mutation_epoch: u64,
    pub scene_epoch: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeExecution {
    pub document: NativeDocument,
    pub dependencies: Vec<crate::native_dependencies::NativeDependency>,
    pub scene: Scene,
    pub artifact: String,
    pub evidence: NativeExecutionEvidence,
    pub translations: Vec<TranslationReceipt>,
    pub native_semantics: bool,
    pub production_promoted: bool,
}

#[derive(Debug, Default)]
pub struct NativeEngineAdapter;

impl EngineAdapter for NativeEngineAdapter {
    type Request = String;
    type Response = NativeExecution;
    type Error = NativeEngineError;

    fn manifest(&self) -> EngineManifest {
        let mut capabilities = vec![
            EngineCapability::HtmlDocument,
            EngineCapability::CssCascade,
            EngineCapability::BlockLayout,
            EngineCapability::InlineLayout,
            EngineCapability::OffscreenRender,
        ];
        if cfg!(feature = "taffy-layout") {
            capabilities.push(EngineCapability::FlexLayout);
            capabilities.push(EngineCapability::GridLayout);
        }
        EngineManifest {
            implementation: "awef-native".into(),
            version: "n2.1".into(),
            capabilities,
            platform_support: vec![
                PlatformClaim {
                    platform: "linux".into(),
                    status: PlatformStatus::RuntimeVerified,
                },
                PlatformClaim {
                    platform: "windows".into(),
                    status: PlatformStatus::CompileVerified,
                },
                PlatformClaim {
                    platform: "macos".into(),
                    status: PlatformStatus::CompileVerified,
                },
                PlatformClaim {
                    platform: "android".into(),
                    status: PlatformStatus::CompileVerified,
                },
                PlatformClaim {
                    platform: "ios".into(),
                    status: PlatformStatus::CompileVerified,
                },
            ],
            promotion: PromotionState::Experiment,
            provenance: "Val Rukhaylo / Altru.dev".into(),
        }
    }

    fn execute(&mut self, request: Self::Request) -> Result<Self::Response, Self::Error> {
        execute_native_document(&request)
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn execute_native_document(input: &str) -> Result<NativeExecution, NativeEngineError> {
    execute_native_document_with_external_css(input, "")
}

/// The broker supplies approved stylesheet bytes. The engine itself has no network access.
/// Author stylesheet text is compiled before layout and bound to the execution evidence.
pub fn execute_native_document_with_external_css(
    input: &str,
    external_css: &str,
) -> Result<NativeExecution, NativeEngineError> {
    let document = parse_document(input)?;
    let dependencies = crate::native_dependencies::discover_dependencies(&document);
    let mut compiled_styles =
        stylesheet_from_document_with_environment(&document, CapabilityEnvironment::default())?;
    if !external_css.is_empty() {
        if external_css.len() > 4_000_000 {
            return Err(NativeEngineError::Css(CssError::MalformedRule));
        }
        let external = crate::native_css::parse_stylesheet_with_environment(
            external_css, CapabilityEnvironment::default()
        )?;
        // Preserve the external author rules ahead of embedded author rules, with
        // monotonically increasing rule order for later source-order comparisons.
        let offset = external.stylesheet.rules.len();
        let mut rules = external.stylesheet.rules;
        for mut rule in compiled_styles.stylesheet.rules {
            rule.order += offset;
            rules.push(rule);
        }
        compiled_styles.stylesheet.rules = rules;
        compiled_styles.translations.extend(external.translations);
        compiled_styles.canonical_source_sha256 = sha256(
            format!("external-css-v1\n{}\ninline-css-v1\n{}", external_css,
                compiled_styles.canonical_source_sha256).as_bytes()
        );
    }
    let translation_material = compiled_styles
        .translations
        .iter()
        .map(TranslationReceipt::canonical_line)
        .collect::<Vec<_>>()
        .join("\n");
    let translation_sha256 = sha256(translation_material.as_bytes());
    let layout = layout_document_with_styles(&document, &compiled_styles.stylesheet, 800.0)?;
    let scene = scene_from_layout(&layout);
    let artifact = DeterministicTextRenderer.render(&scene, 800, 600);

    let input_sha256 = sha256(input.as_bytes());
    let artifact_sha256 = sha256(artifact.as_bytes());
    let execution_material = format!(
        "awef-native-n2.1-execution-v3\n{}\n{}\n{}\n{}\n{}\n{}",
        input_sha256,
        artifact_sha256,
        translation_sha256,
        compiled_styles.canonical_source_sha256,
        document.mutation_epoch(),
        scene.epoch
    );
    let execution_sha256 = sha256(execution_material.as_bytes());

    let evidence = NativeExecutionEvidence {
        input_sha256,
        artifact_sha256,
        execution_sha256,
        translation_sha256,
        canonical_css_sha256: compiled_styles.canonical_source_sha256,
        mutation_epoch: document.mutation_epoch(),
        scene_epoch: scene.epoch,
    };

    Ok(NativeExecution {
        document,
        dependencies,
        scene,
        artifact,
        evidence,
        translations: compiled_styles.translations,
        native_semantics: true,
        production_promoted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n2_manifest_reports_only_verified_platform_states() {
        let adapter = NativeEngineAdapter;
        let manifest = adapter.manifest();
        assert_eq!(manifest.promotion, PromotionState::Experiment);
        assert!(manifest.supports(EngineCapability::HtmlDocument));
        assert!(manifest.supports(EngineCapability::CssCascade));
        assert_eq!(manifest.version, "n2.1");
        assert_eq!(
            manifest.supports(EngineCapability::FlexLayout),
            cfg!(feature = "taffy-layout")
        );
        assert_eq!(
            manifest.supports(EngineCapability::GridLayout),
            cfg!(feature = "taffy-layout")
        );
        assert_eq!(
            manifest.platform_status("linux"),
            Some(PlatformStatus::RuntimeVerified)
        );
        for platform in ["windows", "macos", "android", "ios"] {
            assert_eq!(
                manifest.platform_status(platform),
                Some(PlatformStatus::CompileVerified)
            );
        }
        assert!(!manifest.production_eligible());
    }

    #[test]
    fn canonical_css_source_is_bound_into_native_execution_evidence() {
        let first = execute_native_document("<html><style>p { font-size: 18px; }</style><body><p>Bound</p></body></html>").unwrap();
        let changed = execute_native_document("<html><style>p { font-size: 19px; }</style><body><p>Bound</p></body></html>").unwrap();
        assert_eq!(first.evidence.canonical_css_sha256.len(), 64);
        assert_ne!(first.evidence.canonical_css_sha256, changed.evidence.canonical_css_sha256);
        assert_ne!(first.evidence.execution_sha256, changed.evidence.execution_sha256);
        assert!(first.native_semantics);
        assert!(!first.production_promoted);
    }

    #[test]
    fn zero_pair_gap_keeps_native_layout_and_receipt() {
        let html = "<html><head><style>div { gap: 0 0; }</style></head><body><div><p>A</p><p>B</p></div></body></html>";
        let execution = execute_native_document(html).unwrap();
        assert!(execution.native_semantics);
        assert!(execution.translations.iter().any(|r| r.translation_id == crate::capability_compiler::CSS_GAP_ZERO_PAIR_V1));
    }

    #[test]
    fn linear_calc_translation_is_bound_to_native_execution() {
        let html = "<html><head><style>p { padding-top: calc(0.667em + 2px); }</style></head><body><p>Native calc</p></body></html>";
        let execution = execute_native_document(html).unwrap();
        assert!(execution.native_semantics);
        assert!(execution.artifact.contains("Native calc"));
        assert!(execution.translations.iter().any(|r| r.translation_id == crate::capability_compiler::CSS_CALC_LINEAR_LENGTH_V1));
        let different = execute_native_document(
            "<html><head><style>p { padding-top: calc(0.667em + 3px); }</style></head><body><p>Native calc</p></body></html>"
        ).unwrap();
        assert_ne!(execution.evidence.translation_sha256, different.evidence.translation_sha256);
        assert_ne!(execution.evidence.execution_sha256, different.evidence.execution_sha256);
    }

    #[test]
    fn native_universal_selector_document_executes_with_bound_evidence() {
        let html = "<html><head><style>* { margin-top: 0; } *.highlight { font-size: 18px; }</style></head><body><p class=\"highlight\">Native universal CSS</p></body></html>";
        let execution = execute_native_document(html).unwrap();
        assert!(execution.native_semantics);
        assert!(!execution.production_promoted);
        assert!(execution.artifact.contains("Native universal CSS"));
        assert_eq!(execution.evidence.input_sha256, sha256(html.as_bytes()));
        assert_eq!(execution.evidence.execution_sha256.len(), 64);
    }

    #[test]
    fn native_path_runs_without_servo() {
        let execution =
            execute_native_document("<html><body><h1>Hello</h1><p>World</p></body></html>")
                .unwrap();
        assert!(execution.native_semantics);
        assert!(!execution.production_promoted);
        assert!(execution.artifact.contains("Hello"));
        assert!(execution.artifact.contains("World"));
        assert_eq!(execution.evidence.input_sha256.len(), 64);
        assert_eq!(execution.evidence.artifact_sha256.len(), 64);
        assert_eq!(execution.evidence.execution_sha256.len(), 64);
        assert_eq!(execution.evidence.translation_sha256.len(), 64);
    }

    #[test]
    fn media_query_translation_is_bound_into_native_evidence() {
        let execution = execute_native_document(
            "<html><head><style>@media (min-resolution:192dpi) { p { font-size: 30px; } } p { font-size: 18px; }</style></head><body><p>Translated</p></body></html>",
        )
        .unwrap();
        assert_eq!(execution.translations.len(), 1);
        assert_eq!(
            execution.translations[0].translation_id,
            crate::capability_compiler::CSS_MEDIA_ENVIRONMENT_V1
        );
        assert_eq!(
            execution.translations[0].decision,
            crate::capability_compiler::TranslationDecision::Elided
        );
    }

    #[test]
    fn embedded_css_changes_native_scene() {
        let execution = execute_native_document(
            "<html><head><style>.lead { font-size: 27px; }</style></head><body><p class=\"lead\">Styled</p></body></html>",
        )
        .unwrap();
        let debug = format!("{:?}", execution.scene.commands);
        assert!(debug.contains("27.0"));
        assert!(debug.contains("Styled"));
    }

    #[test]
    fn native_execution_receipt_is_deterministic() {
        let input = "<html><head><style>.lead { font-size: 19px; }</style></head><body><p class=\"lead\">Hello &amp; world</p></body></html>";
        let a = execute_native_document(input).unwrap();
        let b = execute_native_document(input).unwrap();
        assert_eq!(a.evidence, b.evidence);
        assert_eq!(a.artifact, b.artifact);
    }

    #[test]
    fn malformed_inline_css_fails_closed() {
        let result =
            execute_native_document("<html><body><p style=\"font-size nope\">X</p></body></html>");
        assert!(matches!(result, Err(NativeEngineError::Css(_))));
    }

    #[test]
    fn selector_relation_executes_end_to_end() {
        let result = execute_native_document(
            "<html><head><style>main > p { font-size: 20px; }</style></head><body><main><p>X</p></main></body></html>",
        )
        .unwrap();
        let debug = format!("{:?}", result.scene.commands);
        assert!(debug.contains("20.0"));
        assert!(debug.contains("X"));
    }

    #[test]
    fn malformed_selector_relation_fails_closed() {
        let result = execute_native_document(
            "<html><head><style>main > > p { font-size: 20px; }</style></head><body><main><p>X</p></main></body></html>",
        );
        assert!(matches!(
            result,
            Err(NativeEngineError::Css(CssError::UnsupportedSelector(_)))
        ));
    }

    #[test]
    fn native_path_rejects_unknown_entities() {
        let result =
            execute_native_document("<html><body><p>&not-yet-supported;</p></body></html>");
        assert!(matches!(
            result,
            Err(NativeEngineError::UnsupportedEntity(_))
        ));
    }

    #[test]
    fn native_path_rejects_unbalanced_markup() {
        let result = execute_native_document("<html><body><p>Hello</body></html>");
        assert_eq!(
            result,
            Err(NativeEngineError::UnbalancedTag {
                expected: "p".into(),
                found: "body".into(),
            })
        );
    }
}
