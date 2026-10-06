use crate::model::{Classification, Confidence, ExecutionPlane, Signal};

const HARD_COMPLEX_MARKERS: &[(&str, Signal)] = &[
    ("<script", Signal::Script),
    ("type=\"module\"", Signal::ModuleScript),
    ("type='module'", Signal::ModuleScript),
    ("customelements.", Signal::CustomElement),
    ("attachshadow", Signal::ShadowDom),
    ("webassembly", Signal::WebAssembly),
    ("rtcpeerconnection", Signal::WebRtc),
    ("getusermedia", Signal::WebRtc),
    ("webgl", Signal::WebGl),
    ("<canvas", Signal::Canvas),
    ("serviceworker", Signal::ServiceWorker),
    ("import(", Signal::DynamicImport),
    ("<iframe", Signal::NestedBrowsingContext),
    ("<object", Signal::EmbeddedObject),
    ("<embed", Signal::EmbeddedObject),
    ("<form", Signal::Form),
    ("<video", Signal::Media),
    ("<audio", Signal::Media),
    ("<style", Signal::StyleSheet),
    ("rel=\"stylesheet\"", Signal::StyleSheet),
    ("rel='stylesheet'", Signal::StyleSheet),
];

pub fn classify_document(input: &str) -> Classification {
    let lower = input.to_ascii_lowercase();
    let mut signals = Vec::new();

    for (marker, signal) in HARD_COMPLEX_MARKERS {
        if lower.contains(marker) {
            signals.push(signal.clone());
        }
    }

    let looks_like_html = lower.contains("<html")
        || lower.contains("<!doctype html")
        || lower.contains("<body")
        || lower.contains("<article")
        || lower.contains("<main");

    if !signals.is_empty() {
        return Classification {
            plane: ExecutionPlane::Servo,
            confidence: Confidence::Complex,
            rationale: format!(
                "Compatibility plane required: {} unsupported or active capability signal(s) detected.",
                signals.len()
            ),
            signals,
        };
    }

    if !looks_like_html {
        return Classification {
            plane: ExecutionPlane::Servo,
            confidence: Confidence::Uncertain,
            rationale: "Input is not confidently recognized as bounded HTML; fail closed to Servo."
                .into(),
            signals: vec![Signal::UnknownActiveContent],
        };
    }

    Classification {
        plane: ExecutionPlane::Light,
        confidence: Confidence::ProvenBounded,
        rationale: "Document is a Phase 1 Light candidate; the bounded parser must independently validate syntax before execution.".into(),
        signals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_article_can_be_a_light_candidate() {
        let c =
            classify_document("<!doctype html><html><body><article>Hello</article></body></html>");
        assert_eq!(c.plane, ExecutionPlane::Light);
        assert_eq!(c.confidence, Confidence::ProvenBounded);
    }

    #[test]
    fn scripts_force_servo() {
        let c = classify_document("<html><body><script>alert(1)</script></body></html>");
        assert_eq!(c.plane, ExecutionPlane::Servo);
        assert!(c.signals.contains(&Signal::Script));
    }

    #[test]
    fn uppercase_script_still_forces_servo() {
        let c = classify_document("<HTML><BODY><SCRIPT>alert(1)</SCRIPT></BODY></HTML>");
        assert_eq!(c.plane, ExecutionPlane::Servo);
        assert!(c.signals.contains(&Signal::Script));
    }

    #[test]
    fn nested_browsing_context_forces_servo() {
        let c = classify_document("<html><body><iframe src=\"x\"></iframe></body></html>");
        assert_eq!(c.plane, ExecutionPlane::Servo);
        assert!(c.signals.contains(&Signal::NestedBrowsingContext));
    }

    #[test]
    fn wasm_forces_servo() {
        let c = classify_document("<html><body>WebAssembly.instantiate(x)</body></html>");
        assert_eq!(c.plane, ExecutionPlane::Servo);
        assert!(c.signals.contains(&Signal::WebAssembly));
    }

    #[test]
    fn ambiguity_fails_closed() {
        let c = classify_document("not html, maybe generated content");
        assert_eq!(c.plane, ExecutionPlane::Servo);
        assert_eq!(c.confidence, Confidence::Uncertain);
    }
}
