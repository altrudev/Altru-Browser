//! AWEF-owned N2 layout model.
//!
//! Layout consumes AWEF-resolved styles and walks the DOM recursively. This
//! creates the formatting-context boundary needed for later flex/grid support.

use crate::native_css::StyleSheet;
use crate::native_dom::{NativeDocument, NodeId, NodeKind};
use crate::intrinsic_measurement::IntrinsicMeasurement;
use crate::intrinsic_text::measure_ascii_text;
use crate::native_layout_taffy::{
    GeometryAlign, GeometryChild, GeometryJustify, GeometryMode, GeometryRequest, solve_geometry,
};
use crate::native_scene::{Scene, SceneCommand};
use crate::capability_ir::AcirPseudoElement;
use crate::interaction_state::InteractionSnapshot;
use crate::native_style::{
    AlignItems, ComputedStyle, Display, FlexDirection, JustifyContent, ResolvedStyle,
    resolve_generated_pseudo_for_element,
    resolve_pseudo_style_for_element, resolve_styles,
};

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

fn first_text_descendant(document: &NativeDocument, node: NodeId) -> Option<NodeId> {
    let candidate = document.node(node)?;
    for child in &candidate.children {
        let child_node = document.node(*child)?;
        match &child_node.kind {
            NodeKind::Text(text) if !text.trim().is_empty() => return Some(*child),
            NodeKind::Element { .. } => {
                if let Some(found) = first_text_descendant(document, *child) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn first_letter_sizes(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    sheet: &StyleSheet,
) -> Result<Vec<Option<f32>>, crate::native_css::CssError> {
    let mut result = vec![None; document.nodes().len()];
    let interaction = InteractionSnapshot::default();

    for node in document.nodes() {
        if !matches!(node.kind, NodeKind::Element { .. }) {
            continue;
        }
        let Some(text_node) = first_text_descendant(document, node.id) else {
            continue;
        };
        let Some(base_style) = styles.get(node.id) else {
            continue;
        };
        if let Some(pseudo) = resolve_pseudo_style_for_element(
            document,
            node.id,
            base_style,
            sheet,
            &interaction,
            AcirPseudoElement::FirstLetter,
        )? {
            result[text_node] = Some(pseudo.computed.font_size_px);
        }
    }

    Ok(result)
}

fn emit_generated_text(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    sheet: &StyleSheet,
    node: NodeId,
    target: AcirPseudoElement,
    x: f32,
    width: f32,
    y: &mut f32,
    fragments: &mut Vec<LayoutFragment>,
) -> Result<(), crate::native_css::CssError> {
    let Some(base_style) = styles.get(node) else {
        return Ok(());
    };
    let Some(generated) = resolve_generated_pseudo_for_element(
        document,
        node,
        base_style,
        sheet,
        &InteractionSnapshot::default(),
        target,
    )? else {
        return Ok(());
    };

    if generated.content.is_empty() {
        return Ok(());
    }

    let line_height = (generated.computed.font_size_px * 1.35).ceil();
    fragments.push(LayoutFragment {
        node,
        x,
        y: *y,
        width: width.max(1.0),
        height: line_height,
        font_size_px: generated.computed.font_size_px,
        text: Some(generated.content),
    });
    *y += line_height;
    Ok(())
}

/// Supplies intrinsic text widths from real font metrics.
///
/// Layout never guesses a text width. Without a measurer, or when a text can't be
/// measured, auto-sized flex items keep the engine's original behavior of sharing
/// the free space equally.
pub trait TextMeasurer {
    /// Max-content inline width in CSS pixels, or `None` if unmeasurable.
    fn max_content_px(&self, text: &str, font_size_px: f32) -> Option<f32>;
}

/// Measures with one resolved font face (`intrinsic_text`, ASCII only).
pub struct FontFaceMeasurer<'a> {
    pub font_bytes: &'a [u8],
}

impl TextMeasurer for FontFaceMeasurer<'_> {
    fn max_content_px(&self, text: &str, font_size_px: f32) -> Option<f32> {
        match measure_ascii_text(self.font_bytes, font_size_px, text) {
            IntrinsicMeasurement::Measured(size) => Some(size.max_content_px),
            IntrinsicMeasurement::Unknown(_) => None,
        }
    }
}

struct LayoutContext<'a> {
    first_letter_sizes: Vec<Option<f32>>,
    measurer: Option<&'a dyn TextMeasurer>,
}

/// Max-content width of a subtree, or `None` if any text in it can't be measured.
fn max_content_width(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    measurer: &dyn TextMeasurer,
    node: NodeId,
) -> Option<f32> {
    let candidate = document.node(node)?;
    let style = styles
        .get(node)
        .map(|resolved| resolved.computed)
        .unwrap_or_else(ComputedStyle::initial);
    if style.display == Display::None {
        return Some(0.0);
    }
    match &candidate.kind {
        NodeKind::Text(text) => {
            let collapsed = text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                Some(0.0)
            } else {
                measurer.max_content_px(&collapsed, style.font_size_px)
            }
        }
        _ => {
            if let crate::native_style::SizeValue::Px(px) = style.width {
                return Some(px);
            }
            let children = candidate
                .children
                .iter()
                .filter(|child| {
                    styles
                        .get(**child)
                        .is_none_or(|resolved| resolved.computed.display != Display::None)
                })
                .map(|child| {
                    let width = max_content_width(document, styles, measurer, *child)?;
                    let display = styles
                        .get(*child)
                        .map(|resolved| resolved.computed.display)
                        .unwrap_or(Display::Inline);
                    Some((width, display))
                })
                .collect::<Option<Vec<_>>>()?;
            let row = style.display.is_flex_context() && style.flex_direction == FlexDirection::Row;
            let any_block = children
                .iter()
                .any(|(_, display)| !display.is_outer_inline());
            let content = if row {
                children.iter().map(|(w, _)| w).sum::<f32>()
                    + style.gap_px * children.len().saturating_sub(1) as f32
            } else if any_block && !matches!(candidate.kind, NodeKind::Document) {
                children.iter().map(|(w, _)| *w).fold(0.0, f32::max)
            } else {
                children.iter().map(|(w, _)| w).sum::<f32>()
            };
            Some(content + style.padding_left_px + style.padding_right_px)
        }
    }
}

