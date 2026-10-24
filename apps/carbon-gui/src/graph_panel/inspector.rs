//! The inspector beside the canvas: the selected block's title and fields
//! (from its node type), an arrow's ends, or with nothing selected the graph's
//! name and graph-level fields; problems in each case. Every change goes
//! through the graph's undo history; typing in one box is one step.

use std::collections::BTreeSet;

use carbon_core::extension::GraphKind;
use carbon_core::graph::{FieldKind, FieldSpec, GraphEdit, Severity};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{
    Input, InputEvent, InputState, NumberInput, Textarea, TextareaState,
};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::{GraphPanel, InspectorTab};
use crate::style::fixed;

/// A text box of the inspector, kept while its block stays selected.
pub(super) enum InspectorInput {
    Line(Entity<InputState>),
    Area(Entity<TextareaState>),
}

/// What a field belongs to.
#[derive(Clone, Debug, PartialEq)]
enum Target {
    Node(String),
    /// A graph-level field: a dotted key under the file's top level.
    Graph,
}

/// Where a value sits: a field, or one key of one item of a list field.
#[derive(Clone, Debug, PartialEq)]
struct Slot {
    target: Target,
    key: String,
    item: Option<(usize, String)>,
}

impl Slot {
    fn id(&self) -> String {
        let target = match &self.target {
            Target::Node(id) => format!("n:{id}"),
            Target::Graph => "g".into(),
        };
        match &self.item {
            Some((ix, key)) => format!("{target}:{}:{ix}:{key}", self.key),
            None => format!("{target}:{}", self.key),
        }
    }
}

pub(super) fn render_inspector(
    panel: &mut GraphPanel,
    kind: &GraphKind,
    window: &mut Window,
    cx: &mut Context<GraphPanel>,
) -> AnyElement {
    let tabs = tab_row(panel, cx);
    let body: Vec<AnyElement> = if panel.tab == InspectorTab::Run {
        super::run_view::render_run(panel, cx)
    } else if let Some(edge) = panel.selected_edge.clone() {
        edge_section(panel, &edge, cx)
    } else {
        match panel.selected.len() {
            0 => graph_section(panel, kind, window, cx),
            1 => {
                let id = panel.selected.iter().next().cloned().unwrap_or_default();
                node_section(panel, kind, &id, window, cx)
            }
            n => many_section(n, cx),
        }
    };
    v_flex()
        .size_full()
        .child(tabs)
        .child(
            v_flex()
                .id("graph-inspector")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(fixed::GRAPH_INSPECTOR_PADDING)
                .gap(fixed::GRAPH_INSPECTOR_GAP)
                .children(body),
        )
        .into_any_element()
}

/// Edit | Run, with a dot on Run while something waits for the user.
fn tab_row(panel: &GraphPanel, cx: &mut Context<GraphPanel>) -> AnyElement {
    let run = panel.run.as_ref().map(|r| r.state.id);
    let (approvals, permissions) = super::run_view::waiting(run, cx);
    let waiting = run.is_some() && approvals.len() + permissions.len() > 0;
    let tab = |id: &'static str, label: &'static str, which: InspectorTab| {
        let selected = panel.tab == which;
        Button::new(id)
            .label(label)
            .small()
            .when(selected, |b| b.primary())
            .when(!selected, |b| b.ghost())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.tab = which;
                cx.notify()
            }))
    };
    h_flex()
        .flex_none()
        .gap_1()
        .px(fixed::GRAPH_INSPECTOR_PADDING)
        .py_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(tab("graph-tab-edit", "Edit", InspectorTab::Edit))
        .child(tab("graph-tab-run", "Run", InspectorTab::Run))
        .when(waiting, |row| {
            row.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .child("Waiting for you"),
            )
        })
        .into_any_element()
}

fn heading(text: impl Into<SharedString>, detail: Option<String>, cx: &App) -> AnyElement {
    v_flex()
        .gap_0p5()
        .child(div().font_weight(FontWeight::SEMIBOLD).child(text.into()))
        .when_some(detail, |col, detail| {
            col.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail),
            )
        })
        .into_any_element()
}

