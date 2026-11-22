//! PWX Strict on the native engine.
//!
//! A PWX Strict app never touches a document. Its code runs in a sealed worker and
//! sends a short list of UI operations (`create`, `attr`, `prop`, `text`, `append`,
//! `remove`, `focus`, `listen`). The PWX runtime applies only allowlisted tags,
//! attributes, properties and events, and rejects and records everything else.
//!
//! This module is that renderer, owned by Altru Browser: the operations land in an
//! Altru-owned mirror tree, which becomes a `NativeDocument` for the native style,
//! layout and scene pipeline. The allowlists match PWX `app-origin/renderer.js`
//! (spike 0.4.10) exactly; a PWX app that works there is held to the same rules here.
//!
//! v0 rebuilds a fresh `NativeDocument` for each frame (`snapshot`). In-place
//! mutation through a DOM transaction / mutation journal is the next step; it needs
//! detach support in `native_dom` that every whole-arena pass must then respect.
//!
//! What runs the app's JavaScript is out of scope here. Until a `ScriptEngine`
//! (Boa) is wired in, recorded operation streams (`tools/pwx-strict-record.mjs`)
//! drive the bridge.

use std::collections::{BTreeMap, BTreeSet};

use crate::native_css::{CssError, StyleSheet, parse_stylesheet};
use crate::native_dom::{Attribute, NativeDocument, NodeId};
use crate::native_layout::{layout_document_with_styles, scene_from_layout};
use crate::native_scene::Scene;

// ---------------------------------------------------------------------------
// Policy: identical to PWX spike app-origin/renderer.js
// ---------------------------------------------------------------------------

pub const STRICT_TAGS: &[&str] = &[
    "div", "p", "span", "h1", "h2", "h3", "ul", "ol", "li", "button", "input", "label", "strong",
    "em", "img", "section", "header", "footer", "textarea", "nav", "aside", "main", "article",
    "small",
];
pub const STRICT_ATTRS: &[&str] = &[
    "class",
    "id",
    "type",
    "value",
    "placeholder",
    "alt",
    "title",
    "disabled",
    "checked",
    "rows",
    "maxlength",
    "aria-label",
    "aria-current",
    "spellcheck",
];
pub const STRICT_PROPS: &[&str] = &["value", "checked", "disabled"];
pub const STRICT_EVENTS: &[&str] = &["click", "input", "change", "keydown"];

/// Resource limits. Not in the PWX spike; the native host fails closed instead of
/// letting one app exhaust memory on a small machine.
pub const MAX_NODES: usize = 20_000;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;

/// App node id 0 is the body, as in the PWX runtime.
pub const BODY: u64 = 0;

// ---------------------------------------------------------------------------
// Operations and the recorded stream format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    Text(String),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StrictOp {
    Create {
        id: u64,
        tag: String,
    },
    Attr {
        id: u64,
        name: String,
        value: String,
    },
    Text {
        id: u64,
        value: String,
    },
    Prop {
        id: u64,
        name: String,
        value: PropValue,
    },
    Append {
        parent: u64,
        id: u64,
    },
    Remove {
        id: u64,
    },
    Focus {
        id: u64,
    },
    Listen {
        id: u64,
        event: String,
    },
    /// An operation type the runtime doesn't know. Always rejected.
    Unknown {
        name: String,
    },
}

impl StrictOp {
    fn kind(&self) -> &'static str {
        match self {
            Self::Create { .. } => "create",
            Self::Attr { .. } => "attr",
            Self::Text { .. } => "text",
            Self::Prop { .. } => "prop",
            Self::Append { .. } => "append",
            Self::Remove { .. } => "remove",
            Self::Focus { .. } => "focus",
            Self::Listen { .. } => "listen",
            Self::Unknown { .. } => "op",
        }
    }
}

