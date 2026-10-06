use adaptive_web_engine_fabric::native_engine::{NativeEngineError, execute_native_document};

#[test]
fn embedded_css_specificity_reaches_scene() {
    let execution = execute_native_document(
        "<html><head><style>p { font-size: 18px; } .lead { font-size: 20px; } #hero { font-size: 24px; }</style></head><body><p id=\"hero\" class=\"lead\">Hello</p></body></html>",
    )
    .unwrap();

    let debug = format!("{:?}", execution.scene.commands);
    assert!(debug.contains("24.0"));
}

#[test]
fn inline_style_overrides_author_rule() {
    let execution = execute_native_document(
        "<html><head><style>.lead { font-size: 20px; }</style></head><body><p class=\"lead\" style=\"font-size: 26px\">Hello</p></body></html>",
    )
    .unwrap();

    let debug = format!("{:?}", execution.scene.commands);
    assert!(debug.contains("26.0"));
}

#[test]
fn display_none_removes_subtree_from_scene() {
    let execution = execute_native_document(
        "<html><head><style>.hidden { display: none; }</style></head><body><div class=\"hidden\"><p>Secret</p></div><p>Visible</p></body></html>",
    )
    .unwrap();

    assert!(!execution.artifact.contains("Secret"));
    assert!(execution.artifact.contains("Visible"));
}

#[test]
fn malformed_inline_css_fails_closed_end_to_end() {
    let result =
        execute_native_document("<html><body><p style=\"font-size nope\">X</p></body></html>");
    assert!(matches!(result, Err(NativeEngineError::Css(_))));
}