fn labeled(label: &str, description: &str, control: AnyElement, cx: &App) -> AnyElement {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(label.to_string()),
        )
        .when(!description.is_empty(), |col| {
            col.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(description.to_string()),
            )
        })
        .child(control)
        .into_any_element()
}

fn problems_list(problems: Vec<(Severity, String)>, cx: &App) -> Option<AnyElement> {
    if problems.is_empty() {
        return None;
    }
    let theme = cx.theme();
    Some(
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Problems"),
            )
            .children(problems.into_iter().map(|(severity, text)| {
                div()
                    .text_xs()
                    .text_color(match severity {
                        Severity::Error => theme.danger,
                        Severity::Warning => theme.warning,
                        Severity::Info => theme.muted_foreground,
                    })
                    .child(text)
            }))
            .into_any_element(),
    )
}

fn graph_section(
    panel: &mut GraphPanel,
    kind: &GraphKind,
    window: &mut Window,
    cx: &mut Context<GraphPanel>,
) -> Vec<AnyElement> {
    let Some(doc) = &panel.doc else {
        return Vec::new();
    };
    let name = doc.name.clone();
    let counts = format!("{} blocks, {} arrows", doc.nodes.len(), doc.edges.len());
    let mut out = vec![heading(kind.spec.name.clone(), Some(counts), cx)];
    let name_input = panel.line_input("g:name", name, "", window, cx, |this, text, cx| {
        let Some(doc) = &this.doc else {
            return;
        };
        if doc.name != text {
            let edit = GraphEdit::SetName {
                old: doc.name.clone(),
                new: text,
            };
            this.edit(edit, Some("g:name".into()), cx);
        }
    });
    out.push(labeled(
        "Name",
        "",
        Input::new(&name_input).small().into_any_element(),
        cx,
    ));
    for field in kind.spec.fields.clone() {
        let slot = Slot {
            target: Target::Graph,
            key: field.key.clone(),
            item: None,
        };
        out.push(field_control(panel, &field, slot, window, cx));
    }
    let problems = panel
        .problems_of(None)
        .into_iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    out.extend(problems_list(problems, cx));
    out.push(
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(
                "Drag from a block's right-side dot to another block's left-side dot to connect them. \
                 Space or the middle button drags the view; the wheel zooms; 0 shows everything.",
            )
            .into_any_element(),
    );
    out
}

fn many_section(count: usize, cx: &mut Context<GraphPanel>) -> Vec<AnyElement> {
    vec![
        heading(format!("{count} blocks selected"), None, cx),
        h_flex()
            .gap_1()
            .child(
                Button::new("graph-duplicate-many")
                    .label("Duplicate")
                    .small()
                    .on_click(
                        cx.listener(|this, _, w, cx| this.duplicate(&super::Duplicate, w, cx)),
                    ),
            )
            .child(
                Button::new("graph-delete-many")
                    .label("Delete")
                    .small()
                    .danger()
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.delete_selection(&super::DeleteSelection, w, cx)
                    })),
            )
            .into_any_element(),
    ]
}

fn edge_section(panel: &GraphPanel, edge: &str, cx: &mut Context<GraphPanel>) -> Vec<AnyElement> {
    let Some(found) = panel.doc.as_ref().and_then(|doc| doc.edge(edge)).cloned() else {
        return Vec::new();
    };
    let mut out = vec![heading(
        "Arrow",
        Some(format!("{} → {}", found.from, found.to)),
        cx,
    )];
    let problems = panel
        .edge_problems(edge)
        .into_iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    out.extend(problems_list(problems, cx));
    out.push(
        Button::new("graph-delete-edge")
            .label("Delete")
            .small()
            .danger()
            .on_click(
                cx.listener(|this, _, w, cx| this.delete_selection(&super::DeleteSelection, w, cx)),
            )
            .into_any_element(),
    );
    out
}