/// One line of a recorded stream.
#[derive(Debug, Clone, PartialEq)]
pub enum StrictLine {
    Op(StrictOp),
    /// End of one batch the app posted: a point where the screen may be redrawn.
    Frame,
    /// A user action delivered to the app (click, typing).
    UserEvent {
        id: u64,
        event: String,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamError {
    pub line: usize,
    pub reason: String,
}

fn unescape(field: &str) -> Result<String, String> {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            other => return Err(format!("bad escape \\{}", other.unwrap_or(' '))),
        }
    }
    Ok(out)
}

/// Parse a stream written by `tools/pwx-strict-record.mjs`: tab-separated fields,
/// `\t` `\n` `\\` escaped. Malformed lines are an error, never skipped.
pub fn parse_stream(input: &str) -> Result<Vec<StrictLine>, StreamError> {
    let mut lines = Vec::new();
    for (index, raw) in input.lines().enumerate() {
        let line_no = index + 1;
        if raw.is_empty() {
            continue;
        }
        let err = |reason: String| StreamError {
            line: line_no,
            reason,
        };
        let fields = raw
            .split('\t')
            .map(unescape)
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        let id = |i: usize| -> Result<u64, StreamError> {
            fields
                .get(i)
                .and_then(|f| f.parse::<u64>().ok())
                .ok_or_else(|| err(format!("field {i} is not an id")))
        };
        let text = |i: usize| -> Result<String, StreamError> {
            fields
                .get(i)
                .cloned()
                .ok_or_else(|| err(format!("missing field {i}")))
        };
        let want = |n: usize| -> Result<(), StreamError> {
            if fields.len() == n {
                Ok(())
            } else {
                Err(err(format!("expected {n} fields, got {}", fields.len())))
            }
        };
        let parsed = match fields[0].as_str() {
            "frame" => {
                want(1)?;
                StrictLine::Frame
            }
            "event" => {
                want(4)?;
                StrictLine::UserEvent {
                    id: id(1)?,
                    event: text(2)?,
                    value: text(3)?,
                }
            }
            "create" => {
                want(3)?;
                StrictLine::Op(StrictOp::Create {
                    id: id(1)?,
                    tag: text(2)?,
                })
            }
            "attr" => {
                want(4)?;
                StrictLine::Op(StrictOp::Attr {
                    id: id(1)?,
                    name: text(2)?,
                    value: text(3)?,
                })
            }
            "text" => {
                want(3)?;
                StrictLine::Op(StrictOp::Text {
                    id: id(1)?,
                    value: text(2)?,
                })
            }
            "prop" => {
                want(4)?;
                let raw_value = text(3)?;
                let value = if let Some(s) = raw_value.strip_prefix("s:") {
                    PropValue::Text(s.to_string())
                } else if raw_value == "b:true" {
                    PropValue::Bool(true)
                } else if raw_value == "b:false" {
                    PropValue::Bool(false)
                } else {
                    return Err(err(format!("bad prop value {raw_value}")));
                };
                StrictLine::Op(StrictOp::Prop {
                    id: id(1)?,
                    name: text(2)?,
                    value,
                })
            }
            "append" => {
                want(3)?;
                StrictLine::Op(StrictOp::Append {
                    parent: id(1)?,
                    id: id(2)?,
                })
            }
            "remove" => {
                want(2)?;
                StrictLine::Op(StrictOp::Remove { id: id(1)? })
            }
            "focus" => {
                want(2)?;
                StrictLine::Op(StrictOp::Focus { id: id(1)? })
            }
            "listen" => {
                want(3)?;
                StrictLine::Op(StrictOp::Listen {
                    id: id(1)?,
                    event: text(2)?,
                })
            }
            "unknown" => {
                want(2)?;
                StrictLine::Op(StrictOp::Unknown { name: text(1)? })
            }
            other => return Err(err(format!("unknown line type {other}"))),
        };
        lines.push(parsed);
    }
    Ok(lines)
}

// ---------------------------------------------------------------------------
// The bridge
// ---------------------------------------------------------------------------

/// A rejected operation, in the same `op:reason:detail` shape as PWX's
/// `__pwxRejected`, so reports from both runtimes can be compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection(pub String);