/// How a flex/grid item enters the owned geometry request.
fn geometry_child(
    document: &NativeDocument,
    styles: &[ResolvedStyle],
    ctx: &LayoutContext<'_>,
    mode: GeometryMode,
    container_width: f32,
    child: NodeId,
) -> GeometryChild {
    let min_height = estimate_subtree_height(document, styles, child);
    if matches!(mode, GeometryMode::Grid { .. }) {
        return GeometryChild::shared(min_height);
    }
    let style = styles
        .get(child)
        .map(|resolved| resolved.computed)
        .unwrap_or_else(ComputedStyle::initial);
    let width = style.width.resolve(container_width);
    let min_width = style.min_width.resolve(container_width).unwrap_or(0.0);
    let max_width = style.max_width.resolve(container_width);
    let flex_basis = style.flex_basis.resolve(container_width);
    let author_sized = width.is_some() || flex_basis.is_some() || style.flex_grow.is_some();
    if author_sized {
        return GeometryChild {
            min_height,
            width,
            min_width,
            max_width,
            flex_grow: style.flex_grow.unwrap_or(0.0),
            flex_shrink: style.flex_shrink,
            flex_basis,
        };
    }
    if mode == GeometryMode::FlexRow
        && let Some(measurer) = ctx.measurer
        && let Some(content) = max_content_width(document, styles, measurer, child)
    {
        return GeometryChild {
            min_height,
            width: None,
            min_width,
            max_width,
            flex_grow: 0.0,
            flex_shrink: style.flex_shrink,
            flex_basis: Some(content),
        };
    }
    GeometryChild::shared(min_height)
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
    ctx: &LayoutContext<'_>,
    sheet: &StyleSheet,
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
                layout_node(document, styles, ctx, sheet, *child, x, width, y, fragments)?;
            }
        }
        NodeKind::Text(text) => {
            let base_line_height = (style.font_size_px * 1.35).ceil();
            if let Some(first_size) = ctx.first_letter_sizes.get(node).copied().flatten() {
                if let Some((byte_index, ch)) = text.char_indices().find(|(_, ch)| !ch.is_whitespace()) {
                    let ch_len = ch.len_utf8();
                    let before = &text[..byte_index];
                    let first = &text[byte_index..byte_index + ch_len];
                    let after = &text[byte_index + ch_len..];
                    let first_width = (first_size * 0.62).max(1.0);
                    let first_line_height = (first_size * 1.35).ceil();
                    let line_height = base_line_height.max(first_line_height);

                    if !before.is_empty() {
                        fragments.push(LayoutFragment {
                            node,
                            x,
                            y: *y,
                            width: width.max(1.0),
                            height: line_height,
                            font_size_px: style.font_size_px,
                            text: Some(before.to_string()),
                        });
                    }

                    fragments.push(LayoutFragment {
                        node,
                        x,
                        y: *y,
                        width: first_width,
                        height: line_height,
                        font_size_px: first_size,
                        text: Some(first.to_string()),
                    });

                    if !after.is_empty() {
                        fragments.push(LayoutFragment {
                            node,
                            x: x + first_width,
                            y: *y,
                            width: (width - first_width).max(1.0),
                            height: line_height,
                            font_size_px: style.font_size_px,
                            text: Some(after.to_string()),
                        });
                    }

                    *y += line_height;
                } else {
                    *y += base_line_height;
                }
            } else {
                fragments.push(LayoutFragment {
                    node,
                    x,
                    y: *y,
                    width: width.max(1.0),
                    height: base_line_height,
                    font_size_px: style.font_size_px,
                    text: Some(text.clone()),
                });
                *y += base_line_height;
            }
        }
        NodeKind::Element { .. }
            if matches!(style.display, Display::Table | Display::InlineTable) =>
        {
            *y += style.margin_before_px + style.padding_top_px;
            let child_x = x + style.padding_left_px;
            let child_width = (width - style.padding_left_px - style.padding_right_px).max(1.0);

            for target in [AcirPseudoElement::Before, AcirPseudoElement::After] {
                if resolve_generated_pseudo_for_element(
                    document,
                    node,
                    &styles[node],
                    sheet,
                    &InteractionSnapshot::default(),
                    target,
                )?
                .is_some()
                {
                    return Err(crate::native_css::CssError::UnsupportedLayout(
                        "generated pseudo content inside table formatting context".into(),
                    ));
                }
            }

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
                        ctx,
                        sheet,
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

            for target in [AcirPseudoElement::Before, AcirPseudoElement::After] {
                if resolve_generated_pseudo_for_element(
                    document,
                    node,
                    &styles[node],
                    sheet,
                    &InteractionSnapshot::default(),
                    target,
                )?
                .is_some()
                {
                    return Err(crate::native_css::CssError::UnsupportedLayout(
                        "generated pseudo content inside flex/grid formatting context".into(),
                    ));
                }
            }

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
                justify: match style.justify_content {
                    JustifyContent::Start => GeometryJustify::Start,
                    JustifyContent::Center => GeometryJustify::Center,
                    JustifyContent::End => GeometryJustify::End,
                    JustifyContent::SpaceBetween => GeometryJustify::SpaceBetween,
                },
                align: match style.align_items {
                    AlignItems::Stretch => GeometryAlign::Stretch,
                    AlignItems::Start => GeometryAlign::Start,
                    AlignItems::Center => GeometryAlign::Center,
                    AlignItems::End => GeometryAlign::End,
                },
                children: visible_children
                    .iter()
                    .map(|child| geometry_child(document, styles, ctx, mode, child_width, *child))
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
                    ctx,
                    sheet,
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

            emit_generated_text(
                document,
                styles,
                sheet,
                node,
                AcirPseudoElement::Before,
                child_x,
                child_width,
                y,
                fragments,
            )?;

            for child in &candidate.children {
                layout_node(document, styles, ctx, sheet, *child, child_x, child_width, y, fragments)?;
            }

            emit_generated_text(
                document,
                styles,
                sheet,
                node,
                AcirPseudoElement::After,
                child_x,
                child_width,
                y,
                fragments,
            )?;

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
    layout_document_with_measurer(document, sheet, viewport_width, None)
}