fn node_section(
    panel: &mut GraphPanel,
    kind: &GraphKind,
    id: &str,
    window: &mut Window,
    cx: &mut Context<GraphPanel>,
) -> Vec<AnyElement> {
    let Some(node) = panel
        .doc
        .as_ref()
        .and_then(|doc| doc.nodes.get(id))
        .cloned()
    else {
        return Vec::new();
    };
    let spec = kind.node_type(&node.kind).cloned();
    let type_title = spec
        .as_ref()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| node.kind.clone());
    let mut out = vec![heading(type_title.clone(), Some(format!("id: {id}")), cx)];
    if let Some(spec) = &spec
        && !spec.description.is_empty()
    {
        out.push(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(spec.description.clone())
                .into_any_element(),
        );
    }
    let node_id = id.to_string();
    let title_id = format!("n:{id}:title");
    let title_input = panel.line_input(
        &title_id,
        node.title.clone().unwrap_or_default(),
        &type_title,
        window,
        cx,
        {
            let node_id = node_id.clone();
            move |this, text, cx| {
                let Some(node) = this.doc.as_ref().and_then(|d| d.nodes.get(&node_id)) else {
                    return;
                };
                let new = (!text.trim().is_empty()).then_some(text);
                if node.title != new {
                    let edit = GraphEdit::SetTitle {
                        node: node_id.clone(),
                        old: node.title.clone(),
                        new,
                    };
                    this.edit(edit, Some(format!("n:{node_id}:title")), cx);
                }
            }
        },
    );
    out.push(labeled(
        "Title",
        "",
        Input::new(&title_input).small().into_any_element(),
        cx,
    ));
    if let Some(spec) = &spec {
        for field in &spec.fields {
            let slot = Slot {
                target: Target::Node(node_id.clone()),
                key: field.key.clone(),
                item: None,
            };
            out.push(field_control(panel, field, slot, window, cx));
        }
    }
    let disabled = node.disabled;
    out.push(
        h_flex()
            .gap_2()
            .child(
                Switch::new("graph-node-disabled")
                    .checked(disabled)
                    .on_click(cx.listener({
                        let node_id = node_id.clone();
                        move |this, checked: &bool, _, cx| {
                            let edit = GraphEdit::SetDisabled {
                                node: node_id.clone(),
                                disabled: *checked,
                            };
                            this.edit(edit, None, cx)
                        }
                    })),
            )
            .child(div().text_sm().child("Turned off (skipped when run)"))
            .into_any_element(),
    );
    let problems = panel
        .problems_of(Some(id))
        .into_iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    out.extend(problems_list(problems, cx));
    out.push(
        h_flex()
            .gap_1()
            .child(
                Button::new("graph-duplicate")
                    .label("Duplicate")
                    .small()
                    .on_click(
                        cx.listener(|this, _, w, cx| this.duplicate(&super::Duplicate, w, cx)),
                    ),
            )
            .child(
                Button::new("graph-delete")
                    .label("Delete")
                    .small()
                    .danger()
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.delete_selection(&super::DeleteSelection, w, cx)
                    })),
            )
            .into_any_element(),
    );
    out
}