#[derive(Debug, Clone, PartialEq)]
enum Child {
    Element(u64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
struct MirrorNode {
    tag: String,
    attributes: Vec<(String, String)>,
    children: Vec<Child>,
    parent: Option<u64>,
    value: String,
    checked: bool,
    disabled: bool,
}

impl MirrorNode {
    fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: Vec::new(),
            children: Vec::new(),
            parent: None,
            value: String::new(),
            checked: false,
            disabled: false,
        }
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// An event the host forwards to the app's worker as `{t:"event", id, type, value, key}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEvent {
    pub id: u64,
    pub event: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StrictBridge {
    nodes: BTreeMap<u64, MirrorNode>,
    listeners: BTreeSet<(u64, String)>,
    focused: Option<u64>,
    rejected: Vec<Rejection>,
    text_bytes: usize,
    epoch: u64,
}

impl Default for StrictBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl StrictBridge {
    pub fn new() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(BODY, MirrorNode::new("body"));
        Self {
            nodes,
            listeners: BTreeSet::new(),
            focused: None,
            rejected: Vec::new(),
            text_bytes: 0,
            epoch: 0,
        }
    }

    pub fn rejected(&self) -> &[Rejection] {
        &self.rejected
    }

    pub fn focused(&self) -> Option<u64> {
        self.focused
    }

    /// Bumped by every accepted change; a redraw is needed when it moves.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len() - 1
    }

    fn reject(&mut self, op: &StrictOp, reason: &str, detail: &str) {
        self.rejected
            .push(Rejection(format!("{}:{}:{}", op.kind(), reason, detail)));
    }

    fn element(&self, id: u64) -> Option<&MirrorNode> {
        if id == BODY {
            return None;
        }
        self.nodes.get(&id)
    }

    fn charge_text(&mut self, op: &StrictOp, bytes: usize) -> bool {
        if self.text_bytes.saturating_add(bytes) > MAX_TEXT_BYTES {
            self.reject(op, "quota", "text");
            return false;
        }
        self.text_bytes += bytes;
        true
    }

