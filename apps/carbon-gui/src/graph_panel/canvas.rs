//! The canvas: a GPUI `canvas` paints the dot grid, the arrows (cubic
//! curves), the arrow being drawn and the selection box; blocks are divs on
//! top. Every press is hit-tested here against the geometry (ports, blocks,
//! arrows), so blocks need no handlers of their own.
//!
//! Mouse: press a block to select and drag it (Shift adds to the selection),
//! press an output port and drag to an input to connect, press an arrow to
//! select it, press empty canvas to draw a selection box; Space + drag or the
//! middle button pans; the wheel zooms at the pointer (a trackpad pans, with
//! Ctrl it zooms).

use std::collections::BTreeSet;

use carbon_core::extension::GraphKind;
use carbon_core::graph::{GraphEdit, Pos};
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::geometry::{
    self, EDGE_HIT, GRID, HEADER_HEIGHT, NODE_WIDTH, NodeBox, ROW_HEIGHT, SUMMARY_HEIGHT, View,
};
use super::{
    Cancel, DeleteSelection, Drag, Duplicate, GraphPanel, KEY_CONTEXT, SelectAll, ZoomFit, ZoomIn,
    ZoomOut, ZoomReset,
};
use crate::style::fixed;
use carbon_core::graph::Severity;

/// One arrow to paint, in canvas-local px.
struct Arrow {
    curve: [Point<Pixels>; 4],
    selected: bool,
    loop_back: bool,
    problem: bool,
}

/// What the paint pass draws.
struct Scene {
    view: View,
    arrows: Vec<Arrow>,
    preview: Option<[Point<Pixels>; 4]>,
    marquee: Option<Bounds<Pixels>>,
    grid: Hsla,
    arrow: Hsla,
    accent: Hsla,
    danger: Hsla,
}

pub(super) fn render_canvas(
    panel: &mut GraphPanel,
    kind: &GraphKind,
    _: &mut Window,
    cx: &mut Context<GraphPanel>,
) -> AnyElement {
    let theme = cx.theme();
    let Some(doc) = &panel.doc else {
        return div().into_any_element();
    };
    let view = panel.view;
    let boxes = geometry::boxes(doc, &kind.node_types);
    let find = |id: &str| boxes.iter().find(|b| b.id == id);
    let arrows: Vec<Arrow> = doc
        .edges
        .iter()
        .filter_map(|edge| {
            let from = find(&edge.from.node)?.port(true, &edge.from.port)?;
            let to_box = find(&edge.to.node)?;
            let to = to_box.port(false, &edge.to.port)?;
            let loop_back = to_box
                .inputs
                .iter()
                .any(|p| p.id == edge.to.port && p.loop_back);
            Some(Arrow {
                curve: geometry::curve(
                    view.to_screen(from.0, from.1),
                    view.to_screen(to.0, to.1),
                    view.zoom,
                ),
                selected: panel.selected_edge.as_deref() == Some(edge.id.as_str()),
                loop_back,
                problem: !panel.edge_problems(&edge.id).is_empty(),
            })
        })
        .collect();
    let preview = match &panel.drag {
        Some(Drag::Connect { from, to }) => find(&from.node)
            .and_then(|b| b.port(true, &from.port))
            .map(|(x, y)| geometry::curve(view.to_screen(x, y), *to, view.zoom)),
        _ => None,
    };
    let marquee = match &panel.drag {
        Some(Drag::Marquee { start, current, .. }) => Some(geometry::rect(*start, *current)),
        _ => None,
    };
    let scene = Scene {
        view,
        arrows,
        preview,
        marquee,
        grid: theme.border,
        arrow: theme.muted_foreground.opacity(0.75),
        accent: theme.ring,
        danger: theme.danger,
    };
    let bounds_cell = panel.bounds.clone();
    let nodes: Vec<AnyElement> = boxes
        .iter()
        .map(|b| render_node(panel, kind, b, cx))
        .collect();
    let empty = doc.nodes.is_empty();
    let muted = theme.muted_foreground;
    let background = theme.background;
    let grabbing = matches!(panel.drag, Some(Drag::Pan { .. }) | Some(Drag::Move { .. }));

    div()
        .id("graph-canvas")
        .key_context(KEY_CONTEXT)
        .track_focus(&panel.canvas_focus)
        .relative()
        .size_full()
        .overflow_hidden()
        .bg(background)
        .when(panel.space_down || grabbing, |canvas| {
            canvas.cursor(CursorStyle::ClosedHand)
        })
        .on_action(cx.listener(|this, _: &DeleteSelection, w, cx| {
            this.delete_selection(&DeleteSelection, w, cx)
        }))
        .on_action(cx.listener(|this, _: &Duplicate, w, cx| this.duplicate(&Duplicate, w, cx)))
        .on_action(cx.listener(|this, _: &SelectAll, w, cx| this.select_all(&SelectAll, w, cx)))
        .on_action(cx.listener(|this, _: &ZoomIn, w, cx| this.zoom_in(&ZoomIn, w, cx)))
        .on_action(cx.listener(|this, _: &ZoomOut, w, cx| this.zoom_out(&ZoomOut, w, cx)))
        .on_action(cx.listener(|this, _: &ZoomFit, w, cx| this.zoom_fit(&ZoomFit, w, cx)))
        .on_action(cx.listener(|this, _: &ZoomReset, w, cx| this.zoom_reset(&ZoomReset, w, cx)))
        .on_action(cx.listener(|this, _: &Cancel, w, cx| this.cancel(&Cancel, w, cx)))
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "space" && !this.space_down {
                this.space_down = true;
                cx.notify();
            }
        }))
        .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
            if event.keystroke.key == "space" {
                this.space_down = false;
                cx.notify();
            }
        }))
        .on_mouse_down(MouseButton::Left, cx.listener(GraphPanel::press))
        .on_mouse_down(MouseButton::Middle, cx.listener(GraphPanel::press_middle))
        .on_mouse_move(cx.listener(GraphPanel::pointer_move))
        .on_mouse_up(MouseButton::Left, cx.listener(GraphPanel::release))
        .on_mouse_up(MouseButton::Middle, cx.listener(GraphPanel::release))
        .on_scroll_wheel(cx.listener(GraphPanel::wheel))
        .child(
            canvas(
                move |bounds, _, _| bounds_cell.set(bounds),
                move |bounds, _, window, _| paint(&scene, bounds, window),
            )
            .absolute()
            .size_full(),
        )
        .children(nodes)
        .when(empty, |canvas| {
            canvas.child(
                v_flex()
                    .absolute()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(muted)
                    .child("Add a block with Add in the toolbar."),
            )
        })
        .into_any_element()
}

