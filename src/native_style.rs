//! AWEF-owned computed-style and cascade model.
//!
//! N2 keeps browser semantics here. Parsing lives in native_css; renderers and
//! generic layout libraries do not decide cascade, inheritance, or variables.

use std::collections::BTreeMap;

use crate::interaction_state::InteractionSnapshot;
use crate::native_css::{
    CssError, CssGlobalKeyword, CssValue, Declaration, StyleSheet, parse_declarations,
};
use crate::native_dom::{NativeDocument, NodeId, NodeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    None,
    Block,
    Inline,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    Table,
    InlineTable,
    TableRow,
    TableCell,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableCaption,
}

impl Display {
    pub const fn is_flex_context(self) -> bool {
        matches!(self, Self::Flex | Self::InlineFlex)
    }

    pub const fn is_grid_context(self) -> bool {
        matches!(self, Self::Grid | Self::InlineGrid)
    }

    pub const fn is_block_container(self) -> bool {
        matches!(self, Self::Block | Self::InlineBlock)
    }

    pub const fn is_outer_inline(self) -> bool {
        matches!(
            self,
            Self::Inline | Self::InlineBlock | Self::InlineFlex | Self::InlineGrid | Self::InlineTable
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComputedStyle {
    pub display: Display,
    pub font_size_px: f32,
    pub margin_before_px: f32,
    pub margin_after_px: f32,
    pub padding_top_px: f32,
    pub padding_right_px: f32,
    pub padding_bottom_px: f32,
    pub padding_left_px: f32,
    pub flex_direction: FlexDirection,
    pub gap_px: f32,
    pub grid_columns: u16,
}

impl ComputedStyle {
    pub const fn initial() -> Self {
        Self {
            display: Display::Inline,
            font_size_px: 16.0,
            margin_before_px: 0.0,
            margin_after_px: 0.0,
            padding_top_px: 0.0,
            padding_right_px: 0.0,
            padding_bottom_px: 0.0,
            padding_left_px: 0.0,
            flex_direction: FlexDirection::Row,
            gap_px: 0.0,
            grid_columns: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedStyle {
    pub computed: ComputedStyle,
    pub custom_properties: BTreeMap<String, String>,
}

impl ResolvedStyle {
    fn initial() -> Self {
        Self {
            computed: ComputedStyle::initial(),
            custom_properties: BTreeMap::new(),
        }
    }
}

pub fn compute_style(kind: &NodeKind) -> ComputedStyle {
    let mut style = ComputedStyle::initial();

    let NodeKind::Element { tag } = kind else {
        return style;
    };

    match tag.as_str() {
        "html" | "body" | "main" | "article" | "section" | "header" | "footer" | "nav" | "div"
        | "p" | "blockquote" | "ul" | "ol" | "li" | "pre" => {
            style.display = Display::Block;
            style.margin_before_px = if tag == "p" { 8.0 } else { 0.0 };
            style.margin_after_px = if tag == "p" { 8.0 } else { 0.0 };
        }
        "h1" => {
            style.display = Display::Block;
            style.font_size_px = 32.0;
            style.margin_before_px = 12.0;
            style.margin_after_px = 8.0;
        }
        "h2" => {
            style.display = Display::Block;
            style.font_size_px = 28.0;
            style.margin_before_px = 10.0;
            style.margin_after_px = 7.0;
        }
        "h3" => {
            style.display = Display::Block;
            style.font_size_px = 24.0;
            style.margin_before_px = 8.0;
            style.margin_after_px = 6.0;
        }
        "script" | "style" | "head" => {
            style.display = Display::None;
        }
        _ => {}
    }

    style
}

fn ua_specifies_font_size(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Element { tag }
            if matches!(tag.as_str(), "h1" | "h2" | "h3")
    )
}

fn parse_custom_px(property: &str, value: &str) -> Result<f32, CssError> {
    let Some(number) = value.trim().strip_suffix("px") else {
        return Err(CssError::InvalidValue {
            property: property.into(),
            value: value.into(),
        });
    };
    let parsed = number
        .trim()
        .parse::<f32>()
        .map_err(|_| CssError::InvalidValue {
            property: property.into(),
            value: value.into(),
        })?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(CssError::InvalidValue {
            property: property.into(),
            value: value.into(),
        });
    }
    Ok(parsed)
}

fn resolve_value(
    declaration: &Declaration,
    custom_properties: &BTreeMap<String, String>,
    parent_font_size_px: f32,
    current_font_size_px: f32,
) -> Result<CssValue, CssError> {
    match &declaration.value {
        CssValue::Var(name) => {
            let raw = custom_properties
                .get(name)
                .ok_or_else(|| CssError::UnresolvedCustomProperty(name.clone()))?;
            match declaration.property.as_str() {
                "font-size" | "margin-top" | "margin-bottom" | "padding-top" | "padding-right"
                | "padding-bottom" | "padding-left" | "gap" => {
                    Ok(CssValue::Px(parse_custom_px(&declaration.property, raw)?))
                }
                _ => Err(CssError::InvalidValue {
                    property: declaration.property.clone(),
                    value: format!("var({name})"),
                }),
            }
        }
        CssValue::RelativeLength(value) => Ok(CssValue::Px(
            value.resolve_px(parent_font_size_px, current_font_size_px),
        )),
        other => Ok(other.clone()),
    }
}

fn property_inherits_by_default(property: &str) -> bool {
    matches!(property, "font-size")
}

fn assign_property_from(
    target: &mut ComputedStyle,
    source: &ComputedStyle,
    property: &str,
) {
    match property {
        "display" => target.display = source.display,
        "flex-direction" => target.flex_direction = source.flex_direction,
        "grid-template-columns" => target.grid_columns = source.grid_columns,
        "gap" => target.gap_px = source.gap_px,
        "font-size" => target.font_size_px = source.font_size_px,
        "margin-top" => target.margin_before_px = source.margin_before_px,
        "margin-bottom" => target.margin_after_px = source.margin_after_px,
        "padding-top" => target.padding_top_px = source.padding_top_px,
        "padding-right" => target.padding_right_px = source.padding_right_px,
        "padding-bottom" => target.padding_bottom_px = source.padding_bottom_px,
        "padding-left" => target.padding_left_px = source.padding_left_px,
        _ => {}
    }
}

fn apply_global_keyword(
    style: &mut ComputedStyle,
    parent: &ComputedStyle,
    property: &str,
    keyword: CssGlobalKeyword,
) {
    let initial = ComputedStyle::initial();
    match keyword {
        CssGlobalKeyword::Inherit => assign_property_from(style, parent, property),
        CssGlobalKeyword::Initial => assign_property_from(style, &initial, property),
        CssGlobalKeyword::Unset => {
            if property_inherits_by_default(property) {
                assign_property_from(style, parent, property);
            } else {
                assign_property_from(style, &initial, property);
            }
        }
    }
}

fn apply_value(
    style: &mut ComputedStyle,
    parent: &ComputedStyle,
    property: &str,
    value: &CssValue,
) {
    if let CssValue::Global(keyword) = value {
        apply_global_keyword(style, parent, property, *keyword);
        return;
    }

    match (property, value) {
        ("display", CssValue::Display(value)) => {
            style.display = match value.as_str() {
                "none" => Display::None,
                "block" => Display::Block,
                "inline" => Display::Inline,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "inline-flex" => Display::InlineFlex,
                "grid" => Display::Grid,
                "inline-grid" => Display::InlineGrid,
                "table" => Display::Table,
                "inline-table" => Display::InlineTable,
                "table-row" => Display::TableRow,
                "table-cell" => Display::TableCell,
                "table-row-group" => Display::TableRowGroup,
                "table-header-group" => Display::TableHeaderGroup,
                "table-footer-group" => Display::TableFooterGroup,
                "table-caption" => Display::TableCaption,
                _ => style.display,
            };
        }
        ("flex-direction", CssValue::Keyword(value)) => {
            style.flex_direction = match value.as_str() {
                "column" => FlexDirection::Column,
                _ => FlexDirection::Row,
            };
        }
        ("grid-template-columns", CssValue::GridColumns(value)) => style.grid_columns = *value,
        ("gap", CssValue::Px(value)) => style.gap_px = *value,
        ("font-size", CssValue::Px(value)) => style.font_size_px = *value,
        ("margin-top", CssValue::Px(value)) => style.margin_before_px = *value,
        ("margin-bottom", CssValue::Px(value)) => style.margin_after_px = *value,
        ("padding-top", CssValue::Px(value)) => style.padding_top_px = *value,
        ("padding-right", CssValue::Px(value)) => style.padding_right_px = *value,
        ("padding-bottom", CssValue::Px(value)) => style.padding_bottom_px = *value,
        ("padding-left", CssValue::Px(value)) => style.padding_left_px = *value,
        _ => {}
    }
}

fn resolve_element_style(
    document: &NativeDocument,
    node: NodeId,
    parent_style: &ResolvedStyle,
    sheet: &StyleSheet,
    interaction: &InteractionSnapshot,
) -> Result<ResolvedStyle, CssError> {
    let Some(candidate) = document.node(node) else {
        return Ok(ResolvedStyle::initial());
    };

    let mut computed = compute_style(&candidate.kind);
    if !ua_specifies_font_size(&candidate.kind) {
        computed.font_size_px = parent_style.computed.font_size_px;
    }

    let mut declarations = Vec::new();
    let mut matching = sheet
        .rules
        .iter()
        .filter(|rule| {
            rule.selector
                .matches_with_interaction(document, node, interaction)
        })
        .collect::<Vec<_>>();
    matching.sort_by_key(|rule| (rule.selector.specificity(), rule.order));

    for rule in matching {
        declarations.extend(rule.declarations.iter().cloned());
    }

    if let Some(inline) = document.attribute(node, "style") {
        declarations.extend(parse_declarations(inline)?);
    }

    let mut custom_properties = parent_style.custom_properties.clone();
    for declaration in &declarations {
        if declaration.property.starts_with("--")
            && let CssValue::Raw(value) = &declaration.value
        {
            custom_properties.insert(declaration.property.clone(), value.clone());
        }
    }

    // Resolve font-size first because other `em` lengths depend on the element's
    // final computed font size, not declaration order.
    for declaration in declarations.iter().filter(|declaration| declaration.property == "font-size") {
        let value = resolve_value(
            declaration,
            &custom_properties,
            parent_style.computed.font_size_px,
            computed.font_size_px,
        )?;
        apply_value(
            &mut computed,
            &parent_style.computed,
            &declaration.property,
            &value,
        );
    }

    for declaration in &declarations {
        if declaration.property.starts_with("--") || declaration.property == "font-size" {
            continue;
        }
        let value = resolve_value(
            declaration,
            &custom_properties,
            parent_style.computed.font_size_px,
            computed.font_size_px,
        )?;
        apply_value(
            &mut computed,
            &parent_style.computed,
            &declaration.property,
            &value,
        );
    }

    Ok(ResolvedStyle {
        computed,
        custom_properties,
    })
}

pub fn resolve_styles_with_interaction(
    document: &NativeDocument,
    sheet: &StyleSheet,
    interaction: &InteractionSnapshot,
) -> Result<Vec<ResolvedStyle>, CssError> {
    let mut styles = vec![ResolvedStyle::initial(); document.nodes().len()];

    fn walk(
        document: &NativeDocument,
        sheet: &StyleSheet,
        interaction: &InteractionSnapshot,
        node: NodeId,
        parent_style: &ResolvedStyle,
        styles: &mut [ResolvedStyle],
    ) -> Result<(), CssError> {
        let Some(candidate) = document.node(node) else {
            return Ok(());
        };

        let style = match &candidate.kind {
            NodeKind::Document => parent_style.clone(),
            NodeKind::Text(_) => {
                let mut inherited = ResolvedStyle::initial();
                inherited.computed.font_size_px = parent_style.computed.font_size_px;
                inherited.computed.display = parent_style.computed.display;
                inherited.custom_properties = parent_style.custom_properties.clone();
                inherited
            }
            NodeKind::Element { .. } => {
                resolve_element_style(document, node, parent_style, sheet, interaction)?
            }
        };
        styles[node] = style.clone();

        for child in &candidate.children {
            walk(document, sheet, interaction, *child, &style, styles)?;
        }
        Ok(())
    }

    let initial = ResolvedStyle::initial();
    walk(
        document,
        sheet,
        interaction,
        document.root(),
        &initial,
        &mut styles,
    )?;
    Ok(styles)
}

pub fn resolve_styles(
    document: &NativeDocument,
    sheet: &StyleSheet,
) -> Result<Vec<ResolvedStyle>, CssError> {
    resolve_styles_with_interaction(document, sheet, &InteractionSnapshot::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_css::parse_stylesheet;
    use crate::native_html::parse_document;

    #[test]
    fn heading_style_is_engine_owned() {
        let style = compute_style(&NodeKind::Element { tag: "h1".into() });
        assert_eq!(style.display, Display::Block);
        assert_eq!(style.font_size_px, 32.0);
    }

    #[test]
    fn hidden_elements_are_not_layout_visible() {
        let style = compute_style(&NodeKind::Element {
            tag: "script".into(),
        });
        assert_eq!(style.display, Display::None);
    }

    #[test]
    fn table_display_keywords_preserve_explicit_roles() {
        let document = parse_document(
            "<html><body><div class=\"t\"><div class=\"r\"><span class=\"c\">X</span></div></div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".t { display: table; } .r { display: table-row; } .c { display: table-cell; }",
        )
        .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();
        let lookup = |class: &str| {
            document
                .nodes()
                .iter()
                .find(|node| document.attribute(node.id, "class") == Some(class))
                .unwrap()
                .id
        };
        assert_eq!(styles[lookup("t")].computed.display, Display::Table);
        assert_eq!(styles[lookup("r")].computed.display, Display::TableRow);
        assert_eq!(styles[lookup("c")].computed.display, Display::TableCell);
    }

    #[test]
    fn composite_display_keywords_preserve_outer_inline_semantics() {
        let document = parse_document(
            "<html><body><div class=\"ib\">A</div><div class=\"if\">B</div><div class=\"ig\">C</div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".ib { display: inline-block; } .if { display: inline-flex; } .ig { display: inline-grid; }",
        )
        .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();

        let mut matched = document
            .nodes()
            .iter()
            .filter_map(|node| {
                document
                    .attribute(node.id, "class")
                    .map(|class| (class.to_string(), styles[node.id].computed.display))
            })
            .collect::<Vec<_>>();
        matched.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(matched[0].1, Display::InlineBlock);
        assert_eq!(matched[1].1, Display::InlineFlex);
        assert_eq!(matched[2].1, Display::InlineGrid);
        assert!(matched.iter().all(|(_, display)| display.is_outer_inline()));
    }

    #[test]
    fn id_specificity_beats_class_and_tag() {
        let document =
            parse_document("<html><body><p id=\"hero\" class=\"lead\">Hello</p></body></html>")
                .unwrap();
        let sheet = parse_stylesheet(
            "p { font-size: 18px; } .lead { font-size: 20px; } #hero { font-size: 24px; }",
        )
        .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();
        let paragraph = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        assert_eq!(styles[paragraph.id].computed.font_size_px, 24.0);
    }

    #[test]
    fn font_size_inherits_from_parent() {
        let document =
            parse_document("<html><body><div class=\"large\"><span>X</span></div></body></html>")
                .unwrap();
        let sheet = parse_stylesheet(".large { font-size: 23px; }").unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();
        let span = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "span"))
            .unwrap();
        assert_eq!(styles[span.id].computed.font_size_px, 23.0);
    }

    #[test]
    fn inline_style_wins_over_author_rule() {
        let document = parse_document(
            "<html><body><p class=\"lead\" style=\"font-size: 26px\">X</p></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(".lead { font-size: 20px; }").unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();
        let paragraph = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        assert_eq!(styles[paragraph.id].computed.font_size_px, 26.0);
    }

    #[test]
    fn global_keywords_resolve_per_property_semantics() {
        let document = parse_document(
            "<html><body><div class=\"parent\"><span class=\"inherit\">I</span><span class=\"initial\">N</span><span class=\"unset\">U</span></div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".parent { font-size: 20px; display: block; padding-left: 9px; }              .inherit { font-size: inherit; padding-left: inherit; }              .initial { font-size: initial; padding-left: initial; }              .unset { font-size: unset; padding-left: unset; }",
        )
        .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();

        let find = |class: &str| {
            document
                .nodes()
                .iter()
                .find(|node| document.attribute(node.id, "class") == Some(class))
                .unwrap()
                .id
        };

        let inherited = &styles[find("inherit")].computed;
        assert_eq!(inherited.font_size_px, 20.0);
        assert_eq!(inherited.padding_left_px, 9.0);

        let initial = &styles[find("initial")].computed;
        assert_eq!(initial.font_size_px, 16.0);
        assert_eq!(initial.padding_left_px, 0.0);

        let unset = &styles[find("unset")].computed;
        assert_eq!(unset.font_size_px, 20.0);
        assert_eq!(unset.padding_left_px, 0.0);
    }

    #[test]
    fn current_font_em_lengths_ignore_declaration_order() {
        let document = parse_document(
            "<html><body><div class=\"box\">X</div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".box { margin-bottom: 1em; padding-left: .5em; font-size: 20px; }",
        )
        .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();
        let node = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("box"))
            .unwrap();
        assert_eq!(styles[node.id].computed.font_size_px, 20.0);
        assert_eq!(styles[node.id].computed.margin_after_px, 20.0);
        assert_eq!(styles[node.id].computed.padding_left_px, 10.0);
    }

    #[test]
    fn em_font_size_resolves_against_parent_font_size() {
        let document = parse_document(
            "<html><body><div class=\"parent\"><span class=\"child\">X</span></div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".parent { font-size: 20px; } .child { font-size: 1.5em; }",
        )
        .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();

        let span = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "span"))
            .unwrap();
        assert_eq!(styles[span.id].computed.font_size_px, 30.0);
    }

    #[test]
    fn interaction_snapshot_changes_computed_style_only_when_state_is_true() {
        let document = parse_document(
            "<html><body><a class=\"skip\">Skip</a></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".skip { font-size: 16px; } .skip:focus { font-size: 24px; }",
        )
        .unwrap();
        let link = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("skip"))
            .unwrap();

        let idle = resolve_styles(&document, &sheet).unwrap();
        assert_eq!(idle[link.id].computed.font_size_px, 16.0);

        let focused = resolve_styles_with_interaction(
            &document,
            &sheet,
            &InteractionSnapshot {
                focused_node: Some(link.id),
                ..InteractionSnapshot::default()
            },
        )
        .unwrap();
        assert_eq!(focused[link.id].computed.font_size_px, 24.0);
    }