    fn detach(&mut self, id: u64) {
        let Some(parent) = self.nodes.get(&id).and_then(|n| n.parent) else {
            return;
        };
        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node
                .children
                .retain(|c| !matches!(c, Child::Element(child) if *child == id));
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = None;
        }
    }

    /// True if `ancestor` is `node` or one of its ancestors.
    fn is_ancestor(&self, ancestor: u64, node: u64) -> bool {
        let mut current = Some(node);
        let mut steps = 0;
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            steps += 1;
            if steps > MAX_NODES + 1 {
                return true; // corrupt chain: treat as a cycle, fail closed
            }
            current = self.nodes.get(&id).and_then(|n| n.parent);
        }
        false
    }

    pub fn apply(&mut self, op: &StrictOp) {
        match op {
            StrictOp::Create { id, tag } => {
                let tag = tag.to_ascii_lowercase();
                if !STRICT_TAGS.contains(&tag.as_str()) {
                    return self.reject(op, "tag", &tag);
                }
                if *id == BODY {
                    return self.reject(op, "reserved", "0");
                }
                if !self.nodes.contains_key(id) && self.nodes.len() > MAX_NODES {
                    return self.reject(op, "quota", "nodes");
                }
                // Like the PWX renderer, a reused id now names a new, detached node.
                self.detach(*id);
                self.nodes.insert(*id, MirrorNode::new(&tag));
            }
            StrictOp::Attr { id, name, value } => {
                let name = name.to_ascii_lowercase();
                let Some(node) = self.element(*id) else {
                    return;
                };
                if name == "src" && node.tag == "img" {
                    // PWX allows blob: images. The native host has no blob store yet.
                    return self.reject(op, "attr", "src-unsupported");
                }
                if !STRICT_ATTRS.contains(&name.as_str()) {
                    return self.reject(op, "attr", &name);
                }
                if !self.charge_text(op, value.len()) {
                    return;
                }
                let node = self.nodes.get_mut(id).expect("checked above");
                match node.attributes.iter_mut().find(|(n, _)| *n == name) {
                    Some(existing) => existing.1 = value.clone(),
                    None => node.attributes.push((name, value.clone())),
                }
            }
            StrictOp::Text { id, value } => {
                if self.element(*id).is_none() {
                    return;
                }
                if !self.charge_text(op, value.len()) {
                    return;
                }
                // textContent: every child goes, element children become detached.
                let old = std::mem::take(&mut self.nodes.get_mut(id).expect("checked").children);
                for child in old {
                    if let Child::Element(child_id) = child
                        && let Some(n) = self.nodes.get_mut(&child_id)
                    {
                        n.parent = None;
                    }
                }
                if !value.is_empty() {
                    let node = self.nodes.get_mut(id).expect("checked");
                    node.children.push(Child::Text(value.clone()));
                }
            }
            StrictOp::Prop { id, name, value } => {
                if self.element(*id).is_none() || !STRICT_PROPS.contains(&name.as_str()) {
                    return self.reject(op, "prop", name);
                }
                let node = self.nodes.get_mut(id).expect("checked");
                match (name.as_str(), value) {
                    ("value", PropValue::Text(text)) => node.value = text.clone(),
                    ("value", PropValue::Bool(b)) => node.value = b.to_string(),
                    ("checked", v) => node.checked = prop_truthy(v),
                    ("disabled", v) => node.disabled = prop_truthy(v),
                    _ => unreachable!("allowlist checked"),
                }
            }
            StrictOp::Append { parent, id } => {
                let parent_ok = *parent == BODY || self.element(*parent).is_some();
                if !parent_ok || self.element(*id).is_none() {
                    return;
                }
                if self.is_ancestor(*id, *parent) {
                    return self.reject(op, "cycle", &id.to_string());
                }
                self.detach(*id);
                self.nodes
                    .get_mut(parent)
                    .expect("checked")
                    .children
                    .push(Child::Element(*id));
                self.nodes.get_mut(id).expect("checked").parent = Some(*parent);
            }
            StrictOp::Remove { id } => {
                if self.element(*id).is_none() {
                    return;
                }
                self.detach(*id);
                self.nodes.remove(id);
                self.listeners.retain(|(node, _)| node != id);
                if self.focused == Some(*id) {
                    self.focused = None;
                }
            }
            StrictOp::Focus { id } => {
                if self.element(*id).is_some() {
                    self.focused = Some(*id);
                }
            }
            StrictOp::Listen { id, event } => {
                if self.element(*id).is_none() || !STRICT_EVENTS.contains(&event.as_str()) {
                    return self.reject(op, "event", event);
                }
                self.listeners.insert((*id, event.clone()));
            }
            StrictOp::Unknown { name } => return self.reject(op, "op", name),
        }
        self.epoch += 1;
    }

    /// A user action on node `id`. Typing updates the field itself, as a browser
    /// does. Returns the event to forward to the app only if it listens for it.
    pub fn user_event(&mut self, id: u64, event: &str, value: &str) -> Option<AppEvent> {
        let node = self.element(id)?;
        let is_field = node.tag == "input" || node.tag == "textarea";
        if event == "input" && is_field {
            self.nodes.get_mut(&id).expect("checked").value = value.to_string();
            self.epoch += 1;
        }
        self.listeners
            .contains(&(id, event.to_string()))
            .then(|| AppEvent {
                id,
                event: event.to_string(),
                value: is_field.then(|| value.to_string()),
            })
    }

    /// Replay a recorded stream. Returns the events the app would have received.
    pub fn replay(&mut self, lines: &[StrictLine]) -> Vec<AppEvent> {
        let mut delivered = Vec::new();
        for line in lines {
            match line {
                StrictLine::Op(op) => self.apply(op),
                StrictLine::Frame => {}
                StrictLine::UserEvent { id, event, value } => {
                    delivered.extend(self.user_event(*id, event, value));
                }
            }
        }
        delivered
    }

    /// Build a fresh native document from what is attached to the body.
    ///
    /// Form fields show their value as text, or their placeholder when empty,
    /// until the native engine has real form controls.
    pub fn snapshot(&self) -> StrictSnapshot {
        let mut document = NativeDocument::new();
        let html = document
            .append_element(document.root(), "html")
            .expect("root exists");
        let body = document.append_element(html, "body").expect("html exists");
        let mut native_for = BTreeMap::new();
        native_for.insert(BODY, body);
        self.emit_children(BODY, body, &mut document, &mut native_for);
        StrictSnapshot {
            document,
            native_for,
        }
    }

    fn emit_children(
        &self,
        app_id: u64,
        native_parent: NodeId,
        document: &mut NativeDocument,
        native_for: &mut BTreeMap<u64, NodeId>,
    ) {
        let Some(node) = self.nodes.get(&app_id) else {
            return;
        };
        if node.tag == "input" || node.tag == "textarea" {
            let shown = if node.value.is_empty() {
                node.attribute("placeholder").unwrap_or("")
            } else {
                node.value.as_str()
            };
            if !shown.is_empty() {
                document.append_text(native_parent, shown);
            }
            return;
        }
        for child in &node.children {
            match child {
                Child::Text(text) => {
                    document.append_text(native_parent, text.clone());
                }
                Child::Element(child_id) => {
                    let Some(child_node) = self.nodes.get(child_id) else {
                        continue;
                    };
                    let attributes = child_node
                        .attributes
                        .iter()
                        .map(|(name, value)| Attribute {
                            name: name.clone(),
                            value: value.clone(),
                        })
                        .collect();
                    let Some(native) = document.append_element_with_attributes(
                        native_parent,
                        child_node.tag.clone(),
                        attributes,
                    ) else {
                        continue;
                    };
                    native_for.insert(*child_id, native);
                    self.emit_children(*child_id, native, document, native_for);
                }
            }
        }
    }

    /// Serialize the attached tree as an HTML document, for differential
    /// comparison against Chromium, Firefox and WebKit.
    pub fn to_html(&self, stylesheet: &str) -> String {
        let mut out = String::from("<!doctype html><html><head><meta charset=\"utf-8\"><style>");
        out.push_str(&stylesheet.replace("</", "<\\/"));
        out.push_str("</style></head><body>");
        self.html_children(BODY, &mut out);
        out.push_str("</body></html>\n");
        out
    }

    fn html_children(&self, app_id: u64, out: &mut String) {
        let Some(node) = self.nodes.get(&app_id) else {
            return;
        };
        for child in &node.children {
            match child {
                Child::Text(text) => out.push_str(&escape_html(text)),
                Child::Element(id) => {
                    let Some(n) = self.nodes.get(id) else {
                        continue;
                    };
                    out.push('<');
                    out.push_str(&n.tag);
                    for (name, value) in &n.attributes {
                        out.push_str(&format!(" {name}=\"{}\"", escape_html(value)));
                    }
                    if n.tag == "input" && !n.value.is_empty() {
                        out.push_str(&format!(" value=\"{}\"", escape_html(&n.value)));
                    }
                    if n.checked {
                        out.push_str(" checked");
                    }
                    if n.disabled {
                        out.push_str(" disabled");
                    }
                    out.push('>');
                    match n.tag.as_str() {
                        "input" | "img" => continue,
                        "textarea" => out.push_str(&escape_html(&n.value)),
                        _ => self.html_children(*id, out),
                    }
                    out.push_str(&format!("</{}>", n.tag));
                }
            }
        }
    }
}

