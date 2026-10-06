use std::time::Instant;

use adaptive_web_engine_fabric::metrics::process_metrics;
use adaptive_web_engine_fabric::model::ExecutionPlane;
use adaptive_web_engine_fabric::route_document_with_receipts;

fn make_static_document(paragraphs: usize) -> String {
    let mut html = String::from("<!doctype html><html><body><main><article><h1>Benchmark</h1>");
    for _ in 0..paragraphs {
        html.push_str("<p>Deterministic bounded content for the light execution plane.</p>");
    }
    html.push_str("</article></main></body></html>");
    html
}

fn run_case(name: &str, input: &str, iterations: usize) {
    let start = Instant::now();
    let mut light = 0usize;
    let mut servo = 0usize;
    let mut artifact_bytes = 0usize;
    let mut receipt_bytes = 0usize;

    for _ in 0..iterations {
        let (decision, execution, result) = route_document_with_receipts(input);
        match decision.classification.plane {
            ExecutionPlane::Light => light += 1,
            ExecutionPlane::Servo => servo += 1,
            ExecutionPlane::Continuity => {}
        }
        artifact_bytes = artifact_bytes.saturating_add(
            result
                .artifact
                .as_ref()
                .map(|artifact| artifact.len())
                .unwrap_or(0),
        );
        receipt_bytes = receipt_bytes.saturating_add(execution.execution_sha256.len());
    }

    let elapsed = start.elapsed();
    let metrics = process_metrics();
    println!(
        "case={name} iterations={iterations} elapsed_ns={} per_iteration_ns={} light={} servo={} artifact_bytes={} receipt_bytes={} peak_rss_kib={}",
        elapsed.as_nanos(),
        elapsed.as_nanos() / iterations.max(1) as u128,
        light,
        servo,
        artifact_bytes,
        receipt_bytes,
        metrics
            .peak_rss_kib
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".into())
    );
}

fn main() {
    let bounded = make_static_document(100);
    let scripted = "<html><body><script>document.body.dataset.x='1'</script></body></html>";
    let attribute_fallback = "<html><body><p class=\"x\">fallback</p></body></html>";

    run_case("bounded-100p", &bounded, 500);
    run_case("scripted", scripted, 500);
    run_case("light-fallback", attribute_fallback, 500);
}