/// The control for one field (or one key of a list item).
fn field_control(
    panel: &mut GraphPanel,
    field: &FieldSpec,
    slot: Slot,
    window: &mut Window,
    cx: &mut Context<GraphPanel>,
) -> AnyElement {
    let value = panel.slot_value(&slot).or_else(|| field.default_json());
    let control: AnyElement = match field.kind {
        FieldKind::Text | FieldKind::File => {
            let text = value
                .as_ref()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let set = {
                let slot = slot.clone();
                move |this: &mut GraphPanel, text: String, cx: &mut Context<GraphPanel>| {
                    this.set_slot(&slot, Some(json!(text)), true, cx)
                }
            };
            if field.multiline {
                let area = panel.area_input(&slot.id(), text, window, cx, set);
                Textarea::new(&area).small().into_any_element()
            } else {
                let line = panel.line_input(&slot.id(), text, "", window, cx, set);
                Input::new(&line).small().into_any_element()
            }
        }
        FieldKind::Integer | FieldKind::Number => {
            let text = match value.as_ref().and_then(Value::as_f64) {
                Some(n) if n.fract() == 0. => format!("{}", n as i64),
                Some(n) => n.to_string(),
                None => String::new(),
            };
            let integer = field.kind == FieldKind::Integer;
            let (min, max) = (field.min, field.max);
            let line = panel.line_input(&slot.id(), text, "", window, cx, {
                let slot = slot.clone();
                move |this, text, cx| {
                    let Ok(mut n) = text.trim().parse::<f64>() else {
                        return;
                    };
                    if let Some(min) = min {
                        n = n.max(min);
                    }
                    if let Some(max) = max {
                        n = n.min(max);
                    }
                    let value = if integer || n.fract() == 0. {
                        json!(n.round() as i64)
                    } else {
                        json!(n)
                    };
                    this.set_slot(&slot, Some(value), true, cx)
                }
            });
            NumberInput::new(&line)
                .small()
                .w(fixed::SETTINGS_NUMBER_WIDTH)
                .into_any_element()
        }
        FieldKind::Bool => Switch::new(SharedString::from(format!("graph-bool-{}", slot.id())))
            .checked(value.as_ref().and_then(Value::as_bool).unwrap_or_default())
            .on_click(cx.listener({
                let slot = slot.clone();
                move |this, checked: &bool, _, cx| {
                    this.set_slot(&slot, Some(json!(*checked)), false, cx)
                }
            }))
            .into_any_element(),
        FieldKind::Choice | FieldKind::Agent => {
            let choices: Vec<(String, String)> = if field.kind == FieldKind::Agent {
                let registry = crate::extensions::registry(cx);
                registry
                    .active()
                    .flat_map(|loaded| loaded.def.agents.iter())
                    .map(|agent| (agent.id.clone(), agent.name.clone()))
                    .collect()
            } else {
                field
                    .choices
                    .iter()
                    .map(|c| (c.value.clone(), c.label.clone()))
                    .collect()
            };
            let current = value
                .as_ref()
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let label = choices
                .iter()
                .find(|(v, _)| *v == current)
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| {
                    if current.is_empty() {
                        "Choose…".into()
                    } else {
                        current.clone()
                    }
                });
            let entity = cx.entity();
            Button::new(SharedString::from(format!("graph-choice-{}", slot.id())))
                .label(label)
                .small()
                .outline()
                .dropdown_caret(true)
                .dropdown_menu(move |menu, _, _| {
                    choices.iter().fold(menu, |menu, (value, label)| {
                        let (entity, slot, value) = (entity.clone(), slot.clone(), value.clone());
                        menu.item(
                            PopupMenuItem::new(label.clone())
                                .checked(value == current)
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.set_slot(&slot, Some(json!(value)), false, cx)
                                    })
                                }),
                        )
                    })
                })
                .into_any_element()
        }
        FieldKind::List => list_control(panel, field, slot, value, window, cx),
    };
    labeled(&field.label, &field.description, control, cx)
}

/// A list field (a router's routes): each item's fields, Remove, and Add.
/// Items keep an `id` (`route-1`), which names the port they make.
fn list_control(
    panel: &mut GraphPanel,
    field: &FieldSpec,
    slot: Slot,
    value: Option<Value>,
    window: &mut Window,
    cx: &mut Context<GraphPanel>,
) -> AnyElement {
    let items = value
        .as_ref()
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut col = v_flex().gap_2();
    for (ix, item) in items.iter().enumerate() {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut row = v_flex()
            .gap_1()
            .p_2()
            .rounded(px(4.))
            .border_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(id),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "graph-list-remove-{}-{ix}",
                            slot.id()
                        )))
                        .icon(IconName::Close)
                        .ghost()
                        .xsmall()
                        .tooltip("Remove")
                        .on_click(cx.listener({
                            let slot = slot.clone();
                            move |this, _, _, cx| this.remove_item(&slot, ix, cx)
                        })),
                    ),
            );
        for item_field in &field.item_fields {
            let item_slot = Slot {
                item: Some((ix, item_field.key.clone())),
                ..slot.clone()
            };
            row = row.child(field_control(panel, item_field, item_slot, window, cx));
        }
        col = col.child(row);
    }
    col.child(
        Button::new(SharedString::from(format!("graph-list-add-{}", slot.id())))
            .label("Add")
            .icon(IconName::Plus)
            .small()
            .on_click(cx.listener({
                let (slot, field) = (slot.clone(), field.clone());
                move |this, _, _, cx| this.add_item(&slot, &field, cx)
            })),
    )
    .into_any_element()
}