/// One block: title row, a row per port pair with labels, the summary line,
/// and port dots on its edges.
fn render_node(
    panel: &GraphPanel,
    kind: &GraphKind,
    b: &NodeBox,
    cx: &Context<GraphPanel>,
) -> AnyElement {
    let theme = cx.theme();
    let view = panel.view;
    let z = view.zoom;
    let Some(doc) = &panel.doc else {
        return div().into_any_element();
    };
    let node = &doc.nodes[&b.id];
    let spec = kind.node_type(&node.kind);
    let title = node
        .title
        .clone()
        .or_else(|| spec.map(|s| s.title.clone()))
        .unwrap_or_else(|| node.kind.clone());
    let selected = panel.selected.contains(&b.id);
    let node_run = panel
        .run
        .as_ref()
        .and_then(|run| run.state.nodes.get(&b.id))
        .cloned()
        .unwrap_or_default();
    let run_color = super::run_view::state_color(node_run.state, cx);
    let problems = panel.problems_of(Some(&b.id));
    let worst = problems.first().map(|d| d.severity);
    let at = view.to_screen(b.pos.x as f32, b.pos.y as f32);
    let show_text = z >= 0.5;
    let rows = b.inputs.len().max(b.outputs.len()).max(1);
    let port_size = (fixed::GRAPH_PORT_SIZE * z).max(6.);
    let border = if selected {
        theme.ring
    } else if let Some(color) = run_color {
        color
    } else {
        match worst {
            Some(Severity::Error) => theme.danger,
            _ => theme.border,
        }
    };
    let status = run_color.map(|color| {
        let mut text = node_run.state.label().to_string();
        if let Some(badge) = &node_run.badge {
            text = format!("{badge} · {text}");
        }
        (text, color)
    });
    let icon = crate::extension_panel::named_icon(spec.and_then(|s| s.icon.as_deref()))
        .unwrap_or_else(|| Icon::new(IconName::Box));
    let port_dot = |output: bool, ix: usize, loop_back: bool| {
        let top = (HEADER_HEIGHT + ROW_HEIGHT * (ix as f32 + 0.5)) * z - port_size / 2.;
        let left = if output {
            NODE_WIDTH * z - port_size / 2.
        } else {
            -port_size / 2.
        };
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .size(px(port_size))
            .rounded_full()
            .border_1()
            .border_color(theme.muted_foreground)
            .bg(if loop_back {
                theme.background
            } else {
                theme.muted_foreground
            })
    };
    div()
        .absolute()
        .left(at.x)
        .top(at.y)
        .w(px(NODE_WIDTH * z))
        .h(px(b.height * z))
        .rounded(px(6. * z))
        .border(if selected || run_color.is_some() {
            px(2.)
        } else {
            px(1.)
        })
        .border_color(border)
        .bg(theme.popover)
        .text_color(theme.foreground)
        .when(node.disabled, |n| n.opacity(0.5))
        .child(
            h_flex()
                .h(px(HEADER_HEIGHT * z))
                .px(px(8. * z))
                .gap(px(6. * z))
                .border_b_1()
                .border_color(theme.border)
                .when(show_text, |row| {
                    row.child(icon.with_size(gpui_kit::component::Size::Size(px(14. * z))))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(13. * z))
                                .font_weight(FontWeight::MEDIUM)
                                .child(title),
                        )
                        .when_some(status, |row, (text, color)| {
                            row.child(
                                div()
                                    .flex_none()
                                    .text_size(px(11. * z))
                                    .text_color(color)
                                    .child(text),
                            )
                        })
                        .when_some(worst, |row, severity| {
                            let color = if severity == Severity::Error {
                                theme.danger
                            } else {
                                theme.warning
                            };
                            row.child(
                                Icon::new(IconName::TriangleAlert)
                                    .with_size(gpui_kit::component::Size::Size(px(13. * z)))
                                    .text_color(color),
                            )
                        })
                }),
        )
        .when(show_text, |n| {
            n.children((0..rows).map(|ix| {
                let label = |ports: &Vec<carbon_core::graph::PortSpec>| {
                    ports
                        .get(ix)
                        .map(|p| p.label().to_string())
                        .unwrap_or_default()
                };
                h_flex()
                    .h(px(ROW_HEIGHT * z))
                    .px(px(10. * z))
                    .justify_between()
                    .text_size(px(11. * z))
                    .text_color(theme.muted_foreground)
                    .child(label(&b.inputs))
                    .child(label(&b.outputs))
            }))
            .when_some(b.summary.clone(), |n, summary| {
                n.child(
                    div()
                        .h(px(SUMMARY_HEIGHT * z))
                        .px(px(8. * z))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11. * z))
                        .text_color(theme.muted_foreground)
                        .child(summary),
                )
            })
        })
        .children(
            b.inputs
                .iter()
                .enumerate()
                .map(|(ix, p)| port_dot(false, ix, p.loop_back)),
        )
        .children(
            b.outputs
                .iter()
                .enumerate()
                .map(|(ix, _)| port_dot(true, ix, false)),
        )
        .into_any_element()
}

