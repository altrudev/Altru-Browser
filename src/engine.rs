use crate::light::{parse_bounded_html, render_svg};
use crate::model::ExecutionPlane;
use crate::servo_adapter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineResult {
    pub plane: ExecutionPlane,
    pub authoritative: bool,
    pub produced_output: bool,
    pub fallback_required: bool,
    pub promotion_verified: bool,
    pub artifact: Option<String>,
    pub summary: String,
}

pub trait WebEngine {
    fn name(&self) -> &'static str;
    fn execute(&self, document: &str) -> EngineResult;
}

pub struct LightEngine;

impl WebEngine for LightEngine {
    fn name(&self) -> &'static str {
        "light-phase1"
    }

    fn execute(&self, document: &str) -> EngineResult {
        match parse_bounded_html(document) {
            Ok(parsed) => {
                let artifact = render_svg(&parsed, 800);
                let bytes = artifact.len();
                EngineResult {
                    plane: ExecutionPlane::Light,
                    authoritative: true,
                    produced_output: true,
                    fallback_required: false,
                    promotion_verified: false,
                    artifact: Some(artifact),
                    summary: format!(
                        "bounded document rendered deterministically to SVG; artifact_bytes={bytes}"
                    ),
                }
            }
            Err(error) => EngineResult {
                plane: ExecutionPlane::Light,
                authoritative: false,
                produced_output: false,
                fallback_required: true,
                promotion_verified: false,
                artifact: None,
                summary: format!("light renderer rejected candidate: {error}"),
            },
        }
    }
}

pub struct ServoAdapter;

impl WebEngine for ServoAdapter {
    fn name(&self) -> &'static str {
        "servo-adapter-0.6"
    }

    fn execute(&self, _document: &str) -> EngineResult {
        let probe = servo_adapter::linkage_probe();
        EngineResult {
            plane: ExecutionPlane::Servo,
            authoritative: false,
            produced_output: false,
            fallback_required: false,
            promotion_verified: false,
            artifact: None,
            summary: format!(
                "Servo {} adapter boundary present; compiled_with_servo={}; runtime_ready={}",
                probe.crate_version,
                probe.compiled_with_servo,
                servo_adapter::runtime_ready()
            ),
        }
    }
}