impl GraphPanel {
    /// A one-line box: made once per slot; while it is not being typed in it
    /// follows `text` (undo, other edits). `apply` gets the text on change.
    fn line_input(
        &mut self,
        id: &str,
        text: String,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Self, String, &mut Context<Self>) + 'static,
    ) -> Entity<InputState> {
        if let Some(InspectorInput::Line(input)) = self.inputs.get(id) {
            let input = input.clone();
            if !input.focus_handle(cx).is_focused(window) && input.read(cx).value() != text.as_str()
            {
                input.update(cx, |input, cx| input.set_value(text, window, cx));
            }
            return input;
        }
        let placeholder = placeholder.to_string();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(text)
                .placeholder(placeholder)
        });
        let sub = cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    apply(this, text, cx);
                }
            },
        );
        self.inputs
            .insert(id.to_string(), InspectorInput::Line(input.clone()));
        self.input_subs.push(sub);
        input
    }

    /// A multi-line box, like `line_input`.
    fn area_input(
        &mut self,
        id: &str,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Self, String, &mut Context<Self>) + 'static,
    ) -> Entity<TextareaState> {
        if let Some(InspectorInput::Area(input)) = self.inputs.get(id) {
            let input = input.clone();
            if !input.focus_handle(cx).is_focused(window) && input.read(cx).value() != text.as_str()
            {
                input.update(cx, |input, cx| input.set_value(text, window, cx));
            }
            return input;
        }
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 14)
                .default_value(text)
        });
        let sub = cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    apply(this, text, cx);
                }
            },
        );
        self.inputs
            .insert(id.to_string(), InspectorInput::Area(input.clone()));
        self.input_subs.push(sub);
        input
    }

    /// The value at `slot`, if set.
    fn slot_value(&self, slot: &Slot) -> Option<Value> {
        let doc = self.doc.as_ref()?;
        let base = match &slot.target {
            Target::Node(id) => doc.nodes.get(id)?.config.get(&slot.key)?.clone(),
            Target::Graph => meta_get(&doc.meta, &slot.key)?,
        };
        match &slot.item {
            None => Some(base),
            Some((ix, key)) => base.as_array()?.get(*ix)?.get(key).cloned(),
        }
    }

    /// Sets the value at `slot` (`None` removes it). `merge`: typing, so a
    /// run of changes is one undo step.
    fn set_slot(&mut self, slot: &Slot, value: Option<Value>, merge: bool, cx: &mut Context<Self>) {
        let Some(doc) = &self.doc else {
            return;
        };
        let merge_key = merge.then(|| slot.id());
        // A list item's key: the whole list changes.
        let (key, value) = match &slot.item {
            None => (slot.key.clone(), value),
            Some((ix, item_key)) => {
                let list_slot = Slot {
                    item: None,
                    ..slot.clone()
                };
                let mut items = self
                    .slot_value(&list_slot)
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default();
                let Some(item) = items.get_mut(*ix).and_then(Value::as_object_mut) else {
                    return;
                };
                match value {
                    Some(value) => {
                        item.insert(item_key.clone(), value);
                    }
                    None => {
                        item.remove(item_key);
                    }
                }
                (slot.key.clone(), Some(Value::Array(items)))
            }
        };
        let edit = match &slot.target {
            Target::Node(id) => GraphEdit::set_field(doc, id, &key, value),
            Target::Graph => {
                let top = key.split('.').next().unwrap_or_default().to_string();
                let old = doc.meta.get(&top).cloned();
                let new = meta_set(old.clone(), &key, value);
                (old != new).then_some(GraphEdit::SetMeta { key: top, old, new })
            }
        };
        if let Some(edit) = edit {
            self.edit(edit, merge_key, cx);
        }
    }

    fn add_item(&mut self, slot: &Slot, field: &FieldSpec, cx: &mut Context<Self>) {
        let mut items = self
            .slot_value(slot)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let taken: BTreeSet<String> = items
            .iter()
            .filter_map(|i| i.get("id").and_then(Value::as_str).map(str::to_string))
            .collect();
        let base = singular(&field.key);
        let id = (1..)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !taken.contains(id))
            .expect("some number is free");
        let mut item = serde_json::Map::new();
        item.insert("id".into(), json!(id));
        for f in &field.item_fields {
            if let Some(value) = f.default_json() {
                item.insert(f.key.clone(), value);
            }
        }
        items.push(Value::Object(item));
        self.set_slot(slot, Some(Value::Array(items)), false, cx);
    }

    fn remove_item(&mut self, slot: &Slot, ix: usize, cx: &mut Context<Self>) {
        let mut items = self
            .slot_value(slot)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        if ix < items.len() {
            items.remove(ix);
            // Boxes of later items now show other items: make them again.
            self.inputs
                .retain(|id, _| !id.starts_with(&format!("{}:", slot.id())));
            self.set_slot(slot, Some(Value::Array(items)), false, cx);
        }
    }
}

