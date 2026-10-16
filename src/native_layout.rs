//! AWEF-owned N2 layout model.
//!
//! Layout consumes AWEF-resolved styles and walks the DOM recursively. This
//! creates the formatting-context boundary needed for later flex/grid support.

use crate::native_css::StyleSheet;
use crate::native_dom::{NativeDocument, NodeId, NodeKind};
use crate::native_layout_taffy::{GeometryChild, GeometryMode, GeometryRequest, solve_geometry};
use crate::native_scene::{Scene, SceneCommand};
use crate::capability_ir::AcirPseudoElement;
use crate::interaction_state::InteractionSnapshot;
use crate::native_style::{
    ComputedStyle, Display, FlexDirection, ResolvedStyle, resolve_generated_pseudo_for_element,
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
    pub fill: Option<[u8;4]>,
    pub color: [u8;4],
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutTree {
    pub fragments: Vec<LayoutFragment>,
    pub content_height: f32,
    pub epoch: u64,
}

#[derive(Debug, Clone)]
enum FormattingItem {
    Dom(NodeId),
    Generated(String, f32),
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
            image: None,
        node,
        x,
        y: *y,
        width: width.max(1.0),
        height: line_height,
        font_size_px: generated.computed.font_size_px,
        text: Some(generated.content),
        fill: None,
        color: generated.computed.color,
    });
    *y += line_height;
    Ok(())
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
    first_letter_sizes: &[Option<f32>],
    sheet: &StyleSheet,
    node: NodeId,
    x: f32,
    width: f32,
    y: &mut f32,
    fragments: &mut Vec<LayoutFragment>,
    resolved_font: Option<&[u8]>,
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
    if let NodeKind::Element { tag } = &candidate.kind {
        if tag == "img" {
            let src = document.attribute(node, "src").unwrap_or_default().trim();
            let parse_dimension = |name: &str, fallback: f32| {
                document.attribute(node, name).and_then(|value| value.parse::<f32>().ok())
                    .filter(|value| value.is_finite() && *value >= 1.0)
                    .unwrap_or(fallback).min(4096.0)
            };
            let image_width = parse_dimension("width", width.min(240.0));
            let image_height = parse_dimension("height", 135.0);
            if !src.is_empty() {
                fragments.push(LayoutFragment {
                    node, x, y: *y, width: image_width, height: image_height,
                    font_size_px: style.font_size_px, text: None,
                    fill: None, color: style.color, image: Some(src.to_owned()),
                });
                *y += image_height;
            }
            return Ok(());
        }
    }
    let background_index = if style.background_color[3] > 0
        && matches!(candidate.kind, NodeKind::Element { .. })
    {
        let index = fragments.len();
        fragments.push(LayoutFragment {
            image: None,
            node, x, y: *y, width, height: 0.0,
            font_size_px: style.font_size_px,
            text: None, fill: Some(style.background_color),
            color: style.color,
        });
        Some((index, *y))
    } else { None };

    match &candidate.kind {
        NodeKind::Document => {
            for child in &candidate.children {
                layout_node(document, styles, first_letter_sizes, sheet, *child, x, width, y, fragments, resolved_font)?;
            }
        }
        NodeKind::Text(text) => {
            let base_line_height = (style.font_size_px * 1.35).ceil();
            if let Some(first_size) = first_letter_sizes.get(node).copied().flatten() {
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
            image: None,
                            node,
                            x,
                            y: *y,
                            width: width.max(1.0),
                            height: line_height,
                            font_size_px: style.font_size_px,
                            fill: None,
            color: style.color,
            text: Some(before.to_string()),
                        });
                    }

                    fragments.push(LayoutFragment {
            image: None,
                        node,
                        x,
                        y: *y,
                        width: first_width,
                        height: line_height,
                        font_size_px: first_size,
                        fill: None,
            color: style.color,
            text: Some(first.to_string()),
                    });

                    if !after.is_empty() {
                        fragments.push(LayoutFragment {
            image: None,
                            node,
                            x: x + first_width,
                            y: *y,
                            width: (width - first_width).max(1.0),
                            height: line_height,
                            font_size_px: style.font_size_px,
                            fill: None,
            color: style.color,
            text: Some(after.to_string()),
                        });
                    }

                    *y += line_height;
                } else {
                    *y += base_line_height;
                }
            } else {
                fragments.push(LayoutFragment {
            image: None,
                    node,
                    x,
                    y: *y,
                    width: width.max(1.0),
                    height: base_line_height,
                    font_size_px: style.font_size_px,
                    fill: None,
            color: style.color,
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
                        first_letter_sizes,
                        sheet,
                        *cell,
                        child_x + cell_width * index as f32,
                        cell_width.max(1.0),
                        &mut local_y,
                        fragments,
                        resolved_font,
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

            // Generated ::before/::after are anonymous flex/grid items in CSS,
            // ordered around DOM children. They must contribute to geometry.
            let mut items = Vec::new();
            for target in [AcirPseudoElement::Before] {
                if let Some(generated) = resolve_generated_pseudo_for_element(
                    document, node, &styles[node], sheet, &InteractionSnapshot::default(), target
                )? {
                    if !generated.content.is_empty() {
                        items.push(FormattingItem::Generated(generated.content,
                            generated.computed.font_size_px));
                    }
                }
            }
            for child in &candidate.children {
                if styles.get(*child)
                    .map(|resolved| resolved.computed.display != Display::None)
                    .unwrap_or(true)
                {
                    items.push(FormattingItem::Dom(*child));
                }
            }
            if let Some(generated) = resolve_generated_pseudo_for_element(
                document, node, &styles[node], sheet, &InteractionSnapshot::default(),
                AcirPseudoElement::After
            )? {
                if !generated.content.is_empty() {
                    items.push(FormattingItem::Generated(
                        generated.content, generated.computed.font_size_px));
                }
            }
            let visible_children = items.iter().filter_map(|item| {
                if let FormattingItem::Dom(id) = item { Some(*id) } else { None }
            }).collect::<Vec<_>>();

            // Explicit-font H3 path: tightly bounded single-row text-only grids.
            // No implicit host font lookup or silent semantic approximation.
            if style.display.is_grid_context() && style.grid_track_weights[..usize::from(style.grid_columns)].iter().any(|w| *w != 1000) {
                if items.iter().any(|item| matches!(item, FormattingItem::Generated(..))) {
                    return Err(crate::native_css::CssError::UnsupportedLayout(
                        "mixed grid with generated pseudo item requires intrinsic measurement".into()
                    ));
                }
                let Some(font_bytes) = resolved_font else {
                    return Err(crate::native_css::CssError::UnsupportedLayout("mixed grid needs explicitly resolved font".into()));
                };
                if visible_children.len() != usize::from(style.grid_columns) {
                    return Err(crate::native_css::CssError::UnsupportedLayout("mixed grid requires one item per track".into()));
                }
                let mut measurements = Vec::new();
                for child in &visible_children {
                    let node = document.node(*child).ok_or_else(|| crate::native_css::CssError::UnsupportedLayout("missing grid child".into()))?;
                    // No nested formatting contexts, replaced elements, or multi-text compositing.
                    if node.children.len() != 1 {
                        return Err(crate::native_css::CssError::UnsupportedLayout("grid child is not a single text run".into()));
                    }
                    let text = match document.node(node.children[0]).map(|n| &n.kind) {
                        Some(NodeKind::Text(text)) => text,
                        _ => return Err(crate::native_css::CssError::UnsupportedLayout("grid child text unresolved".into())),
                    };
                    let size = styles[*child].computed.font_size_px;
                    let measured = crate::intrinsic_text::measure_ascii_text(font_bytes, size, text);
                    if !matches!(measured, crate::intrinsic_measurement::IntrinsicMeasurement::Measured(_)) {
                        return Err(crate::native_css::CssError::UnsupportedLayout("grid child intrinsic measurement unknown".into()));
                    }
                    measurements.push(measured);
                }
                let tracks = crate::capability_ir::AcirGridTrackList {
                    columns: style.grid_track_weights[..usize::from(style.grid_columns)].iter().map(|w| {
                        if *w == 0 { crate::capability_ir::AcirGridTrack::Auto }
                        else { crate::capability_ir::AcirGridTrack::FractionMilli(*w) }
                    }).collect()
                };
                let geometry = crate::native_layout_taffy::solve_candidate_mixed_grid(
                    &tracks, &measurements, child_width, style.gap_px, 24.0
                ).map_err(crate::native_css::CssError::UnsupportedLayout)?;
                let origin_y = *y;
                let mut bottom = origin_y;
                for (child, rect) in visible_children.iter().zip(geometry.children.iter()) {
                    let mut local_y = origin_y + rect.y;
                    layout_node(document, styles, first_letter_sizes, sheet, *child,
                        child_x + rect.x, rect.width.max(1.0), &mut local_y, fragments, resolved_font)?;
                    bottom = bottom.max(local_y).max(origin_y + rect.y + rect.height);
                }
                *y = bottom.max(origin_y + geometry.height) + style.padding_bottom_px + style.margin_after_px;
                return Ok(());
            }

            let mode = if style.display.is_flex_context() {
                match style.flex_direction {
                    FlexDirection::Row => GeometryMode::FlexRow,
                    FlexDirection::Column => GeometryMode::FlexColumn,
                    FlexDirection::RowReverse => GeometryMode::FlexRowReverse,
                    FlexDirection::ColumnReverse => GeometryMode::FlexColumnReverse,
                }
            } else if style.display.is_grid_context() {
                // H3 safety boundary: geometry request still models equal fractions only.
                // Do not silently lower Auto or weighted fractions to GridColumns.
                let count = usize::from(style.grid_columns);
                if count > 12 || style.grid_track_weights[..count].iter().any(|weight| *weight != 1000) {
                    return Err(crate::native_css::CssError::UnsupportedLayout(
                        "canonical mixed grid tracks require measured live H3 geometry".into()
                    ));
                }
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
                children: items
                    .iter()
                    .map(|item| GeometryChild {
                        min_height: match item {
                            FormattingItem::Dom(child) => estimate_subtree_height(document, styles, *child),
                            FormattingItem::Generated(_, font_size) => (font_size * 1.35).ceil(),
                        },
                    })
                    .collect(),
            };
            let geometry =
                solve_geometry(&request).map_err(crate::native_css::CssError::UnsupportedLayout)?;
            let origin_y = *y;
            let mut content_bottom = origin_y;
            for (item, geometry_box) in items.iter().zip(geometry.children.iter()) {
                let mut local_y = origin_y + geometry_box.y;
                match item {
                    FormattingItem::Dom(child) => layout_node(
                        document, styles, first_letter_sizes, sheet, *child,
                        child_x + geometry_box.x, geometry_box.width.max(1.0),
                        &mut local_y, fragments, resolved_font,
                    )?,
                    FormattingItem::Generated(content, font_size) => {
                        fragments.push(LayoutFragment {
            image: None,
                            node,
                            x: child_x + geometry_box.x,
                            y: local_y,
                            width: geometry_box.width.max(1.0),
                            height: (font_size * 1.35).ceil(),
                            font_size_px: *font_size,
                            fill: None,
            color: style.color,
            text: Some(content.clone()),
                        });
                        local_y += (font_size * 1.35).ceil();
                    }
                }
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
                layout_node(document, styles, first_letter_sizes, sheet, *child, child_x, child_width, y, fragments, resolved_font)?;
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
    if let Some((index, top)) = background_index {
        fragments[index].height = (*y - top).max(0.0);
    }
    Ok(())
}

pub fn layout_document_with_styles(
    document: &NativeDocument,
    sheet: &StyleSheet,
    viewport_width: f32,
) -> Result<LayoutTree, crate::native_css::CssError> {
    layout_document_with_styles_and_font(document, sheet, viewport_width, None)
}

/// Explicitly resolved font bytes are required for bounded mixed grid geometry.
/// This is not a production font resolver and does not claim full CSS Grid support.
pub fn layout_document_with_styles_and_font(
    document: &NativeDocument,
    sheet: &StyleSheet,
    viewport_width: f32,
    resolved_font: Option<&[u8]>,
) -> Result<LayoutTree, crate::native_css::CssError> {
    let width = viewport_width.max(1.0);
    let styles = resolve_styles(document, sheet)?;
    let first_letter_sizes = first_letter_sizes(document, &styles, sheet)?;
    let mut y = 16.0f32;
    let mut fragments = Vec::new();

    layout_node(
        document,
        &styles,
        &first_letter_sizes,
        sheet,
        document.root(),
        16.0,
        (width - 32.0).max(1.0),
        &mut y,
        &mut fragments,
        resolved_font,
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
    let commands = layout.fragments.iter().filter_map(|fragment| {
        if let Some(src) = &fragment.image {
            Some(SceneCommand::Image {
                x: fragment.x, y: fragment.y, width: fragment.width,
                height: fragment.height, src: src.clone(),
            })
        } else if let Some(rgba) = fragment.fill {
            Some(SceneCommand::Fill {
                x: fragment.x, y: fragment.y, width: fragment.width,
                height: fragment.height, rgba,
            })
        } else {
            fragment.text.as_ref().map(|text| {
                if fragment.color != [0,0,0,255] {
                    SceneCommand::ColoredText {
                        x: fragment.x, y: fragment.y + fragment.font_size_px,
                        size_px: fragment.font_size_px, text: text.clone(),
                        rgba: fragment.color,
                    }
                } else {
                    SceneCommand::Text {
                        x: fragment.x, y: fragment.y + fragment.font_size_px,
                        size_px: fragment.font_size_px, text: text.clone(),
                    }
                }
            })
        }
    }).collect();
    Scene { commands, epoch: layout.epoch }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_css::parse_stylesheet;
    use crate::native_html::parse_document;

    #[test]
    fn native_image_dom_dimensions_flow_into_scene() {
        let document = parse_document(
            "<html><body><img src='/asset.png' width='120' height='80'></body></html>"
        ).unwrap();
        let layout = layout_document(&document, 400.0).unwrap();
        let scene = scene_from_layout(&layout);
        assert!(scene.commands.iter().any(|command| matches!(command,
            SceneCommand::Image { src, width, height, .. }
            if src == "/asset.png" && *width == 120.0 && *height == 80.0)));
    }

    #[test]
    fn native_background_color_reaches_painted_scene() {
        let document = parse_document("<html><body><div class='panel'>Painted</div></body></html>").unwrap();
        let sheet = parse_stylesheet(".panel { background-color: #1268ac; color: white; padding-top: 8px; }").unwrap();
        let layout = layout_document_with_styles(&document, &sheet, 360.0).unwrap();
        let scene = scene_from_layout(&layout);
        assert!(scene.commands.iter().any(|command| matches!(command,
            SceneCommand::Fill { rgba: [18, 104, 172, 255], height, .. } if *height > 8.0)));
        assert!(scene.commands.iter().any(|command| matches!(command,
            SceneCommand::ColoredText { text, rgba: [255,255,255,255], .. }
                if text == "Painted")));
    }

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

    #[cfg(feature = "taffy-layout")]
    #[test]
    fn generated_items_contribute_to_flex_and_grid_geometry() {
        for mode in ["flex", "grid"] {
            let html = "<html><body><div class='box'><span>A</span><span>B</span></div></body></html>";
            let document = parse_document(html).unwrap();
            let css = format!(".box {{ display: {mode}; gap: 8px; }} .box:before {{ content: 'First'; }} .box:after {{ content: 'Last'; }}");
            let sheet = parse_stylesheet(&css).unwrap();
            let layout = layout_document_with_styles(&document, &sheet, 320.0).unwrap();
            let texts = layout.fragments.iter().filter_map(|f| f.text.as_deref()).collect::<Vec<_>>();
            assert_eq!(texts, vec!["First", "A", "B", "Last"], "{mode}");
            assert!(layout.fragments[0].width > 0.0);
            assert!(layout.fragments[3].x.is_finite());
        }
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

    #[test]
    fn mixed_grid_semantics_reach_layout_and_fail_closed() {
        let document = parse_document(
            "<html><body><div class=\"grid\"><span>One</span><span>More words</span></div></body></html>",
        ).unwrap();
        for tracks in ["1fr auto", "auto 1fr", "2fr 1fr"] {
            let sheet = parse_stylesheet(&format!(
                ".grid {{ display: grid; grid-template-columns: {tracks}; gap: 10px; }}"
            )).unwrap();
            let styles = crate::native_style::resolve_styles(&document, &sheet).unwrap();
            let grid = document.nodes().iter()
                .find(|node| document.attribute(node.id, "class") == Some("grid"))
                .unwrap();
            assert_eq!(styles[grid.id].computed.grid_columns, 2);
            assert!(styles[grid.id].computed.grid_track_weights[..2].iter().any(|weight| *weight != 1000));
            assert!(matches!(
                layout_document_with_styles(&document, &sheet, 320.0),
                Err(crate::native_css::CssError::UnsupportedLayout(_))
            ), "{tracks} must not be rendered as equal fractions");
        }
    }

    #[cfg(feature = "taffy-layout")]
    #[test]
    fn explicitly_resolved_font_enters_bounded_native_grid_geometry() {
        let document = parse_document(
            "<html><body><div class=\"grid\"><span>aaa</span><span>bbbbbbbb</span></div></body></html>",
        ).unwrap();
        let sheet = parse_stylesheet(
            ".grid { display: grid; grid-template-columns: 1fr auto; gap: 10px; }"
        ).unwrap();
        assert!(layout_document_with_styles_and_font(&document, &sheet, 320.0, Some(b"not a font")).is_err());
        let Ok(font_path) = std::env::var("ALTRU_TEST_FONT") else { return };
        let bytes = std::fs::read(font_path).expect("font should be readable");
        let result = layout_document_with_styles_and_font(&document, &sheet, 320.0, Some(&bytes));
        assert!(result.is_ok(), "single-row measured geometry should agree with Taffy: {result:?}");
        let layout = result.unwrap();
        assert_eq!(layout.fragments.len(), 2);
        let a = &layout.fragments[0];
        let b = &layout.fragments[1];
        assert_eq!(a.text.as_deref(), Some("aaa"));
        assert_eq!(b.text.as_deref(), Some("bbbbbbbb"));
        assert!(a.x.is_finite() && b.x.is_finite() && b.x > a.x);
        let scene = scene_from_layout(&layout);
        assert_eq!(scene.commands.len(), 2);
        match (&scene.commands[0], &scene.commands[1]) {
            (
                crate::native_scene::SceneCommand::Text { x: ax, text: at, .. },
                crate::native_scene::SceneCommand::Text { x: bx, text: bt, .. },
            ) => {
                assert_eq!(at, "aaa");
                assert_eq!(bt, "bbbbbbbb");
                assert_eq!(*ax, a.x);
                assert_eq!(*bx, b.x);
            }
            _ => panic!("expected two native text scene commands"),
        }
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