/// Layout with intrinsic text widths from `measurer`, so auto-sized flex items
/// take their content width instead of sharing space equally.
pub fn layout_document_with_measurer(
    document: &NativeDocument,
    sheet: &StyleSheet,
    viewport_width: f32,
    measurer: Option<&dyn TextMeasurer>,
) -> Result<LayoutTree, crate::native_css::CssError> {
    let width = viewport_width.max(1.0);
    let styles = resolve_styles(document, sheet)?;
    let ctx = LayoutContext {
        first_letter_sizes: first_letter_sizes(document, &styles, sheet)?,
        measurer,
    };
    let mut y = 16.0f32;
    let mut fragments = Vec::new();

    layout_node(
        document,
        &styles,
        &ctx,
        sheet,
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
    fn generated_before_and_after_content_become_native_fragments() {
        let document =
            parse_document("<html><body><p class=\"card\">Body</p></body></html>").unwrap();
        let sheet = parse_stylesheet(
            ".card:before { content: \"Before\"; } .card:after { content: \"After\"; }",
        )
        .unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        let texts = layout
            .fragments
            .iter()
            .filter_map(|fragment| fragment.text.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["Before", "Body", "After"]);
    }

    #[test]
    fn first_letter_rule_splits_rendered_text_fragment() {
        let document =
            parse_document("<html><body><p class=\"drop\">Hello</p></body></html>").unwrap();
        let sheet = parse_stylesheet(
            ".drop { font-size: 20px; } .drop:first-letter { font-size: 40px; }",
        )
        .unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
        assert!(layout.fragments.len() >= 2);
        assert_eq!(layout.fragments[0].text.as_deref(), Some("H"));
        assert_eq!(layout.fragments[0].font_size_px, 40.0);
        assert_eq!(layout.fragments[1].text.as_deref(), Some("ello"));
        assert_eq!(layout.fragments[1].font_size_px, 20.0);
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
