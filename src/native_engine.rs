//! Servo-independent AWEF native execution path.
//!
//! N2 adds an AWEF-owned bounded CSS cascade and recursive box-layout path.
//! This remains an experimental compatibility subset.

use sha2::{Digest, Sha256};

use crate::engine_api::{
    EngineAdapter, EngineCapability, EngineManifest, PlatformClaim, PlatformStatus, PromotionState,
};
use crate::acir::EnvironmentSnapshot;
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
    pub mutation_epoch: u64,
    pub scene_epoch: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeExecution {
    pub document: NativeDocument,
    pub scene: Scene,
    pub artifact: String,
    pub evidence: NativeExecutionEvidence,
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
    let document = parse_document(input)?;
    let environment = EnvironmentSnapshot::desktop_preview();
    let stylesheet = stylesheet_from_document_with_environment(&document, environment)?;
    let layout = layout_document_with_styles(&document, &stylesheet, environment.viewport_width_px as f32)?;
    let scene = scene_from_layout(&layout);
    let artifact = DeterministicTextRenderer.render(&scene, 800, 600);

    let input_sha256 = sha256(input.as_bytes());
    let artifact_sha256 = sha256(artifact.as_bytes());
    let execution_material = format!(
        "awef-native-n2.1-execution-v1\n{}\n{}\n{}\n{}",
        input_sha256,
        artifact_sha256,
        document.mutation_epoch(),
        scene.epoch
    );
    let execution_sha256 = sha256(execution_material.as_bytes());

    let evidence = NativeExecutionEvidence {
        input_sha256,
        artifact_sha256,
        execution_sha256,
        mutation_epoch: document.mutation_epoch(),
        scene_epoch: scene.epoch,
    };

    Ok(NativeExecution {
        document,
        scene,
        artifact,
        evidence,
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
    fn unsupported_selector_fails_closed() {
        let result = execute_native_document(
            "<html><head><style>main p { font-size: 20px; }</style></head><body><main><p>X</p></main></body></html>",
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
