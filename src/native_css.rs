//! AWEF-owned N2 CSS parser and selector model.
//!
//! This is deliberately a bounded CSS subset. Unsupported selectors fail
//! closed; unknown properties follow CSS error handling and are ignored.

use crate::capability_compiler::{
    CapabilityCompilerError, TranslationReceipt, compile_current_font_length_capability,
    compile_font_size_capability,
    compile_selector_boolean_capability, compile_selector_capability,
    compile_selector_list_capability, compile_selector_state_capability,
    compile_selector_structural_capability, compile_stylesheet_capabilities,
    compile_zero_length_capability,
};
use crate::capability_ir::{
    AcirInteractionPredicate, AcirRelativeLength, AcirSelectorBoolean, AcirSelectorRelation,
    AcirStructuralPredicate, CapabilityEnvironment,
};
use crate::interaction_state::InteractionSnapshot;
use crate::native_dom::{NativeDocument, NodeId, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssError {
    MalformedRule,
    UnsupportedSelector(String),
    MalformedDeclaration(String),
    InvalidValue { property: String, value: String },
    UnresolvedCustomProperty(String),
    UnsupportedLayout(String),
    CapabilityCompiler(CapabilityCompilerError),
}

impl From<CapabilityCompilerError> for CssError {
    fn from(value: CapabilityCompilerError) -> Self {
        Self::CapabilityCompiler(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeOperator {
    Equals,
    IncludesWord,
    DashPrefix,
    Prefix,
    Suffix,
    Contains,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    pub name: String,
    pub operator: AttributeOperator,
    pub value: String,
    pub case_insensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<AttributeSelector>,
    pub root: bool,
    pub states: Vec<AcirInteractionPredicate>,
    pub structural: Vec<AcirStructuralPredicate>,
    pub any_of: Vec<Selector>,
    pub where_any_of: Vec<Selector>,
    pub none_of: Vec<Selector>,
    pub ancestor: Option<(AcirSelectorRelation, Box<Selector>)>,
}

impl Selector {
    pub fn specificity(&self) -> (u16, u16, u16) {
        let base = (
            u16::from(self.id.is_some()),
            (self.classes.len()
                + self.attributes.len()
                + self.states.len()
                + self.structural.len()
                + usize::from(self.root))
                .min(u16::MAX as usize) as u16,
            u16::from(self.tag.is_some()),
        );
        let any_nested = self
            .any_of
            .iter()
            .map(Selector::specificity)
            .max()
            .unwrap_or((0, 0, 0));
        let none_nested = self
            .none_of
            .iter()
            .map(Selector::specificity)
            .max()
            .unwrap_or((0, 0, 0));
        let nested = (
            any_nested.0.saturating_add(none_nested.0),
            any_nested.1.saturating_add(none_nested.1),
            any_nested.2.saturating_add(none_nested.2),
        );
        let ancestor = self
            .ancestor
            .as_ref()
            .map(|(_, selector)| selector.specificity())
            .unwrap_or((0, 0, 0));
        (
            base.0
                .saturating_add(nested.0)
                .saturating_add(ancestor.0),
            base.1
                .saturating_add(nested.1)
                .saturating_add(ancestor.1),
            base.2
                .saturating_add(nested.2)
                .saturating_add(ancestor.2),
        )
    }

    pub fn matches(&self, document: &NativeDocument, node: NodeId) -> bool {
        self.matches_with_interaction(document, node, &InteractionSnapshot::default())
    }

    pub fn matches_with_interaction(
        &self,
        document: &NativeDocument,
        node: NodeId,
        interaction: &InteractionSnapshot,
    ) -> bool {
        let Some(candidate) = document.node(node) else {
            return false;
        };
        let NodeKind::Element { tag } = &candidate.kind else {
            return false;
        };

        if let Some(expected) = &self.tag
            && tag != expected
        {
            return false;
        }

        if let Some(expected) = &self.id
            && document.attribute(node, "id") != Some(expected.as_str())
        {
            return false;
        }

        if self.root && candidate.parent != Some(document.root()) {
            return false;
        }

        if !self.classes.is_empty() {
            let classes = document
                .attribute(node, "class")
                .unwrap_or_default()
                .split_ascii_whitespace()
                .collect::<Vec<_>>();
            if self
                .classes
                .iter()
                .any(|required| !classes.contains(&required.as_str()))
            {
                return false;
            }
        }

        for attribute in &self.attributes {
            let Some(actual) = document.attribute(node, &attribute.name) else {
                return false;
            };
            let matched = if attribute.case_insensitive {
                let actual = actual.to_ascii_lowercase();
                let expected = attribute.value.to_ascii_lowercase();
                match attribute.operator {
                    AttributeOperator::Equals => actual == expected,
                    AttributeOperator::IncludesWord => {
                        actual.split_ascii_whitespace().any(|word| word == expected)
                    }
                    AttributeOperator::DashPrefix => {
                        actual == expected || actual.starts_with(&(expected + "-"))
                    }
                    AttributeOperator::Prefix => actual.starts_with(&expected),
                    AttributeOperator::Suffix => actual.ends_with(&expected),
                    AttributeOperator::Contains => actual.contains(&expected),
                }
            } else {
                match attribute.operator {
                    AttributeOperator::Equals => actual == attribute.value,
                    AttributeOperator::IncludesWord => actual
                        .split_ascii_whitespace()
                        .any(|word| word == attribute.value),
                    AttributeOperator::DashPrefix => {
                        actual == attribute.value
                            || actual.starts_with(&(attribute.value.clone() + "-"))
                    }
                    AttributeOperator::Prefix => actual.starts_with(&attribute.value),
                    AttributeOperator::Suffix => actual.ends_with(&attribute.value),
                    AttributeOperator::Contains => actual.contains(&attribute.value),
                }
            };
            if !matched {
                return false;
            }
        }

        if !self.any_of.is_empty()
            && !self
                .any_of
                .iter()
                .any(|selector| selector.matches_with_interaction(document, node, interaction))
        {
            return false;
        }

        if !self.where_any_of.is_empty()
            && !self
                .where_any_of
                .iter()
                .any(|selector| selector.matches_with_interaction(document, node, interaction))
        {
            return false;
        }

        if self
            .none_of
            .iter()
            .any(|selector| selector.matches_with_interaction(document, node, interaction))
        {
            return false;
        }

        if self
            .states
            .iter()
            .any(|predicate| !interaction.matches(document, node, *predicate))
        {
            return false;
        }

        if !self.structural.is_empty() {
            let Some(parent) = candidate.parent else {
                return false;
            };
            let Some(index) = document.element_index(node) else {
                return false;
            };
            let count = document.element_children(parent).len();
            if self
                .structural
                .iter()
                .any(|predicate| !predicate.matches(index, count))
            {
                return false;
            }
        }

        if let Some((relation, related)) = &self.ancestor {
            let matched = match relation {
                AcirSelectorRelation::Descendant => document
                    .ancestor_elements(node)
                    .into_iter()
                    .any(|ancestor_id| related.matches_with_interaction(document, ancestor_id, interaction)),
                AcirSelectorRelation::Child => document
                    .element_parent(node)
                    .is_some_and(|parent_id| related.matches_with_interaction(document, parent_id, interaction)),
                AcirSelectorRelation::AdjacentSibling => document
                    .previous_element_sibling(node)
                    .is_some_and(|sibling_id| related.matches_with_interaction(document, sibling_id, interaction)),
                AcirSelectorRelation::GeneralSibling => document
                    .previous_element_siblings(node)
                    .into_iter()
                    .any(|sibling_id| related.matches_with_interaction(document, sibling_id, interaction)),
            };
            if !matched {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssGlobalKeyword {
    Inherit,
    Initial,
    Unset,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    Display(String),
    Keyword(String),
    GridColumns(u16),
    Global(CssGlobalKeyword),
    Px(f32),
    RelativeLength(AcirRelativeLength),
    Var(String),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub property: String,
    pub value: CssValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleSheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStyleSheet {
    pub stylesheet: StyleSheet,
    pub translations: Vec<TranslationReceipt>,
}

fn parse_attribute_selector(input: &str) -> Result<AttributeSelector, CssError> {
    let input = input.trim();
    let (body, case_insensitive) = if let Some(body) = input.strip_suffix(" i") {
        (body.trim_end(), true)
    } else {
        (input, false)
    };

    let operators = [
        ("~=", AttributeOperator::IncludesWord),
        ("|=", AttributeOperator::DashPrefix),
        ("^=", AttributeOperator::Prefix),
        ("$=", AttributeOperator::Suffix),
        ("*=", AttributeOperator::Contains),
        ("=", AttributeOperator::Equals),
    ];
    let Some((name, operator, raw_value)) = operators
        .into_iter()
        .find_map(|(token, operator)| body.split_once(token).map(|(name, value)| (name, operator, value)))
    else {
        return Err(CssError::UnsupportedSelector(format!("[{input}]")));
    };

    let name = name.trim().to_ascii_lowercase();
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'))
    {
        return Err(CssError::UnsupportedSelector(format!("[{input}]")));
    }

    let raw_value = raw_value.trim();
    let value = if (raw_value.starts_with('"') && raw_value.ends_with('"'))
        || (raw_value.starts_with('\'') && raw_value.ends_with('\''))
    {
        if raw_value.len() < 2 {
            return Err(CssError::UnsupportedSelector(format!("[{input}]")));
        }
        raw_value[1..raw_value.len() - 1].to_string()
    } else {
        raw_value.to_string()
    };
    if value.is_empty() {
        return Err(CssError::UnsupportedSelector(format!("[{input}]")));
    }

    Ok(AttributeSelector {
        name,
        operator,
        value,
        case_insensitive,
    })
}

fn split_selector_list(input: &str) -> Result<Vec<&str>, CssError> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut bracket_depth = 0usize;
    let mut quote: Option<char> = None;

    for (index, ch) in input.char_indices() {
        if let Some(current_quote) = quote {
            if ch == current_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => {
                if bracket_depth == 0 {
                    return Err(CssError::UnsupportedSelector(input.into()));
                }
                bracket_depth -= 1;
            }
            ',' if bracket_depth == 0 => {
                let part = input[start..index].trim();
                if part.is_empty() {
                    return Err(CssError::UnsupportedSelector(input.into()));
                }
                parts.push(part);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    if quote.is_some() || bracket_depth != 0 {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    let tail = input[start..].trim();
    if tail.is_empty() {
        return Err(CssError::UnsupportedSelector(input.into()));
    }
    parts.push(tail);
    Ok(parts)
}

fn parse_simple_selector(input: &str) -> Result<Selector, CssError> {
    let input = input.trim();
    if input.is_empty()
        || input.trim_start().starts_with('*')
        || input.contains(':')
        || input
            .chars()
            .any(|ch| ch.is_whitespace() && !input.contains('['))
    {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    let mut selector = Selector {
        tag: None,
        id: None,
        classes: Vec::new(),
        attributes: Vec::new(),
        root: false,
        states: Vec::new(),
        structural: Vec::new(),
        any_of: Vec::new(),
        where_any_of: Vec::new(),
        none_of: Vec::new(),
        ancestor: None,
    };
    let mut cursor = 0usize;

    if !matches!(input.as_bytes().first(), Some(b'.' | b'#' | b'[')) {
        let end = input.find(['.', '#', '[']).unwrap_or(input.len());
        let tag = input[..end].trim();
        if tag.is_empty() || tag.chars().any(char::is_whitespace) {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        selector.tag = Some(tag.to_ascii_lowercase());
        cursor = end;
    }

    while cursor < input.len() {
        match input.as_bytes()[cursor] {
            b'.' | b'#' => {
                let marker = input.as_bytes()[cursor];
                cursor += 1;
                let rest = &input[cursor..];
                let end_rel = rest.find(['.', '#', '[']).unwrap_or(rest.len());
                let value = &rest[..end_rel];
                if value.is_empty() || value.chars().any(char::is_whitespace) {
                    return Err(CssError::UnsupportedSelector(input.into()));
                }
                if marker == b'#' {
                    if selector.id.is_some() {
                        return Err(CssError::UnsupportedSelector(input.into()));
                    }
                    selector.id = Some(value.into());
                } else {
                    selector.classes.push(value.into());
                }
                cursor += end_rel;
            }
            b'[' => {
                let rest = &input[cursor + 1..];
                let Some(close_rel) = rest.find(']') else {
                    return Err(CssError::UnsupportedSelector(input.into()));
                };
                let body = &rest[..close_rel];
                selector.attributes.push(parse_attribute_selector(body)?);
                cursor += close_rel + 2;
            }
            _ => return Err(CssError::UnsupportedSelector(input.into())),
        }
    }

    if selector.tag.is_none()
        && selector.id.is_none()
        && selector.classes.is_empty()
        && selector.attributes.is_empty()
        && !selector.root
    {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    Ok(selector)
}

fn parse_selector(input: &str) -> Result<Selector, CssError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    let compiled_selector = match compile_selector_capability(input) {
        Ok(compiled) => compiled,
        Err(CapabilityCompilerError::UnsupportedSelector(_)) => {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        Err(error) => return Err(error.into()),
    };

    if let Some(compiled) = compiled_selector {
        let mut current: Option<Selector> = None;
        for (index, compound) in compiled.chain.compounds.iter().enumerate() {
            let mut selector = parse_selector(compound)?;
            if index > 0 {
                let relation = compiled.chain.combinators[index - 1];
                let ancestor = current
                    .take()
                    .ok_or_else(|| CssError::UnsupportedSelector(input.into()))?;
                selector.ancestor = Some((relation, Box::new(ancestor)));
            }
            current = Some(selector);
        }
        return current.ok_or_else(|| CssError::UnsupportedSelector(input.into()));
    }

    let compiled_boolean = match compile_selector_boolean_capability(input) {
        Ok(compiled) => compiled,
        Err(CapabilityCompilerError::UnsupportedSelector(_)) => {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        Err(error) => return Err(error.into()),
    };
    if let Some(compiled) = compiled_boolean {
        let mut selector = if compiled.base_selector.is_empty() {
            Selector {
                tag: None,
                id: None,
                classes: Vec::new(),
                attributes: Vec::new(),
                root: false,
                states: Vec::new(),
                structural: Vec::new(),
                any_of: Vec::new(),
                where_any_of: Vec::new(),
                none_of: Vec::new(),
                ancestor: None,
            }
        } else {
            parse_selector(&compiled.base_selector)?
        };
        let alternatives = compiled
            .alternatives
            .iter()
            .map(|alternative| parse_selector(alternative))
            .collect::<Result<Vec<_>, _>>()?;
        match compiled.operation {
            AcirSelectorBoolean::Any => selector.any_of = alternatives,
            AcirSelectorBoolean::AnyZeroSpecificity => selector.where_any_of = alternatives,
            AcirSelectorBoolean::None => selector.none_of = alternatives,
        }
        return Ok(selector);
    }

    let compiled_state = match compile_selector_state_capability(input) {
        Ok(compiled) => compiled,
        Err(CapabilityCompilerError::UnsupportedSelector(_)) => {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        Err(error) => return Err(error.into()),
    };
    if let Some(compiled) = compiled_state {
        let mut selector = if compiled.base_selector.is_empty() {
            Selector {
                tag: None,
                id: None,
                classes: Vec::new(),
                attributes: Vec::new(),
                root: false,
                states: Vec::new(),
                structural: Vec::new(),
                any_of: Vec::new(),
                where_any_of: Vec::new(),
                none_of: Vec::new(),
                ancestor: None,
            }
        } else {
            parse_selector(&compiled.base_selector)?
        };
        selector.states.extend(compiled.predicates);
        return Ok(selector);
    }

    let compiled_structural = match compile_selector_structural_capability(input) {
        Ok(compiled) => compiled,
        Err(CapabilityCompilerError::UnsupportedSelector(_)) => {
            return Err(CssError::UnsupportedSelector(input.into()));
        }
        Err(error) => return Err(error.into()),
    };
    if let Some(compiled) = compiled_structural {
        let mut selector = if compiled.base_selector.is_empty() {
            Selector {
                tag: None,
                id: None,
                classes: Vec::new(),
                attributes: Vec::new(),
                root: false,
                states: Vec::new(),
                structural: Vec::new(),
                any_of: Vec::new(),
                where_any_of: Vec::new(),
                none_of: Vec::new(),
                ancestor: None,
            }
        } else {
            parse_selector(&compiled.base_selector)?
        };
        selector.structural.extend(compiled.predicates);
        return Ok(selector);
    }

    if input == ":root" {
        return Ok(Selector {
            tag: None,
            id: None,
            classes: Vec::new(),
            attributes: Vec::new(),
            root: true,
            states: Vec::new(),
            structural: Vec::new(),
            any_of: Vec::new(),
            where_any_of: Vec::new(),
            none_of: Vec::new(),
            ancestor: None,
        });
    }

    if split_selector_list(input)?.len() > 1 {
        return Err(CssError::UnsupportedSelector(input.into()));
    }

    parse_simple_selector(input)
}

fn parse_px(property: &str, value: &str) -> Result<f32, CssError> {
    let Some(number) = value.strip_suffix("px") else {
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

fn parse_declarations_with_translations(
    input: &str,
) -> Result<(Vec<Declaration>, Vec<TranslationReceipt>), CssError> {
    let mut declarations = Vec::new();
    let mut translations = Vec::new();

    for raw in input.split(';') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let Some((property, value)) = raw.split_once(':') else {
            return Err(CssError::MalformedDeclaration(raw.into()));
        };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim();

        let global_keyword = match value {
            "inherit" => Some(CssGlobalKeyword::Inherit),
            "initial" => Some(CssGlobalKeyword::Initial),
            "unset" => Some(CssGlobalKeyword::Unset),
            _ => None,
        };

        let parsed = if property.starts_with("--") {
            Some(CssValue::Raw(value.into()))
        } else if let Some(keyword) = global_keyword {
            match property.as_str() {
                "display" | "flex-direction" | "grid-template-columns" | "font-size"
                | "margin-top" | "margin-bottom" | "padding-top" | "padding-right"
                | "padding-bottom" | "padding-left" | "gap" => Some(CssValue::Global(keyword)),
                _ => None,
            }
        } else {
            match property.as_str() {
                "display" => match value {
                    "none" | "block" | "inline" | "inline-block" | "flex" | "inline-flex"
                    | "grid" | "inline-grid" | "table" | "inline-table" | "table-row"
                    | "table-cell" | "table-row-group" | "table-header-group"
                    | "table-footer-group" | "table-caption" => {
                        Some(CssValue::Display(value.into()))
                    },
                    _ => {
                        return Err(CssError::InvalidValue {
                            property,
                            value: value.into(),
                        });
                    }
                },
                "flex-direction" => match value {
                    "row" | "column" => Some(CssValue::Keyword(value.into())),
                    _ => {
                        return Err(CssError::InvalidValue {
                            property,
                            value: value.into(),
                        });
                    }
                },
                "grid-template-columns" => {
                    let tracks = value.split_ascii_whitespace().collect::<Vec<_>>();
                    if tracks.is_empty()
                        || tracks.len() > 12
                        || tracks.iter().any(|track| *track != "1fr")
                    {
                        return Err(CssError::InvalidValue {
                            property,
                            value: value.into(),
                        });
                    }
                    Some(CssValue::GridColumns(tracks.len() as u16))
                }
                "font-size" => {
                    if let Some(inner) =
                        value.strip_prefix("var(").and_then(|v| v.strip_suffix(')'))
                    {
                        let name = inner.trim();
                        if !name.starts_with("--") {
                            return Err(CssError::InvalidValue {
                                property,
                                value: value.into(),
                            });
                        }
                        Some(CssValue::Var(name.into()))
                    } else if let Some(receipt) = compile_zero_length_capability(value)? {
                        translations.push(receipt);
                        Some(CssValue::Px(0.0))
                    } else if let Some(compiled) = compile_font_size_capability(value)? {
                        translations.push(compiled.receipt);
                        Some(CssValue::RelativeLength(compiled.value))
                    } else {
                        Some(CssValue::Px(parse_px(&property, value)?))
                    }
                }
                "margin-top" | "margin-bottom" | "padding-top" | "padding-right"
                | "padding-bottom" | "padding-left" | "gap" => {
                    if let Some(inner) =
                        value.strip_prefix("var(").and_then(|v| v.strip_suffix(')'))
                    {
                        let name = inner.trim();
                        if !name.starts_with("--") {
                            return Err(CssError::InvalidValue {
                                property,
                                value: value.into(),
                            });
                        }
                        Some(CssValue::Var(name.into()))
                    } else if let Some(receipt) = compile_zero_length_capability(value)? {
                        translations.push(receipt);
                        Some(CssValue::Px(0.0))
                    } else if let Some(compiled) = compile_current_font_length_capability(value)? {
                        translations.push(compiled.receipt);
                        Some(CssValue::RelativeLength(compiled.value))
                    } else {
                        Some(CssValue::Px(parse_px(&property, value)?))
                    }
                }
                // CSS-compatible recovery for properties N2 does not implement.
                _ => None,
            }
        };

        if let Some(value) = parsed {
            declarations.push(Declaration { property, value });
        }
    }

    Ok((declarations, translations))
}

pub fn parse_declarations(input: &str) -> Result<Vec<Declaration>, CssError> {
    Ok(parse_declarations_with_translations(input)?.0)
}
fn strip_css_comments(input: &str) -> Result<String, CssError> {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;

    loop {
        let Some(start) = rest.find("/*") else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("*/") else {
            return Err(CssError::MalformedRule);
        };
        rest = &after_start[end + 2..];
    }

    Ok(output)
}

fn collect_selector_translation_receipts(
    selector_source: &str,
    translations: &mut Vec<TranslationReceipt>,
) -> Result<(), CssError> {
    match compile_selector_capability(selector_source) {
        Ok(Some(compiled_selector)) => {
            for compound in &compiled_selector.chain.compounds {
                if let Some(boolean) = compile_selector_boolean_capability(compound)? {
                    translations.push(boolean.receipt);
                }
                if let Some(state) = compile_selector_state_capability(compound)? {
                    translations.push(state.receipt);
                }
                if let Some(structural) = compile_selector_structural_capability(compound)? {
                    translations.push(structural.receipt);
                }
            }
            translations.push(compiled_selector.receipt);
        }
        Ok(None) => {
            if let Some(boolean) = compile_selector_boolean_capability(selector_source)? {
                translations.push(boolean.receipt);
            }
            if let Some(state) = compile_selector_state_capability(selector_source)? {
                translations.push(state.receipt);
            }
            if let Some(structural) = compile_selector_structural_capability(selector_source)? {
                translations.push(structural.receipt);
            }
        }
        Err(CapabilityCompilerError::UnsupportedSelector(_)) => {
            return Err(CssError::UnsupportedSelector(selector_source.into()));
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn parse_compiled_stylesheet(
    input: &str,
) -> Result<(StyleSheet, Vec<TranslationReceipt>), CssError> {
    let mut rules = Vec::new();
    let mut translations = Vec::new();
    let mut rest = input;
    let mut order = 0usize;

    while !rest.trim().is_empty() {
        let Some(open) = rest.find('{') else {
            return Err(CssError::MalformedRule);
        };
        let Some(close_rel) = rest[open + 1..].find('}') else {
            return Err(CssError::MalformedRule);
        };
        let close = open + 1 + close_rel;

        let selector_source = rest[..open].trim();
        let selectors = if let Some(compiled_list) =
            compile_selector_list_capability(selector_source)?
        {
            translations.push(compiled_list.receipt);
            compiled_list.selectors
        } else {
            vec![selector_source.to_string()]
        };

        let (declarations, declaration_translations) =
            parse_declarations_with_translations(&rest[open + 1..close])?;
        translations.extend(declaration_translations);

        for selector_source in selectors {
            collect_selector_translation_receipts(&selector_source, &mut translations)?;
            let selector = parse_selector(&selector_source)?;
            rules.push(Rule {
                selector,
                declarations: declarations.clone(),
                order,
            });
        }

        order += 1;
        rest = &rest[close + 1..];
    }

    Ok((StyleSheet { rules }, translations))
}

pub fn parse_stylesheet_with_environment(
    input: &str,
    environment: CapabilityEnvironment,
) -> Result<CompiledStyleSheet, CssError> {
    let cleaned = strip_css_comments(input)?;
    let compiled = compile_stylesheet_capabilities(&cleaned, environment)?;
    let (stylesheet, selector_translations) = parse_compiled_stylesheet(&compiled.source)?;
    let mut translations = compiled.receipts;
    translations.extend(selector_translations);
    Ok(CompiledStyleSheet {
        stylesheet,
        translations,
    })
}

pub fn parse_stylesheet(input: &str) -> Result<StyleSheet, CssError> {
    Ok(parse_stylesheet_with_environment(input, CapabilityEnvironment::default())?.stylesheet)
}

fn document_style_source(document: &NativeDocument) -> String {
    let mut combined = String::new();
    for node in document.nodes() {
        if matches!(&node.kind, NodeKind::Element { tag } if tag == "style") {
            for child in &node.children {
                if let Some(crate::native_dom::Node {
                    kind: NodeKind::Text(text),
                    ..
                }) = document.node(*child)
                {
                    combined.push_str(text);
                    combined.push('\n');
                }
            }
        }
    }
    combined
}

pub fn stylesheet_from_document_with_environment(
    document: &NativeDocument,
    environment: CapabilityEnvironment,
) -> Result<CompiledStyleSheet, CssError> {
    parse_stylesheet_with_environment(&document_style_source(document), environment)
}

pub fn stylesheet_from_document(document: &NativeDocument) -> Result<StyleSheet, CssError> {
    Ok(
        stylesheet_from_document_with_environment(document, CapabilityEnvironment::default())?
            .stylesheet,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_html::parse_document;

    #[test]
    fn parses_compound_selector_specificity() {
        let sheet = parse_stylesheet("p.lead#hero { font-size: 20px; }").unwrap();
        assert_eq!(sheet.rules[0].selector.specificity(), (1, 1, 1));
    }

    #[test]
    fn selector_matches_owned_attributes() {
        let document =
            parse_document("<html><body><p id=\"hero\" class=\"lead x\">Hi</p></body></html>")
                .unwrap();
        let node = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        let selector = parse_selector("p#hero.lead").unwrap();
        assert!(selector.matches(&document, node.id));
    }

    #[test]
    fn attribute_prefix_selector_preserves_quoted_comma_with_case_flag() {
        let selector = parse_attribute_selector("sizes^=\"auto,\" i").unwrap();
        assert_eq!(selector.name, "sizes");
        assert_eq!(selector.operator, AttributeOperator::Prefix);
        assert_eq!(selector.value, "auto,");
        assert!(selector.case_insensitive);
    }

    #[test]
    fn attribute_only_selector_survives_simple_selector_parser() {
        let selector = parse_simple_selector("[sizes^=\"auto,\" i]").unwrap();
        assert_eq!(selector.attributes.len(), 1);
    }

    #[test]
    fn attribute_only_selector_survives_full_selector_pipeline() {
        let selector = parse_selector("[sizes^=\"auto,\" i]").unwrap();
        assert_eq!(selector.attributes.len(), 1);
        assert_eq!(selector.attributes[0].value, "auto,");
    }

    #[test]
    fn bare_interaction_selector_matches_only_explicit_state() {
        let document =
            parse_document("<html><body><button>Go</button><div>Other</div></body></html>").unwrap();
        let button = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "button"))
            .unwrap();
        let selector = parse_selector(":active").unwrap();

        assert!(!selector.matches(&document, button.id));
        let interaction = InteractionSnapshot {
            active_node: Some(button.id),
            ..InteractionSnapshot::default()
        };
        assert!(selector.matches_with_interaction(&document, button.id, &interaction));
    }

    #[test]
    fn parses_full_attribute_operator_family() {
        let document = parse_document(
            "<html><body><div data-words=\"alpha beta\" lang=\"en-US\" data-prefix=\"start-middle-end\" style=\"border-top-color:red\"></div></body></html>",
        )
        .unwrap();
        let node = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "div"))
            .unwrap();

        for selector in [
            "[data-words~=beta]",
            "[lang|=en]",
            "[data-prefix^=start]",
            "[data-prefix$=end]",
            "[data-prefix*=middle]",
            "[style*=border-top-color]",
        ] {
            assert!(parse_selector(selector).unwrap().matches(&document, node.id), "{selector}");
        }
    }

    #[test]
    fn attribute_operator_case_flag_applies_to_all_operator_forms() {
        let document =
            parse_document("<html><body><div data-x=\"Alpha Beta-Gamma\" lang=\"EN-us\"></div></body></html>")
                .unwrap();
        let node = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "div"))
            .unwrap();

        for selector in [
            "[data-x~=alpha i]",
            "[lang|=en i]",
            "[data-x^=alpha i]",
            "[data-x$=gamma i]",
            "[data-x*=beta i]",
        ] {
            assert!(parse_selector(selector).unwrap().matches(&document, node.id), "{selector}");
        }
    }

    #[test]
    fn parses_is_with_case_insensitive_attribute_selectors() {
        let document =
            parse_document("<html><body><img sizes=\"AUTO, 100vw\"></body></html>").unwrap();
        let image = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "img"))
            .unwrap();
        let selector = parse_selector("img:is([sizes=auto i],[sizes^=\"auto,\" i])").unwrap();

        assert!(selector.matches(&document, image.id));
        assert_eq!(selector.specificity(), (0, 1, 1));
    }

    #[test]
    fn root_selector_matches_only_document_element() {
        let document =
            parse_document("<html><body><div id=\"child\"></div></body></html>").unwrap();
        let html = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "html"))
            .unwrap();
        let div = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "div"))
            .unwrap();
        let selector = parse_selector(":root").unwrap();

        assert!(selector.matches(&document, html.id));
        assert!(!selector.matches(&document, div.id));
        assert_eq!(selector.specificity(), (0, 1, 0));
    }

    #[test]
    fn descendant_relation_matches_observed_root_class_selector() {
        let document = parse_document(
            "<html><body><section><div class=\"has-very-light-gray-background-color\">X</div></section></body></html>",
        )
        .unwrap();
        let target = document
            .nodes()
            .iter()
            .find(|node| {
                matches!(&node.kind, NodeKind::Element { tag } if tag == "div")
                    && document.attribute(node.id, "class")
                        == Some("has-very-light-gray-background-color")
            })
            .unwrap();
        let compiled = parse_stylesheet_with_environment(
            ":root .has-very-light-gray-background-color { font-size: 20px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        let selector = &compiled.stylesheet.rules[0].selector;

        assert!(selector.matches(&document, target.id));
        assert_eq!(selector.specificity(), (0, 2, 0));
        assert!(compiled
            .translations
            .iter()
            .any(|receipt| receipt.translation_id == "css.selector-descendant.v1"));
    }

    #[test]
    fn descendant_relation_requires_matching_ancestor() {
        let document = parse_document(
            "<html><body><section><p class=\"lead\">X</p></section></body></html>",
        )
        .unwrap();
        let target = document
            .nodes()
            .iter()
            .find(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "p"))
            .unwrap();
        let selector = parse_selector("article .lead").unwrap();
        assert!(!selector.matches(&document, target.id));
    }

    #[test]
    fn structural_selector_family_matches_element_positions() {
        let document = parse_document(
            "<html><body><div><button class=\"wp-block-button\">A</button><button class=\"wp-block-button\">B</button><button class=\"wp-block-button\">C</button></div></body></html>",
        )
        .unwrap();
        let buttons = document
            .nodes()
            .iter()
            .filter(|node| matches!(&node.kind, NodeKind::Element { tag } if tag == "button"))
            .collect::<Vec<_>>();

        assert!(parse_selector(".wp-block-button:first-child")
            .unwrap()
            .matches(&document, buttons[0].id));
        assert!(parse_selector(".wp-block-button:last-child")
            .unwrap()
            .matches(&document, buttons[2].id));
        assert!(parse_selector(".wp-block-button:nth-child(2)")
            .unwrap()
            .matches(&document, buttons[1].id));
        assert!(parse_selector(".wp-block-button:nth-child(odd)")
            .unwrap()
            .matches(&document, buttons[2].id));
        assert!(!parse_selector(".wp-block-button:only-child")
            .unwrap()
            .matches(&document, buttons[0].id));
    }

    #[test]
    fn structural_selector_translation_is_receipted() {
        let compiled = parse_stylesheet_with_environment(
            ".wp-block-button:last-child { margin-bottom: 0; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(compiled
            .translations
            .iter()
            .any(|receipt| receipt.translation_id == "css.selector-structural.v1"));
    }

    #[test]
    fn unsupported_nth_formula_stays_fail_closed() {
        assert!(matches!(
            parse_stylesheet("li:nth-child(2n+1) { margin-bottom: 0; }"),
            Err(CssError::UnsupportedSelector(_))
        ));
    }

    #[test]
    fn unsupported_pseudo_class_still_fails_closed() {
        assert!(matches!(
            parse_selector("a:visited"),
            Err(CssError::UnsupportedSelector(_))
        ));
    }

    #[test]
    fn child_and_sibling_relations_execute_through_acir() {
        let document = parse_document(
            "<html><body><main><h2>H</h2><p class=\"lead\">A</p><span>X</span><p class=\"later\">B</p></main></body></html>",
        )
        .unwrap();
        let lead = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("lead"))
            .unwrap();
        let later = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("later"))
            .unwrap();

        let child = parse_selector("main > .lead").unwrap();
        assert!(child.matches(&document, lead.id));

        let adjacent = parse_selector("h2 + .lead").unwrap();
        assert!(adjacent.matches(&document, lead.id));

        let general = parse_selector("h2 ~ .later").unwrap();
        assert!(general.matches(&document, later.id));
        assert_eq!(general.specificity(), (0, 1, 1));

        let compiled = parse_stylesheet_with_environment(
            "main > .lead { font-size: 20px; } h2 + .lead { margin-top: 4px; } h2 ~ .later { margin-bottom: 4px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(compiled
            .translations
            .iter()
            .filter(|receipt| receipt.translation_id == "css.selector-relations.v1")
            .count()
            >= 3);
    }

    #[test]
    fn malformed_selector_relation_still_fails_closed() {
        assert!(matches!(
            parse_selector("main > > p"),
            Err(CssError::UnsupportedSelector(_))
        ));
    }

    #[test]
    fn boolean_selector_family_matches_and_preserves_specificity_rules() {
        let document = parse_document(
            "<html><body><p class=\"lead has-border-color\">A</p><p class=\"summary\">B</p></body></html>",
        )
        .unwrap();
        let lead = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("lead has-border-color"))
            .unwrap();
        let summary = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("summary"))
            .unwrap();

        let where_selector = parse_selector(":where(.has-border-color)").unwrap();
        assert!(where_selector.matches(&document, lead.id));
        assert!(!where_selector.matches(&document, summary.id));
        assert_eq!(where_selector.specificity(), (0, 0, 0));

        let is_selector = parse_selector("p:is(.lead,.summary)").unwrap();
        assert!(is_selector.matches(&document, lead.id));
        assert!(is_selector.matches(&document, summary.id));
        assert_eq!(is_selector.specificity(), (0, 1, 1));

        let not_selector = parse_selector("p:not(.summary)").unwrap();
        assert!(not_selector.matches(&document, lead.id));
        assert!(!not_selector.matches(&document, summary.id));
        assert_eq!(not_selector.specificity(), (0, 1, 1));

        let compiled = parse_stylesheet_with_environment(
            ":where(.has-border-color) { font-size: 20px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(compiled
            .translations
            .iter()
            .any(|receipt| receipt.translation_id == "css.selector-boolean.v1"));
    }

    #[test]
    fn interaction_state_selector_is_explicit_and_snapshot_bound() {
        let document = parse_document(
            "<html><body><a class=\"screen-reader-text\">Skip</a></body></html>",
        )
        .unwrap();
        let target = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("screen-reader-text"))
            .unwrap();

        let compiled = parse_stylesheet_with_environment(
            ".screen-reader-text:focus { font-size: 20px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        let selector = &compiled.stylesheet.rules[0].selector;

        assert!(!selector.matches(&document, target.id));
        let focused = InteractionSnapshot {
            focused_node: Some(target.id),
            ..InteractionSnapshot::default()
        };
        assert!(selector.matches_with_interaction(&document, target.id, &focused));
        assert_eq!(selector.specificity(), (0, 2, 0));
        assert!(compiled
            .translations
            .iter()
            .any(|receipt| receipt.translation_id == "css.selector-interaction-state.v1"));
    }

    #[test]
    fn focus_within_uses_real_descendant_state() {
        let document = parse_document(
            "<html><body><div class=\"group\"><button class=\"button\">X</button></div></body></html>",
        )
        .unwrap();
        let group = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("group"))
            .unwrap();
        let button = document
            .nodes()
            .iter()
            .find(|node| document.attribute(node.id, "class") == Some("button"))
            .unwrap();
        let selector = parse_selector(".group:focus-within").unwrap();
        let snapshot = InteractionSnapshot {
            focused_node: Some(button.id),
            ..InteractionSnapshot::default()
        };
        assert!(selector.matches_with_interaction(&document, group.id, &snapshot));
    }

    #[test]
    fn font_em_is_lowered_through_acir_and_receipted() {
        let compiled = parse_stylesheet_with_environment(
            "p { font-size: 1.25em; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();

        assert!(matches!(
            compiled.stylesheet.rules[0].declarations[0].value,
            CssValue::RelativeLength(ref value) if value.milli_factor == 1_250
        ));
        assert!(compiled
            .translations
            .iter()
            .any(|receipt| receipt.translation_id == "css.font-em.v1"));
    }

    #[test]
    fn rem_is_not_silently_treated_as_em() {
        assert!(matches!(
            parse_declarations("font-size: 1rem;"),
            Err(CssError::InvalidValue { property, .. }) if property == "font-size"
        ));
    }

    #[test]
    fn strips_css_comments_before_selector_parsing() {
        let sheet =
            parse_stylesheet("/*# sourceURL=inline-css */ p.lead { font-size: 20px; }").unwrap();
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].selector.tag.as_deref(), Some("p"));
    }

    #[test]
    fn unterminated_css_comment_fails_closed() {
        assert!(matches!(
            parse_stylesheet("/* broken p { font-size: 20px; }"),
            Err(CssError::MalformedRule)
        ));
    }

    #[test]
    fn unknown_property_is_ignored_not_invented() {
        let declarations = parse_declarations("future-property: 1; font-size: 18px;").unwrap();
        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0].property, "font-size");
    }

    #[test]
    fn unitless_zero_normalizes_with_receipt_and_nonzero_unitless_fails() {
        let compiled = parse_stylesheet_with_environment(
            ".x { margin-bottom: 0; font-size: 0; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(compiled.stylesheet.rules[0]
            .declarations
            .iter()
            .all(|declaration| matches!(declaration.value, CssValue::Px(0.0))));
        assert_eq!(
            compiled
                .translations
                .iter()
                .filter(|receipt| receipt.translation_id == "css.length-zero.v1")
                .count(),
            2
        );

        assert!(matches!(
            parse_declarations("margin-bottom: 1;"),
            Err(CssError::InvalidValue { property, .. }) if property == "margin-bottom"
        ));
    }

    #[test]
    fn non_font_size_em_uses_current_font_relative_acir() {
        let compiled = parse_stylesheet_with_environment(
            ".box { margin-bottom: 1em; padding-left: .5em; gap: 2em; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert!(compiled.stylesheet.rules[0]
            .declarations
            .iter()
            .all(|declaration| matches!(declaration.value, CssValue::RelativeLength(_))));
        assert_eq!(
            compiled
                .translations
                .iter()
                .filter(|receipt| receipt.translation_id == "css.length-em.current-font.v1")
                .count(),
            3
        );
    }

    #[test]
    fn selector_list_expands_to_rules_with_shared_source_order() {
        let compiled = parse_stylesheet_with_environment(
            ".a,.b[data-x=\"x,y\"] { font-size: 20px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();

        assert_eq!(compiled.stylesheet.rules.len(), 2);
        assert_eq!(compiled.stylesheet.rules[0].order, 0);
        assert_eq!(compiled.stylesheet.rules[1].order, 0);
        assert!(compiled
            .translations
            .iter()
            .any(|receipt| receipt.translation_id == "css.selector-list.v1"));
    }

    #[test]
    fn parses_global_inheritance_keywords_without_approximating_revert() {
        for value in ["inherit", "initial", "unset"] {
            let declarations =
                parse_declarations(&format!("font-size: {value}; display: {value};")).unwrap();
            assert_eq!(declarations.len(), 2);
            assert!(matches!(declarations[0].value, CssValue::Global(_)));
            assert!(matches!(declarations[1].value, CssValue::Global(_)));
        }

        assert!(matches!(
            parse_declarations("font-size: revert;"),
            Err(CssError::InvalidValue { property, .. }) if property == "font-size"
        ));
    }

    #[test]
    fn parses_table_display_family() {
        for value in [
            "table",
            "inline-table",
            "table-row",
            "table-cell",
            "table-row-group",
            "table-header-group",
            "table-footer-group",
            "table-caption",
        ] {
            let declarations = parse_declarations(&format!("display: {value};")).unwrap();
            assert!(matches!(
                &declarations[0].value,
                CssValue::Display(parsed) if parsed == value
            ));
        }
    }

    #[test]
    fn parses_composite_display_family() {
        for value in ["inline-block", "inline-flex", "inline-grid"] {
            let declarations = parse_declarations(&format!("display: {value};")).unwrap();
            assert!(matches!(
                &declarations[0].value,
                CssValue::Display(parsed) if parsed == value
            ));
        }
    }

    #[test]
    fn parses_custom_property_reference() {
        let declarations =
            parse_declarations("--text-size: 22px; font-size: var(--text-size);").unwrap();
        assert_eq!(declarations.len(), 2);
        assert_eq!(declarations[0].property, "--text-size");
        assert!(matches!(declarations[0].value, CssValue::Raw(_)));
        assert!(matches!(declarations[1].value, CssValue::Var(_)));
    }

    #[test]
    fn parses_bounded_flex_and_grid_declarations() {
        let flex = parse_declarations("display: flex; flex-direction: column; gap: 12px;").unwrap();
        assert!(matches!(flex[0].value, CssValue::Display(ref value) if value == "flex"));
        assert!(matches!(flex[1].value, CssValue::Keyword(ref value) if value == "column"));
        assert!(matches!(flex[2].value, CssValue::Px(value) if value == 12.0));

        let grid =
            parse_declarations("display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 8px;")
                .unwrap();
        assert!(matches!(grid[1].value, CssValue::GridColumns(3)));
    }

    #[test]
    fn rejects_unbounded_grid_track_syntax() {
        assert!(matches!(
            parse_declarations("grid-template-columns: 100px 1fr;"),
            Err(CssError::InvalidValue { property, .. }) if property == "grid-template-columns"
        ));
    }

    #[test]
    fn media_resolution_is_lowered_before_selector_parsing() {
        let compiled = parse_stylesheet_with_environment(
            "@media (min-resolution:192dpi) { p { font-size: 20px; } } h1 { font-size: 24px; }",
            CapabilityEnvironment::desktop(800),
        )
        .unwrap();
        assert_eq!(compiled.stylesheet.rules.len(), 1);
        assert_eq!(
            compiled.stylesheet.rules[0].selector.tag.as_deref(),
            Some("h1")
        );
        assert_eq!(compiled.translations.len(), 1);
    }

    #[test]
    fn unsupported_media_query_remains_fail_closed() {
        let result =
            parse_stylesheet("@media (prefers-color-scheme:dark) { p { font-size: 20px; } }");
        assert!(matches!(result, Err(CssError::CapabilityCompiler(_))));
    }

    #[test]
    fn extracts_style_elements_from_native_document() {
        let document = parse_document(
            "<html><head><style>p.lead { font-size: 22px; }</style></head><body><p class=\"lead\">X</p></body></html>",
        )
        .unwrap();
        let sheet = stylesheet_from_document(&document).unwrap();
        assert_eq!(sheet.rules.len(), 1);
    }
}