fn prop_truthy(value: &PropValue) -> bool {
    match value {
        PropValue::Bool(b) => *b,
        PropValue::Text(s) => !s.is_empty(),
    }
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub struct StrictSnapshot {
    pub document: NativeDocument,
    /// App node id -> native node id, for hit testing and event routing.
    pub native_for: BTreeMap<u64, NodeId>,
}

impl StrictSnapshot {
    pub fn scene(&self, sheet: &StyleSheet, viewport_width: f32) -> Result<Scene, CssError> {
        let layout = layout_document_with_styles(&self.document, sheet, viewport_width)?;
        Ok(scene_from_layout(&layout))
    }
}

// ---------------------------------------------------------------------------
// The shared PWX stylesheet on the native engine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// The native style profile of PWX `ui.css`.
///
/// Exactly two constructs in `ui.css` aren't native yet, and both are handled here
/// rather than approximated by the parser:
/// - `* { box-sizing: border-box; }`: the universal selector. Dropped; reported.
/// - `@media (prefers-color-scheme: dark) { :root { … } }`: the host already knows
///   its scheme, so the block is dropped for Light and its body applied for Dark.
///
/// Anything else the native parser rejects is an error, never silently skipped.
pub fn native_ui_stylesheet(
    ui_css: &str,
    scheme: ColorScheme,
) -> Result<(StyleSheet, Vec<String>), CssError> {
    let mut kept = String::new();
    let mut dark_body = String::new();
    let mut adapted = Vec::new();
    let mut rest = strip_comments(ui_css);
    loop {
        let trimmed = rest.trim_start();
        if trimmed.is_empty() {
            break;
        }
        let Some((prelude, block, after)) = next_block(trimmed) else {
            kept.push_str(trimmed);
            break;
        };
        let prelude_norm = prelude.split_whitespace().collect::<Vec<_>>().join(" ");
        if prelude_norm == "*" {
            adapted.push(format!("dropped universal rule: * {{{}}}", block.trim()));
        } else if prelude_norm == "@media (prefers-color-scheme: dark)" {
            adapted.push(format!("color scheme resolved by host: {scheme:?}"));
            dark_body = block.to_string();
        } else {
            kept.push_str(&format!("{prelude}{{{block}}}\n"));
        }
        rest = after.to_string();
    }
    if scheme == ColorScheme::Dark {
        kept.push_str(&dark_body);
        kept.push('\n');
    }
    Ok((parse_stylesheet(&kept)?, adapted))
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Split `prelude { block } rest` at the first top-level block.
fn next_block(input: &str) -> Option<(&str, &str, &str)> {
    let open = input.find('{')?;
    let mut depth = 0usize;
    for (offset, c) in input[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let close = open + offset;
                    return Some((&input[..open], &input[open + 1..close], &input[close + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_dom::NodeKind;
    #[cfg(feature = "taffy-layout")]
    use crate::native_scene::SceneCommand;

    const UI_CSS: &str = include_str!("../fixtures/pwx-strict/ui.css");
    const NOTES_EMPTY: &str = include_str!("../fixtures/pwx-strict/notes-empty.strict");
    const NOTES_SEEDED: &str = include_str!("../fixtures/pwx-strict/notes-seeded.strict");
    const NOTES_TYPED: &str = include_str!("../fixtures/pwx-strict/notes-new-typed.strict");

    fn op(line: &str) -> StrictOp {
        match parse_stream(line).unwrap().remove(0) {
            StrictLine::Op(op) => op,
            other => panic!("not an op: {other:?}"),
        }
    }

    fn texts(snapshot: &StrictSnapshot) -> Vec<String> {
        snapshot
            .document
            .nodes()
            .iter()
            .filter_map(|n| match &n.kind {
                NodeKind::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    #[cfg(feature = "taffy-layout")]
    fn scene_texts(scene: &Scene) -> Vec<String> {
        scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn run(stream: &str) -> (StrictBridge, Vec<AppEvent>) {
        let mut bridge = StrictBridge::new();
        let events = bridge.replay(&parse_stream(stream).unwrap());
        (bridge, events)
    }

    #[test]
    fn parses_escapes_and_rejects_malformed_lines() {
        let lines = parse_stream("text\t3\ta\\tb\\nc\\\\d\nframe\n").unwrap();
        assert_eq!(
            lines[0],
            StrictLine::Op(StrictOp::Text {
                id: 3,
                value: "a\tb\nc\\d".into()
            })
        );
        assert_eq!(lines[1], StrictLine::Frame);
        for bad in [
            "create\tx\tdiv",
            "create\t1",
            "prop\t1\tvalue\tz:1",
            "teleport\t1",
            "text\t1\t\\q",
        ] {
            assert!(parse_stream(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn enforces_the_pwx_allowlists() {
        let mut bridge = StrictBridge::new();
        for line in [
            "create\t1\tscript",
            "create\t2\tiframe",
            "create\t3\tdiv",
            "attr\t3\tonclick\talert(1)",
            "attr\t3\tstyle\tcolor:red",
            "attr\t3\thref\thttps://example.com",
            "prop\t3\tinnerHTML\ts:<b>x</b>",
            "listen\t3\tmouseover",
            "create\t4\timg",
            "attr\t4\tsrc\tblob:x",
            "unknown\teval",
            "create\t0\tdiv",
        ] {
            bridge.apply(&op(line));
        }
        let got: Vec<_> = bridge.rejected().iter().map(|r| r.0.as_str()).collect();
        assert_eq!(
            got,
            [
                "create:tag:script",
                "create:tag:iframe",
                "attr:attr:onclick",
                "attr:attr:style",
                "attr:attr:href",
                "prop:prop:innerHTML",
                "listen:event:mouseover",
                "attr:attr:src-unsupported",
                "op:op:eval",
                "create:reserved:0",
            ]
        );
        assert_eq!(bridge.node_count(), 2);
    }

    #[test]
    fn append_moves_and_refuses_cycles() {
        let mut bridge = StrictBridge::new();
        for line in [
            "create\t1\tdiv",
            "create\t2\tdiv",
            "create\t3\tp",
            "append\t1\t2",
            "append\t2\t3",
            "append\t3\t1",
            "append\t0\t1",
            "append\t1\t3",
        ] {
            bridge.apply(&op(line));
        }
        assert_eq!(bridge.rejected()[0].0, "append:cycle:1");
        let snap = bridge.snapshot();
        let body = snap.native_for[&BODY];
        let div1 = snap.native_for[&1];
        assert_eq!(snap.document.element_children(body), vec![div1]);
        // 3 moved from 2 to 1: div1 has two element children, 2 then 3.
        assert_eq!(
            snap.document.element_children(div1),
            vec![snap.native_for[&2], snap.native_for[&3]]
        );
        assert!(
            snap.document
                .element_children(snap.native_for[&2])
                .is_empty()
        );
    }

    #[test]
    fn text_replaces_children_and_remove_detaches() {
        let mut bridge = StrictBridge::new();
        for line in [
            "create\t1\tdiv",
            "create\t2\tspan",
            "append\t1\t2",
            "append\t0\t1",
            "text\t1\thello",
            "create\t3\tp",
            "append\t0\t3",
            "listen\t3\tclick",
            "remove\t3",
        ] {
            bridge.apply(&op(line));
        }
        let snap = bridge.snapshot();
        assert_eq!(texts(&snap), ["hello"]);
        assert!(!snap.native_for.contains_key(&2));
        assert!(!snap.native_for.contains_key(&3));
        assert_eq!(bridge.user_event(3, "click", ""), None);
    }

    #[test]
    fn quotas_fail_closed() {
        let mut bridge = StrictBridge::new();
        bridge.apply(&op("create\t1\tp"));
        let big = "x".repeat(MAX_TEXT_BYTES);
        bridge.apply(&StrictOp::Text {
            id: 1,
            value: big.clone(),
        });
        bridge.apply(&StrictOp::Text { id: 1, value: big });
        assert_eq!(bridge.rejected().last().unwrap().0, "text:quota:text");
    }

    #[test]
    fn notes_first_run_shows_the_empty_state() {
        let (bridge, events) = run(NOTES_EMPTY);
        assert!(events.is_empty());
        assert!(bridge.rejected().is_empty(), "{:?}", bridge.rejected());
        let snap = bridge.snapshot();
        let shown = texts(&snap);
        for expected in [
            "Notes",
            "Stored only on this device",
            "New note",
            "No notes yet. Create one to start.",
        ] {
            assert!(shown.iter().any(|t| t == expected), "{expected}");
        }
    }

    #[test]
    fn notes_seeded_lists_newest_first_and_opens_it() {
        let (bridge, _) = run(NOTES_SEEDED);
        assert!(bridge.rejected().is_empty(), "{:?}", bridge.rejected());
        let shown = texts(&bridge.snapshot());
        let swap = shown.iter().position(|t| t == "Shift swap").unwrap();
        let groceries = shown.iter().position(|t| t == "Groceries").unwrap();
        assert!(swap < groceries, "newest first");
        assert!(shown.iter().any(|t| t == "Ask about Thursday night"));
        let html = bridge.to_html("");
        assert!(html.contains("<button class=\"item\" aria-current=\"true\">Shift swap</button>"));
        assert!(html.contains("<input class=\"field field-title\" placeholder=\"Title\" aria-label=\"Title\" value=\"Shift swap\">"));
    }

    #[test]
    fn notes_new_note_and_typing_reach_the_app() {
        let (bridge, events) = run(NOTES_TYPED);
        assert!(bridge.rejected().is_empty(), "{:?}", bridge.rejected());
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event, "click");
        assert_eq!(events[1].value.as_deref(), Some("Hello from Altru Metal"));
        let shown = texts(&bridge.snapshot());
        assert!(shown.iter().any(|t| t == "Hello from Altru Metal"));
        assert!(
            shown
                .iter()
                .any(|t| t == "Typed into a PWX app, drawn by our own engine.")
        );
        assert!(shown.iter().any(|t| t == "Saved on this device"));
    }

    #[test]
    fn ui_css_has_a_native_profile() {
        let (_, adapted) = native_ui_stylesheet(UI_CSS, ColorScheme::Light).unwrap();
        assert_eq!(adapted.len(), 2, "{adapted:?}");
        let (_, adapted) = native_ui_stylesheet(UI_CSS, ColorScheme::Dark).unwrap();
        assert!(adapted.iter().any(|a| a.contains("Dark")));
        assert!(
            native_ui_stylesheet(
                "@media (prefers-reduced-motion: reduce) { p { font-size: 2px } }",
                ColorScheme::Light
            )
            .is_err()
        );
    }

    /// ui.css lays the app out with flexbox, which needs the Taffy geometry adapter.
    #[cfg(not(feature = "taffy-layout"))]
    #[test]
    fn notes_layout_fails_closed_without_the_flex_adapter() {
        let (sheet, _) = native_ui_stylesheet(UI_CSS, ColorScheme::Light).unwrap();
        let (bridge, _) = run(NOTES_SEEDED);
        assert!(matches!(
            bridge.snapshot().scene(&sheet, 1600.0),
            Err(CssError::UnsupportedLayout(_))
        ));
    }

    #[cfg(feature = "taffy-layout")]
    #[test]
    fn notes_render_through_native_style_and_layout() {
        let (sheet, _) = native_ui_stylesheet(UI_CSS, ColorScheme::Light).unwrap();
        let (bridge, _) = run(NOTES_SEEDED);
        let scene = bridge.snapshot().scene(&sheet, 1600.0).unwrap();
        let drawn = scene_texts(&scene);
        assert!(drawn.iter().any(|t| t.contains("Notes")), "{drawn:?}");
        assert!(drawn.iter().any(|t| t.contains("Groceries")), "{drawn:?}");
        // The empty-state paragraph is display:none while notes exist.
        assert!(
            !drawn.iter().any(|t| t.contains("No notes yet")),
            "{drawn:?}"
        );
        let again = bridge.snapshot().scene(&sheet, 1600.0).unwrap();
        assert_eq!(scene, again, "deterministic");
    }
}
