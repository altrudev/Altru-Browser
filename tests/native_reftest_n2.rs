use adaptive_web_engine_fabric::native_engine::execute_native_document;

fn scene_debug(input: &str) -> String {
    let execution = execute_native_document(input).unwrap();
    format!("{:?}", execution.scene.commands)
}

#[test]
fn equivalent_author_and_inline_font_size_render_the_same_scene() {
    let author = scene_debug(
        "<html><head><style>.lead { font-size: 24px; }</style></head><body><p class=\"lead\">Reference</p></body></html>",
    );
    let inline =
        scene_debug("<html><body><p style=\"font-size: 24px\">Reference</p></body></html>");
    assert_eq!(author, inline);
}

#[test]
fn ignored_unknown_property_does_not_change_scene() {
    let baseline = scene_debug(
        "<html><head><style>p { font-size: 18px; }</style></head><body><p>X</p></body></html>",
    );
    let with_unknown = scene_debug(
        "<html><head><style>p { future-property: 999; font-size: 18px; }</style></head><body><p>X</p></body></html>",
    );
    assert_eq!(baseline, with_unknown);
}

#[test]
fn higher_specificity_changes_reference_scene() {
    let class_only = scene_debug(
        "<html><head><style>.lead { font-size: 20px; }</style></head><body><p id=\"hero\" class=\"lead\">X</p></body></html>",
    );
    let id_override = scene_debug(
        "<html><head><style>.lead { font-size: 20px; } #hero { font-size: 25px; }</style></head><body><p id=\"hero\" class=\"lead\">X</p></body></html>",
    );
    assert_ne!(class_only, id_override);
}