/// `routes` -> `route`, for new item ids.
fn singular(key: &str) -> String {
    let last = key.rsplit('.').next().unwrap_or(key);
    last.strip_suffix('s').unwrap_or(last).replace('_', "-")
}

/// A dotted key's value under the graph's top level (`budget.max_cost_usd`).
fn meta_get(meta: &indexmap::IndexMap<String, Value>, key: &str) -> Option<Value> {
    let mut parts = key.split('.');
    let mut value = meta.get(parts.next()?)?;
    for part in parts {
        value = value.get(part)?;
    }
    Some(value.clone())
}

/// The new top-level value after setting a dotted key inside `old`.
fn meta_set(old: Option<Value>, key: &str, value: Option<Value>) -> Option<Value> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() == 1 {
        return value;
    }
    let mut root = old.unwrap_or_else(|| json!({}));
    if !root.is_object() {
        root = json!({});
    }
    let mut at = &mut root;
    for part in &parts[1..parts.len() - 1] {
        let map = at.as_object_mut().expect("objects all the way down");
        at = map.entry(part.to_string()).or_insert_with(|| json!({}));
        if !at.is_object() {
            *at = json!({});
        }
    }
    let map = at.as_object_mut().expect("an object");
    let last = parts[parts.len() - 1].to_string();
    match value {
        Some(value) => {
            map.insert(last, value);
        }
        None => {
            map.remove(&last);
        }
    }
    let empty = root.as_object().is_some_and(|m| m.is_empty());
    (!empty).then_some(root)
}

#[cfg(test)]
mod tests {
    use super::{meta_get, meta_set, singular};
    use serde_json::json;

    #[test]
    fn dotted_graph_keys_nest() {
        let set = meta_set(
            Some(json!({ "max_tokens": 5 })),
            "budget.max_cost_usd",
            Some(json!(2)),
        );
        assert_eq!(set, Some(json!({ "max_tokens": 5, "max_cost_usd": 2 })));
        let mut meta = indexmap::IndexMap::new();
        meta.insert("budget".to_string(), set.unwrap());
        assert_eq!(meta_get(&meta, "budget.max_cost_usd"), Some(json!(2)));
        let cleared = meta_set(Some(json!({ "x": 1 })), "budget.x", None);
        assert_eq!(cleared, None);
        assert_eq!(singular("routes"), "route");
    }
}