    #[test]
    fn custom_property_inherits_and_resolves() {
        let document = parse_document(
            "<html><body><div class=\"scope\"><span class=\"child\">X</span></div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(".scope { --size: 21px; } .child { font-size: var(--size); }")
            .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();
        let span = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "span"))
            .unwrap();
        assert_eq!(styles[span.id].computed.font_size_px, 21.0);
        assert_eq!(
            styles[span.id]
                .custom_properties
                .get("--size")
                .map(String::as_str),
            Some("21px")
        );
    }

    #[test]
    fn owned_flex_grid_semantics_reach_computed_style() {
        let document = parse_document(
            "<html><body><div class=\"flex\">A</div><div class=\"grid\">B</div></body></html>",
        )
        .unwrap();
        let sheet = parse_stylesheet(
            ".flex { display: flex; flex-direction: column; gap: 9px; } .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }",
        )
        .unwrap();
        let styles = resolve_styles(&document, &sheet).unwrap();
        let flex = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("flex"))
            .unwrap();
        let grid = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("grid"))
            .unwrap();
        assert_eq!(styles[flex.id].computed.display, Display::Flex);
        assert_eq!(
            styles[flex.id].computed.flex_direction,
            FlexDirection::Column
        );
        assert_eq!(styles[flex.id].computed.gap_px, 9.0);
        assert_eq!(styles[grid.id].computed.display, Display::Grid);
        assert_eq!(styles[grid.id].computed.grid_columns, 2);
        assert_eq!(styles[grid.id].computed.gap_px, 7.0);
    }

    #[test]
    fn unresolved_custom_property_fails_closed() {
        let document = parse_document("<html><body><p class=\"x\">X</p></body></html>").unwrap();
        let sheet = parse_stylesheet(".x { font-size: var(--missing); }").unwrap();
        assert!(matches!(
            resolve_styles(&document, &sheet),
            Err(CssError::UnresolvedCustomProperty(name)) if name == "--missing"
        ));
    }
}
