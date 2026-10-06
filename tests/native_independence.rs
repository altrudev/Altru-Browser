use adaptive_web_engine_fabric::native_dom::NodeKind;
use adaptive_web_engine_fabric::native_engine::execute_native_document;

#[test]
fn native_engine_is_servo_independent_for_bounded_document() {
    let execution = execute_native_document(
        "<html><body><main><h1>AWEF</h1><p>Native</p></main></body></html>",
    )
    .expect("native bounded document should execute");

    assert!(execution.native_semantics);
    assert!(!execution.production_promoted);
    assert!(!execution.scene.is_empty());
    assert!(execution.artifact.contains("AWEF"));
    assert!(execution.artifact.contains("Native"));
}

#[test]
fn native_attributes_execute_without_servo() {
    let execution = execute_native_document(
        r#"<html><body><p class="native-owned" data-test="1">x</p></body></html>"#,
    )
    .expect("native attributes should execute without Servo");

    let paragraph = execution
        .document
        .nodes()
        .iter()
        .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
        .unwrap();

    assert_eq!(
        execution.document.attribute(paragraph.id, "class"),
        Some("native-owned")
    );
    assert_eq!(
        execution.document.attribute(paragraph.id, "data-test"),
        Some("1")
    );
    assert!(execution.native_semantics);
    assert!(!execution.production_promoted);
}
