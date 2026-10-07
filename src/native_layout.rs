//! AWEF-owned N2 layout model.
//!
//! Layout consumes AWEF-resolved styles and walks the DOM recursively. This
//! creates the formatting-context boundary needed for later flex/grid support.

use crate::native_css::StyleSheet;
use crate::native_dom::{NativeDocument, NodeId, NodeKind};
use crate::native_layout_taffy::{GeometryChild, GeometryMode, GeometryRequest, solve_geometry};
use crate::native_scene::{Scene, SceneCommand};
use crate::native_style::{ComputedStyle, Display, FlexDirection, ResolvedStyle, resolve_styles};

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutFragment {
    pub node: NodeId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size_px: f32,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutTree {
    pub fragments: Vec<LayoutFragment>,
    pub content_height: f32,
    pub epoch: u64,
}

fn estimate_subtree_height(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    node: NodeId,
) -> f32 {
    let Some(candidate) = document.node(node) else {
        return 1.0;
    };
    let style = styles
        .get(node)
        .map(|resolved| resolved.computed)
        .unwrap_or_else(ComputedStyle::initial);
    if style.display == Display::None {
        return 0.0;
    }
    match &candidate.kind {
        NodeKind::Text(_) => (style.font_size_px * 1.35).ceil(),
        _ => {
            let children = candidate
                .children
                .iter()
                .map(|child| estimate_subtree_height(document, styles, *child))
                .sum::<f32>();
            (children
                + style.padding_top_px
                + style.padding_bottom_px
                + style.margin_before_px
                + style.margin_after_px)
                .max(1.0)
        }
    }
}

fn visible_children(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    node: NodeId,
) -> Vec<NodeId> {
    document
        .node(node)
        .map(|candidate| {
            candidate
                .children
                .iter()
                .copied()
                .filter(|child| {
                    styles
                        .get(*child)
                        .map(|resolved| resolved.computed.display != Display::None)
                        .unwrap_or(true)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn table_row_cells(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    row: NodeId,
) -> Vec<NodeId> {
    let children = visible_children(document, styles, row);
    if children.is_empty() {
        vec![row]
    } else {
        children
    }
}

fn table_rows(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    table: NodeId,
) -> Vec<Vec<NodeId>> {
    let mut rows = Vec::new();
    for child in visible_children(document, styles, table) {
        let display = styles
            .get(child)
            .map(|resolved| resolved.computed.display)
            .unwrap_or(Display::Inline);
        match display {
            Display::TableRow => rows.push(table_row_cells(document, styles, child)),
            Display::TableRowGroup | Display::TableHeaderGroup | Display::TableFooterGroup => {
                for row in visible_children(document, styles, child) {
                    let row_display = styles
                        .get(row)
                        .map(|resolved| resolved.computed.display)
                        .unwrap_or(Display::Inline);
                    if row_display == Display::TableRow {
                        rows.push(table_row_cells(document, styles, row));
                    } else {
                        rows.push(vec![row]);
                    }
                }
            }
            Display::TableCaption => rows.push(vec![child]),
            _ => rows.push(vec![child]),
        }
    }
    rows
}

fn layout_node(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    node: NodeId,
    x: f32,
    width: f32,
    y: &mut f32,
    fragments: &mut Vec<LayoutFragment>,
) -> Result<(), crate::native_css::CssError> {
    let Some(candidate) = document.node(node) else {
        return Ok(());
    };
    let style = styles
        .get(node)
        .map(|resolved| resolved.computed)
        .unwrap_or_else(ComputedStyle::initial);

    if style.display == Display::None {
        return Ok(());
    }

    match &candidate.kind {
        NodeKind::Document => {
            for child in &candidate.children {
                layout_node(document, styles, *child, x, width, y, fragments)?;
            }
        }
        NodeKind::Text(text) => {
            let line_height = (style.font_size_px * 1.35).ceil();
            fragments.push(LayoutFragment {
                node,
                x,
                y: *y,
                width: width.max(1.0),
                height: line_height,
                font_size_px: style.font_size_px,
                text: Some(text.clone()),
            });
            *y += line_height;
        }
        NodeKind::Element { .. }
            if matches!(style.display, Display::Table | Display::InlineTable) =>
        {
            *y += style.margin_before_px + style.padding_top_px;
            let child_x = x + style.padding_left_px;
            let child_width = (width - style.padding_left_px - style.padding_right_px).max(1.0);

            for cells in table_rows(document, styles, node) {
                if cells.is_empty() {
                    continue;
                }
                let origin_y = *y;
                let cell_width = child_width / cells.len() as f32;
                let mut row_bottom = origin_y;
                for (index, cell) in cells.iter().enumerate() {
                    let mut local_y = origin_y;
                    layout_node(
                        document,
                        styles,
                        *cell,
                        child_x + cell_width * index as f32,
                        cell_width.max(1.0),
                        &mut local_y,
                        fragments,
                    )?;
                    row_bottom = row_bottom.max(local_y);
                }
                *y = row_bottom.max(origin_y + 1.0);
            }

            *y += style.padding_bottom_px + style.margin_after_px;
        }
        NodeKind::Element { .. }
            if style.display.is_flex_context() || style.display.is_grid_context() =>
        {
            *y += style.margin_before_px + style.padding_top_px;
            let child_x = x + style.padding_left_px;
            let child_width = (width - style.padding_left_px - style.padding_right_px).max(1.0);
            let visible_children = candidate
                .children
                .iter()
                .copied()
                .filter(|child| {
                    styles
                        .get(*child)
                        .map(|resolved| resolved.computed.display != Display::None)
                        .unwrap_or(true)
                })
                .collect::<Vec<_>>();

            let mode = if style.display.is_flex_context() {
                match style.flex_direction {
                    FlexDirection::Row => GeometryMode::FlexRow,
                    FlexDirection::Column => GeometryMode::FlexColumn,
                }
            } else if style.display.is_grid_context() {
                GeometryMode::Grid {
                    columns: style.grid_columns,
                }
            } else {
                unreachable!()
            };
            let request = GeometryRequest {
                mode,
                width: child_width,
                gap: style.gap_px,
                children: visible_children
                    .iter()
                    .map(|child| GeometryChild {
                        min_height: estimate_subtree_height(document, styles, *child),
                    })
                    .collect(),
            };
            let geometry =
                solve_geometry(&request).map_err(crate::native_css::CssError::UnsupportedLayout)?;
            let origin_y = *y;
            let mut content_bottom = origin_y;
            for (child, geometry_box) in visible_children.iter().zip(geometry.children.iter()) {
                let mut local_y = origin_y + geometry_box.y;
                layout_node(
                    document,
                    styles,
                    *child,
                    child_x + geometry_box.x,
                    geometry_box.width.max(1.0),
                    &mut local_y,
                    fragments,
                )?;
                content_bottom = content_bottom
                    .max(local_y)
                    .max(origin_y + geometry_box.y + geometry_box.height);
            }
            *y = content_bottom.max(origin_y + geometry.height);
            *y += style.padding_bottom_px + style.margin_after_px;
        }
        NodeKind::Element { .. } => {
            let block = style.display.is_block_container();
            if block {
                *y += style.margin_before_px + style.padding_top_px;
            }

            let child_x = x + style.padding_left_px;
            let child_width = (width - style.padding_left_px - style.padding_right_px).max(1.0);

            for child in &candidate.children {
                layout_node(document, styles, *child, child_x, child_width, y, fragments)?;
            }

            if block {
                *y += style.padding_bottom_px + style.margin_after_px;
            }
        }
    }
    Ok(())
}

pub fn layout_document_with_styles(
    document: &NativeDocument,
    sheet: &StyleSheet,
    viewport_width: f32,
) -> Result<LayoutTree, crate::native_css::CssError> {
    let width = viewport_width.max(1.0);
    let styles = resolve_styles(document, sheet)?;
    let mut y = 16.0f32;
    let mut fragments = Vec::new();

    layout_node(
        document,
        &styles,
        document.root(),
        16.0,
        (width - 32.0).max(1.0),
        &mut y,
        &mut fragments,
    )?;

    Ok(LayoutTree {
        fragments,
        content_height: y + 16.0,
        epoch: document.mutation_epoch(),
    })
}

pub fn layout_document(
    document: &NativeDocument,
    viewport_width: f32,
) -> Result<LayoutTree, crate::native_css::CssError> {
    layout_document_with_styles(document, &StyleSheet::default(), viewport_width)
}

pub fn scene_from_layout(layout: &LayoutTree) -> Scene {
    let commands = layout
        .fragments
        .iter()
        .filter_map(|fragment| {
            fragment.text.as_ref().map(|text| SceneCommand::Text {
                x: fragment.x,
                y: fragment.y + fragment.font_size_px,
                size_px: fragment.font_size_px,
                text: text.clone(),
            })
        })
        .collect();

    Scene {
        commands,
        epoch: layout.epoch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_css::parse_stylesheet;
    use crate::native_html::parse_document;

    #[test]
    fn heading_text_inherits_heading_size() {
        let document = parse_document("<html><body><h1>Title</h1></body></html>").unwrap();
        let layout = layout_document(&document, 800.0).unwrap();
        assert_eq!(layout.fragments.len(), 1);
        assert_eq!(layout.fragments[0].font_size_px, 32.0);
    }

    #[test]
    fn author_font_size_reaches_text_fragment() {
        let document =
            parse_document("<html><body><p class=\"lead\">Hello</p></body></html>").unwrap();
        let sheet = parse_stylesheet(".lead { font-size: 22px; }").unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        assert_eq!(layout.fragments[0].font_size_px, 22.0);
    }

    #[test]
    fn padding_changes_text_origin_and_available_width() {
        let document =
            parse_document("<html><body><div class=\"box\">Hello</div></body></html>").unwrap();
        let sheet = parse_stylesheet(".box { padding-left: 12px; padding-right: 8px; }").unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        let fragment = &layout.fragments[0];
        assert_eq!(fragment.x, 28.0);
        assert_eq!(fragment.width, 268.0);
    }

    #[test]
    fn inline_block_uses_block_container_internals() {
        let document =
            parse_document("<html><body><span class=\"box\">Hello</span></body></html>").unwrap();
        let sheet = parse_stylesheet(
            ".box { display: inline-block; padding-top: 5px; padding-bottom: 7px; }",
        )
        .unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        assert_eq!(layout.fragments.len(), 1);
        assert!(layout.content_height > layout.fragments[0].height + 16.0);
    }

    #[test]
    fn display_none_hides_entire_subtree() {
        let document = parse_document(
            "<html><body><div class=\"hide\"><p>Secret</p></div><p>Visible</p></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(".hide { display: none; }").unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        assert_eq!(layout.fragments.len(), 1);
        assert_eq!(layout.fragments[0].text.as_deref(), Some("Visible"));
    }

    #[test]
    fn owned_table_context_arranges_rows_and_cells() {
        let document = parse_document(
            "<html><body><div class=\"table\"><div class=\"row\"><span class=\"cell\">A</span><span class=\"cell\">B</span></div><div class=\"row\"><span class=\"cell\">C</span></div></div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".table { display: table; } .row { display: table-row; } .cell { display: table-cell; }",
        )
        .unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        assert_eq!(layout.fragments.len(), 3);
        assert!(layout.fragments[1].x > layout.fragments[0].x);
        assert!(layout.fragments[2].y > layout.fragments[0].y);
    }

    #[cfg(feature = "taffy-layout")]
    #[test]
    fn owned_flex_style_drives_geometry_adapter() {
        let document = parse_document(
            "<html><body><div class=\"row\"><span>A</span><span>B</span></div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(".row { display: flex; gap: 10px; }").unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        assert_eq!(layout.fragments.len(), 2);
        assert!(layout.fragments[1].x > layout.fragments[0].x);
    }

    #[cfg(feature = "taffy-layout")]
    #[test]
    fn owned_grid_style_drives_geometry_adapter() {
        let document = parse_document(
            "<html><body><div class=\"grid\"><span>A</span><span>B</span><span>C</span></div></body></html>",
        ).unwrap();
        let sheet =
            parse_stylesheet(".grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }")
                .unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        assert_eq!(layout.fragments.len(), 3);
        assert!(layout.fragments[1].x > layout.fragments[0].x);
        assert!(layout.fragments[2].y > layout.fragments[0].y);
    }

    #[cfg(not(feature = "taffy-layout"))]
    #[test]
    fn flex_grid_fail_closed_without_geometry_adapter() {
        let document = parse_document(
            "<html><body><div class=\"row\"><span>A</span><span>B</span></div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(".row { display: flex; }").unwrap();
        assert!(matches!(
            layout_document_with_styles(&document, &sheet, 320.0),
            Err(crate::native_css::CssError::UnsupportedLayout(_))
        ));
    }

    #[test]
    fn scene_is_derived_from_layout_fragments() {
        let document = parse_document("<p>Hello</p>").unwrap();
        let layout = layout_document(&document, 320.0).unwrap();
        let scene = scene_from_layout(&layout);
        assert_eq!(scene.commands.len(), 1);
        assert_eq!(scene.epoch, document.mutation_epoch());
    }
}
