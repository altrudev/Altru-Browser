use sha2::{Digest, Sha256};

use crate::engine::EngineResult;
use crate::model::{Classification, ExecutionPlane};

#[derive(Debug, Clone)]
pub struct DecisionReceipt {
    pub input_sha256: String,
    pub decision_sha256: String,
    pub classification: Classification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub decision_sha256: String,
    pub execution_sha256: String,
    pub artifact_sha256: Option<String>,
    pub plane: ExecutionPlane,
    pub authoritative: bool,
    pub produced_output: bool,
    pub promotion_verified: bool,
}

fn sha256(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

pub fn receipt(input: &str, classification: Classification) -> DecisionReceipt {
    let decision_material = format!(
        "awef-phase1-decision-v1\n{:?}\n{:?}\n{:?}\n{}",
        classification.plane,
        classification.confidence,
        classification.signals,
        classification.rationale
    );

    DecisionReceipt {
        input_sha256: sha256(input.as_bytes()),
        decision_sha256: sha256(decision_material.as_bytes()),
        classification,
    }
}

pub fn execution_receipt(decision: &DecisionReceipt, result: &EngineResult) -> ExecutionReceipt {
    let artifact_sha256 = result
        .artifact
        .as_deref()
        .map(|artifact| sha256(artifact.as_bytes()));

    let execution_material = format!(
        "awef-phase1-execution-v1\n{}\n{:?}\n{}\n{}\n{}\n{}",
        decision.decision_sha256,
        result.plane,
        result.authoritative,
        result.produced_output,
        result.promotion_verified,
        artifact_sha256.as_deref().unwrap_or("none")
    );

    ExecutionReceipt {
        decision_sha256: decision.decision_sha256.clone(),
        execution_sha256: sha256(execution_material.as_bytes()),
        artifact_sha256,
        plane: result.plane,
        authoritative: result.authoritative,
        produced_output: result.produced_output,
        promotion_verified: result.promotion_verified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::classify_document;

    #[test]
    fn decision_receipts_are_deterministic_for_identical_inputs() {
        let input = "<html><body><main>Hello</main></body></html>";
        let a = receipt(input, classify_document(input));
        let b = receipt(input, classify_document(input));
        assert_eq!(a.input_sha256, b.input_sha256);
        assert_eq!(a.decision_sha256, b.decision_sha256);
        assert_eq!(a.input_sha256.len(), 64);
        assert_eq!(a.decision_sha256.len(), 64);
    }

    #[test]
    fn execution_receipt_binds_artifact_and_promotion_state() {
        let decision = receipt(
            "<html><body>Hello</body></html>",
            classify_document("<html><body>Hello</body></html>"),
        );
        let result = EngineResult {
            plane: ExecutionPlane::Light,
            authoritative: true,
            produced_output: true,
            fallback_required: false,
            promotion_verified: false,
            artifact: Some("<svg>hello</svg>".into()),
            summary: "test".into(),
        };
        let execution = execution_receipt(&decision, &result);
        assert_eq!(execution.decision_sha256, decision.decision_sha256);
        assert_eq!(execution.artifact_sha256.as_deref().map(str::len), Some(64));
        assert_eq!(execution.execution_sha256.len(), 64);
        assert!(!execution.promotion_verified);
    }
}