fn paint(scene: &Scene, bounds: Bounds<Pixels>, window: &mut Window) {
    let origin = bounds.origin;
    let z = scene.view.zoom;
    // Dots every grid step, aligned to graph units.
    let step = GRID * z;
    if step >= 8. {
        let (gx, gy) = scene.view.to_graph(point(px(0.), px(0.)));
        let first_x = (gx / GRID).ceil() * GRID;
        let first_y = (gy / GRID).ceil() * GRID;
        let dot = px(1.5);
        let mut y = first_y;
        while scene.view.to_screen(0., y).y < bounds.size.height {
            let mut x = first_x;
            while scene.view.to_screen(x, 0.).x < bounds.size.width {
                let at = scene.view.to_screen(x, y);
                window.paint_quad(fill(Bounds::new(origin + at, size(dot, dot)), scene.grid));
                x += GRID;
            }
            y += GRID;
        }
    }
    let width = |selected: bool| px(if selected { 2.5 } else { 1.5 } * z.max(0.75));
    for arrow in &scene.arrows {
        let color = if arrow.selected {
            scene.accent
        } else if arrow.problem {
            scene.danger
        } else {
            scene.arrow
        };
        stroke_curve(
            &arrow.curve,
            origin,
            width(arrow.selected),
            arrow.loop_back,
            color,
            window,
        );
        arrow_head(arrow.curve[3], origin, z, color, window);
    }
    if let Some(curve) = &scene.preview {
        stroke_curve(curve, origin, width(true), true, scene.accent, window);
    }
    if let Some(rect) = scene.marquee {
        let rect = Bounds::new(origin + rect.origin, rect.size);
        window.paint_quad(fill(rect, scene.accent.opacity(0.08)));
        window.paint_quad(outline(rect, scene.accent, BorderStyle::Solid));
    }
}

