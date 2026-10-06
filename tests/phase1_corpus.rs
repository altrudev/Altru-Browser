use adaptive_web_engine_fabric::model::{ExecutionPlane, Signal};
use adaptive_web_engine_fabric::route_document;

#[test]
fn corpus_routes_static_structure_to_light() {
    let fixtures = [
        "<html><body><main><h1>A</h1><p>B</p></main></body></html>",
        "<html><body><article><blockquote>Hello world</blockquote></article></body></html>",
        "<html><body><ul><li>One</li><li>Two</li></ul></body></html>",
        "<html><body><pre><code>let x = 1;</code></pre></body></html>",
    ];

    for fixture in fixtures {
        let (receipt, result) = route_document(fixture);
        assert_eq!(receipt.classification.plane, ExecutionPlane::Light);
        assert!(result.produced_output);
        assert!(
            result
                .artifact
                .as_deref()
                .unwrap_or_default()
                .starts_with("<svg")
        );
    }
}

#[test]
fn corpus_fails_closed_for_richer_web_content() {
    let fixtures = [
        "<html><body><p class=\"x\">Attribute</p></body></html>",
        "<html><body><style>p{color:red}</style><p>CSS</p></body></html>",
        "<html><body><form><input></form></body></html>",
        "<html><body><iframe src=\"about:blank\"></iframe></body></html>",
        "<html><body><video></video></body></html>",
        "<html><body><marquee>Unknown</marquee></body></html>",
    ];

    for fixture in fixtures {
        let (receipt, _) = route_document(fixture);
        assert_eq!(receipt.classification.plane, ExecutionPlane::Servo);
    }
}

#[test]
fn parser_fallback_is_evident_in_receipt() {
    let (receipt, _) = route_document("<html><body><p data-x=\"1\">Looks static</p></body></html>");
    assert!(
        receipt
            .classification
            .signals
            .contains(&Signal::UnsupportedLightSyntax)
    );
}
