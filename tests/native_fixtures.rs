use adaptive_web_engine_fabric::native_dom::NodeKind;
use adaptive_web_engine_fabric::native_engine::execute_native_document;

const BASIC: &str = include_str!("../fixtures/native-n1/basic-text.html");
const HEADINGS: &str = include_str!("../fixtures/native-n1/headings.html");
const NESTED: &str = include_str!("../fixtures/native-n1/nested-blocks.html");
const ATTRIBUTES: &str = include_str!("../fixtures/native-n1/reject-attributes.html");

#[test]
fn native_fixture_basic_text_executes() {
    let execution = execute_native_document(BASIC).unwrap();
    assert!(execution.artifact.contains("Hello native engine"));
    assert_eq!(execution.scene.commands.len(), 1);
}

#[test]
fn native_fixture_headings_preserve_size_order() {
    let execution = execute_native_document(HEADINGS).unwrap();
    let debug = format!("{:?}", execution.scene.commands);
    assert!(debug.contains("32.0"));
    assert!(debug.contains("28.0"));
    assert!(debug.contains("16.0"));
}

#[test]
fn native_fixture_nested_blocks_execute() {
    let execution = execute_native_document(NESTED).unwrap();
    assert!(execution.artifact.contains("Nested block content"));
}

#[test]
fn native_fixture_attributes_are_preserved() {
    let execution = execute_native_document(ATTRIBUTES).unwrap();
    let paragraph = execution
        .document
        .nodes()
        .iter()
        .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
        .unwrap();

    assert_eq!(
        execution.document.attribute(paragraph.id, "class"),
        Some("not-yet-supported")
    );
}