fn stroke_curve(
    curve: &[Point<Pixels>; 4],
    origin: Point<Pixels>,
    width: Pixels,
    dashed: bool,
    color: Hsla,
    window: &mut Window,
) {
    let mut builder = PathBuilder::stroke(width);
    if dashed {
        builder = builder.dash_array(&[px(6.), px(4.)]);
    }
    builder.move_to(origin + curve[0]);
    builder.cubic_bezier_to(origin + curve[3], origin + curve[1], origin + curve[2]);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// A small filled triangle ending at the input port, pointing right.
fn arrow_head(tip: Point<Pixels>, origin: Point<Pixels>, z: f32, color: Hsla, window: &mut Window) {
    let (len, half) = (px(8. * z.max(0.75)), px(4. * z.max(0.75)));
    let tip = origin + tip;
    let mut builder = PathBuilder::fill();
    builder.move_to(tip);
    builder.line_to(point(tip.x - len, tip.y - half));
    builder.line_to(point(tip.x - len, tip.y + half));
    builder.close();
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

// ---- pointer handling (on GraphPanel) ------------------------------------

impl GraphPanel {
    fn local(&self, position: Point<Pixels>) -> Point<Pixels> {
        position - self.bounds.get().origin
    }

    fn boxes(&self, cx: &App) -> Vec<NodeBox> {
        let registry = crate::extensions::registry(cx);
        match (&self.doc, self.kind(&registry)) {
            (Some(doc), Some(kind)) => geometry::boxes(doc, &kind.node_types),
            _ => Vec::new(),
        }
    }

    pub(super) fn press(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.canvas_focus, cx);
        if self.doc.is_none() {
            return;
        }
        let at = self.local(event.position);
        if self.space_down {
            self.drag = Some(Drag::Pan {
                start: at,
                origin: self.view,
            });
            cx.notify();
            return;
        }
        let boxes = self.boxes(cx);
        // An output port: start an arrow.
        if let Some(from) = geometry::port_at(&boxes, self.view, at, true) {
            self.drag = Some(Drag::Connect { from, to: at });
            cx.notify();
            return;
        }
        // A block (the topmost under the pointer).
        let (gx, gy) = self.view.to_graph(at);
        if let Some(hit) = boxes.iter().rev().find(|b| b.contains(gx, gy)) {
            let id = hit.id.clone();
            let mut selection = self.selected.clone();
            if event.modifiers.shift {
                if !selection.remove(&id) {
                    selection.insert(id.clone());
                }
            } else if !selection.contains(&id) {
                selection = BTreeSet::from([id.clone()]);
            }
            self.select_nodes(selection, cx);
            let doc = self.doc.as_ref().expect("checked");
            let origins = self
                .selected
                .iter()
                .map(|id| (id.clone(), doc.pos(id)))
                .collect();
            self.drag = Some(Drag::Move {
                start: at,
                origins,
                moved: false,
            });
            cx.notify();
            return;
        }
        // An arrow.
        if let Some(edge) = self.edge_at(&boxes, at) {
            self.select_nodes(BTreeSet::new(), cx);
            self.selected_edge = Some(edge);
            cx.notify();
            return;
        }
        // Empty canvas: a selection box.
        if !event.modifiers.shift {
            self.select_nodes(BTreeSet::new(), cx);
        }
        self.drag = Some(Drag::Marquee {
            start: at,
            current: at,
            extend: event.modifiers.shift,
        });
        cx.notify();
    }

    pub(super) fn press_middle(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.canvas_focus, cx);
        let at = self.local(event.position);
        self.drag = Some(Drag::Pan {
            start: at,
            origin: self.view,
        });
        cx.notify();
    }

    fn edge_at(&self, boxes: &[NodeBox], at: Point<Pixels>) -> Option<String> {
        let doc = self.doc.as_ref()?;
        let find = |id: &str| boxes.iter().find(|b| b.id == id);
        doc.edges
            .iter()
            .filter_map(|edge| {
                let from = find(&edge.from.node)?.port(true, &edge.from.port)?;
                let to = find(&edge.to.node)?.port(false, &edge.to.port)?;
                let curve = geometry::curve(
                    self.view.to_screen(from.0, from.1),
                    self.view.to_screen(to.0, to.1),
                    self.view.zoom,
                );
                let d = geometry::distance_to_curve(&curve, at);
                (d <= EDGE_HIT).then(|| (d, edge.id.clone()))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    pub(super) fn pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drag.is_none() {
            return;
        }
        // The button came up outside the canvas.
        if event.pressed_button.is_none() {
            self.finish_drag(event.position, window, cx);
            return;
        }
        let at = self.local(event.position);
        let zoom = self.view.zoom;
        match self.drag.as_mut() {
            Some(Drag::Pan { start, origin }) => {
                let d = at - *start;
                self.view = View {
                    x: origin.x - f32::from(d.x) / zoom,
                    y: origin.y - f32::from(d.y) / zoom,
                    zoom: origin.zoom,
                };
            }
            Some(Drag::Move {
                start,
                origins,
                moved,
            }) => {
                let d = at - *start;
                if !*moved && f32::from(d.x).abs() + f32::from(d.y).abs() < 3. {
                    return;
                }
                *moved = true;
                let (dx, dy) = (f32::from(d.x) / zoom, f32::from(d.y) / zoom);
                let free = event.modifiers.alt;
                if let Some(doc) = self.doc.as_mut() {
                    for (id, from) in origins.iter() {
                        let (x, y) = (from.x + dx as f64, from.y + dy as f64);
                        let pos = if free {
                            Pos::new(x.round(), y.round())
                        } else {
                            Pos::new(geometry::snap(x), geometry::snap(y))
                        };
                        doc.layout.nodes.insert(id.clone(), pos);
                    }
                }
            }
            Some(Drag::Connect { to, .. }) => *to = at,
            Some(Drag::Marquee { current, .. }) => *current = at,
            None => {}
        }
        cx.notify();
    }

    pub(super) fn release(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_drag(event.position, window, cx);
    }

    fn finish_drag(&mut self, position: Point<Pixels>, _: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        let at = self.local(position);
        match drag {
            Drag::Move {
                origins,
                moved: true,
                ..
            } => {
                let doc = self.doc.as_ref().expect("dragging a document");
                let moves: Vec<_> = origins
                    .into_iter()
                    .map(|(id, from)| {
                        let to = doc.pos(&id);
                        (id, from, to)
                    })
                    .filter(|(_, from, to)| from != to)
                    .collect();
                if !moves.is_empty() {
                    // Already applied while dragging: record the step only.
                    self.history.record(GraphEdit::MoveNodes(moves));
                    self.after_change(false, cx);
                }
            }
            Drag::Connect { from, .. } => {
                let boxes = self.boxes(cx);
                if let Some(to) = geometry::port_at(&boxes, self.view, at, false) {
                    self.connect(from, to, cx);
                }
            }
            Drag::Marquee { start, extend, .. } => {
                let rect = geometry::rect(start, at);
                let boxes = self.boxes(cx);
                let mut selection = if extend {
                    self.selected.clone()
                } else {
                    BTreeSet::new()
                };
                for b in &boxes {
                    let top_left = self.view.to_screen(b.pos.x as f32, b.pos.y as f32);
                    let size = size(
                        px(NODE_WIDTH * self.view.zoom),
                        px(b.height * self.view.zoom),
                    );
                    if rect.intersects(&Bounds::new(top_left, size)) {
                        selection.insert(b.id.clone());
                    }
                }
                self.select_nodes(selection, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self.local(event.position);
        let zoom_gesture = event.modifiers.control || event.modifiers.platform;
        match event.delta {
            ScrollDelta::Pixels(delta) if !zoom_gesture => {
                // A trackpad: pan.
                self.view.x -= f32::from(delta.x) / self.view.zoom;
                self.view.y -= f32::from(delta.y) / self.view.zoom;
            }
            ScrollDelta::Pixels(delta) => {
                let factor = (1.0f32 + f32::from(delta.y) / 200.).clamp(0.5, 2.);
                self.view = self.view.zoom_at(self.view.zoom * factor, at);
            }
            ScrollDelta::Lines(lines) => {
                let factor = 1.1f32.powf(lines.y);
                self.view = self.view.zoom_at(self.view.zoom * factor, at);
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
}
