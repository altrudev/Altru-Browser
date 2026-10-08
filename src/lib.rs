pub mod browser_kernel;
pub mod capability_compiler;
pub mod capability_ir;
pub mod classifier;
pub mod companion_observation;
pub mod engine;
pub mod engine_api;
pub mod governor;
pub mod html_canonical_data;
pub mod intrinsic_measurement;
pub mod intrinsic_text;
pub mod interaction_state;
pub mod light;
pub mod metrics;
pub mod model;
pub mod native_css;
pub mod native_dom;
pub mod native_engine;
pub mod native_html;
pub mod native_invalidation;
pub mod native_layout;
pub mod native_layout_taffy;
pub mod native_runtime;
pub mod native_scene;
pub mod native_style;
pub mod platform;
pub mod receipt;
pub mod resource_api;
pub mod script_engine;
pub mod servo_adapter;
pub mod servo_runtime;

use classifier::classify_document;
use engine::{LightEngine, ServoAdapter, WebEngine};
use model::{Confidence, ExecutionPlane, Signal};
use receipt::{DecisionReceipt, ExecutionReceipt, execution_receipt, receipt};

pub fn route_document_with_receipts(
    input: &str,
) -> (DecisionReceipt, ExecutionReceipt, engine::EngineResult) {
    let mut classification = classify_document(input);

    if classification.plane == ExecutionPlane::Light {
        let light_result = LightEngine.execute(input);
        if !light_result.fallback_required {
            let decision_receipt = receipt(input, classification);
            let execution = execution_receipt(&decision_receipt, &light_result);
            return (decision_receipt, execution, light_result);
        }

        classification.plane = ExecutionPlane::Servo;
        classification.confidence = Confidence::Uncertain;
        classification.signals.push(Signal::UnsupportedLightSyntax);
        classification.rationale = format!(
            "{} Fail-closed escalation: {}",
            classification.rationale, light_result.summary
        );
    }

    let decision_receipt = receipt(input, classification);
    let result = ServoAdapter.execute(input);
    let execution = execution_receipt(&decision_receipt, &result);
    (decision_receipt, execution, result)
}

pub fn route_document(input: &str) -> (DecisionReceipt, engine::EngineResult) {
    let (decision, _execution, result) = route_document_with_receipts(input);
    (decision, result)
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn bounded_document_renders_in_light_plane() {
        let (decision, execution, result) = route_document_with_receipts(
            "<html><body><article><h1>Bounded</h1></article></body></html>",
        );
        assert_eq!(decision.classification.plane, ExecutionPlane::Light);
        assert!(result.authoritative);
        assert!(result.produced_output);
        assert!(!result.fallback_required);
        assert!(!result.promotion_verified);
        assert!(
            result
                .artifact
                .as_deref()
                .unwrap_or_default()
                .starts_with("<svg")
        );
        assert_eq!(execution.artifact_sha256.as_deref().map(str::len), Some(64));
        assert_eq!(execution.plane, ExecutionPlane::Light);
        assert!(!execution.promotion_verified);
    }

    #[test]
    fn active_content_routes_to_servo_boundary() {
        let (decision, execution, result) =
            route_document_with_receipts("<html><body><script>doWork()</script></body></html>");
        assert_eq!(decision.classification.plane, ExecutionPlane::Servo);
        assert!(!result.authoritative);
        assert!(!result.produced_output);
        assert!(execution.artifact_sha256.is_none());
        assert_eq!(execution.plane, ExecutionPlane::Servo);
    }

    #[test]
    fn unsupported_light_syntax_escalates_to_servo() {
        let (decision, execution, result) = route_document_with_receipts(
            "<html><body><p class=\"x\">Bounded-looking</p></body></html>",
        );
        assert_eq!(decision.classification.plane, ExecutionPlane::Servo);
        assert_eq!(decision.classification.confidence, Confidence::Uncertain);
        assert!(
            decision
                .classification
                .signals
                .contains(&Signal::UnsupportedLightSyntax)
        );
        assert_eq!(result.plane, ExecutionPlane::Servo);
        assert_eq!(execution.plane, ExecutionPlane::Servo);
    }
}
