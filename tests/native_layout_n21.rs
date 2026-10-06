#[cfg(not(feature = "taffy-layout"))]
use adaptive_web_engine_fabric::native_css::CssError;
use adaptive_web_engine_fabric::native_css::parse_stylesheet;
use adaptive_web_engine_fabric::native_html::parse_document;
use adaptive_web_engine_fabric::native_layout::layout_document_with_styles;
#[cfg(feature = "taffy-layout")]
use adaptive_web_engine_fabric::native_layout::scene_from_layout;

#[cfg(not(feature = "taffy-layout"))]
#[test]
fn flex_document_fails_closed_when_geometry_adapter_is_absent() {
    let document = parse_document(
        "<html><body><div class=\"row\"><span>A</span><span>B</span></div></body></html>",
    )
    .unwrap();
    let sheet = parse_stylesheet(".row { display: flex; gap: 8px; }").unwrap();
    assert!(matches!(
        layout_document_with_styles(&document, &sheet, 400.0),
        Err(CssError::UnsupportedLayout(_))
    ));
}

#[cfg(feature = "taffy-layout")]
#[test]
fn author_and_inline_flex_semantics_produce_same_scene() {
    let author = parse_document(
        "<html><body><div class=\"row\"><span>A</span><span>B</span></div></body></html>",
    )
    .unwrap();
    let author_sheet = parse_stylesheet(".row { display: flex; gap: 8px; }").unwrap();
    let author_scene =
        scene_from_layout(&layout_document_with_styles(&author, &author_sheet, 400.0).unwrap());

    let inline = parse_document(
        "<html><body><div style=\"display: flex; gap: 8px;\"><span>A</span><span>B</span></div></body></html>",
    )
    .unwrap();
    let inline_scene = scene_from_layout(
        &layout_document_with_styles(&inline, &Default::default(), 400.0).unwrap(),
    );
    assert_eq!(author_scene.commands, inline_scene.commands);
}

#[cfg(feature = "taffy-layout")]
#[test]
fn bounded_grid_is_deterministic_and_places_second_row() {
    let document = parse_document(
        "<html><body><div class=\"grid\"><span>A</span><span>B</span><span>C</span></div></body></html>",
    )
    .unwrap();
    let sheet =
        parse_stylesheet(".grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }")
            .unwrap();
    let first = layout_document_with_styles(&document, &sheet, 400.0).unwrap();
    let second = layout_document_with_styles(&document, &sheet, 400.0).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.fragments.len(), 3);
    assert!(first.fragments[1].x > first.fragments[0].x);
    assert!(first.fragments[2].y > first.fragments[0].y);
}
