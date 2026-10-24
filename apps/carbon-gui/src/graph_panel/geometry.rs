//! Where things are on the graph canvas: node boxes and port points in graph
//! units, the graph-to-screen transform (pan and zoom), and hit tests for
//! ports and arrows. No GPUI drawing here, so it is easy to test.

use carbon_core::graph::{GraphDoc, Node, NodeTypeSpec, PortRef, PortSpec, Pos, Viewport};
use gpui_kit::{Bounds, Pixels, Point, point, px};

/// Node width in graph units.
pub const NODE_WIDTH: f32 = 200.;
/// Title row height.
pub const HEADER_HEIGHT: f32 = 30.;
/// One port row.
pub const ROW_HEIGHT: f32 = 22.;
/// The one-line summary under the ports (a node's first text field).
pub const SUMMARY_HEIGHT: f32 = 20.;
/// Space below the rows.
pub const BOTTOM_PAD: f32 = 6.;
/// Zoom limits.
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 2.;
/// A press this close to a port (screen px) grabs it.
pub const PORT_HIT: f32 = 12.;
/// A press this close to an arrow (screen px) selects it.
pub const EDGE_HIT: f32 = 6.;
/// Grid spacing and snap step, graph units.
pub const GRID: f32 = 20.;
pub const SNAP: f32 = 8.;

/// A node's ports and box, in graph units.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeBox {
    pub id: String,
    pub pos: Pos,
    pub height: f32,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    pub summary: Option<String>,
}

impl NodeBox {
    pub fn new(id: &str, node: &Node, spec: Option<&NodeTypeSpec>, pos: Pos) -> Self {
        let (inputs, outputs) = match spec {
            Some(spec) => (spec.inputs.clone(), spec.outputs_for(node)),
            None => (Vec::new(), Vec::new()),
        };
        let summary = spec.and_then(|spec| summary(node, spec));
        let rows = inputs.len().max(outputs.len()).max(1) as f32;
        let height = HEADER_HEIGHT
            + rows * ROW_HEIGHT
            + if summary.is_some() {
                SUMMARY_HEIGHT
            } else {
                0.
            }
            + BOTTOM_PAD;
        Self {
            id: id.to_string(),
            pos,
            height,
            inputs,
            outputs,
            summary,
        }
    }

    /// Port `ix` of a side: the input side is the left edge, outputs the
    /// right edge, one row each under the title.
    pub fn port_point(&self, output: bool, ix: usize) -> (f32, f32) {
        let x = self.pos.x as f32 + if output { NODE_WIDTH } else { 0. };
        let y = self.pos.y as f32 + HEADER_HEIGHT + ROW_HEIGHT * (ix as f32 + 0.5);
        (x, y)
    }

    pub fn port(&self, output: bool, port: &str) -> Option<(f32, f32)> {
        let ports = if output { &self.outputs } else { &self.inputs };
        let ix = ports.iter().position(|p| p.id == port)?;
        Some(self.port_point(output, ix))
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        let (left, top) = (self.pos.x as f32, self.pos.y as f32);
        x >= left && x <= left + NODE_WIDTH && y >= top && y <= top + self.height
    }
}

/// The first non-empty text field, on one line (what a node does at a glance).
fn summary(node: &Node, spec: &NodeTypeSpec) -> Option<String> {
    spec.fields
        .iter()
        .filter(|field| field.kind == carbon_core::graph::FieldKind::Text)
        .find_map(|field| {
            let text = node.config.get(&field.key)?.as_str()?.trim();
            let line = text.lines().next()?.trim();
            (!line.is_empty()).then(|| line.to_string())
        })
}

/// Every node's box.
pub fn boxes(doc: &GraphDoc, types: &[&NodeTypeSpec]) -> Vec<NodeBox> {
    doc.nodes
        .iter()
        .map(|(id, node)| {
            let spec = types.iter().copied().find(|t| t.id == node.kind);
            NodeBox::new(id, node, spec, doc.pos(id))
        })
        .collect()
}

/// Graph units <-> canvas-local screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl From<Viewport> for View {
    fn from(viewport: Viewport) -> Self {
        Self {
            x: viewport.x as f32,
            y: viewport.y as f32,
            zoom: (viewport.zoom as f32).clamp(MIN_ZOOM, MAX_ZOOM),
        }
    }
}

impl View {
    pub fn viewport(self) -> Viewport {
        Viewport {
            x: round(self.x),
            y: round(self.y),
            zoom: round(self.zoom),
        }
    }

    pub fn to_screen(self, x: f32, y: f32) -> Point<Pixels> {
        point(px((x - self.x) * self.zoom), px((y - self.y) * self.zoom))
    }

    pub fn to_graph(self, at: Point<Pixels>) -> (f32, f32) {
        (
            f32::from(at.x) / self.zoom + self.x,
            f32::from(at.y) / self.zoom + self.y,
        )
    }

    /// Zooms to `zoom`, keeping the graph point under `anchor` (local px) still.
    pub fn zoom_at(self, zoom: f32, anchor: Point<Pixels>) -> Self {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let (gx, gy) = self.to_graph(anchor);
        Self {
            x: gx - f32::from(anchor.x) / zoom,
            y: gy - f32::from(anchor.y) / zoom,
            zoom,
        }
    }

    /// The view that shows every box in `size` (local px), at most 100%.
    pub fn fit(boxes: &[NodeBox], size: (f32, f32)) -> Self {
        let Some((left, top, right, bottom)) = extent(boxes) else {
            return Self {
                x: 0.,
                y: 0.,
                zoom: 1.,
            };
        };
        let margin = 40.;
        let (w, h) = (right - left + margin * 2., bottom - top + margin * 2.);
        let zoom = (size.0 / w).min(size.1 / h).clamp(MIN_ZOOM, 1.);
        Self {
            x: left - margin - (size.0 / zoom - w) / 2.,
            y: top - margin - (size.1 / zoom - h) / 2.,
            zoom,
        }
    }
}

/// Two decimals: enough for a layout, and stable in the file.
fn round(value: f32) -> f64 {
    (value as f64 * 100.).round() / 100.
}

/// Left, top, right, bottom of all boxes.
pub fn extent(boxes: &[NodeBox]) -> Option<(f32, f32, f32, f32)> {
    boxes.iter().fold(None, |acc, b| {
        let (l, t) = (b.pos.x as f32, b.pos.y as f32);
        let (r, bt) = (l + NODE_WIDTH, t + b.height);
        Some(match acc {
            None => (l, t, r, bt),
            Some((a, b2, c, d)) => (a.min(l), b2.min(t), c.max(r), d.max(bt)),
        })
    })
}

/// Snaps a graph coordinate to the snap step.
pub fn snap(value: f64) -> f64 {
    (value / SNAP as f64).round() * SNAP as f64
}

/// The four points of an arrow's curve from an output to an input (screen).
pub fn curve(from: Point<Pixels>, to: Point<Pixels>, zoom: f32) -> [Point<Pixels>; 4] {
    let reach = (f32::from(to.x - from.x).abs() / 2.).max(40. * zoom);
    [
        from,
        point(from.x + px(reach), from.y),
        point(to.x - px(reach), to.y),
        to,
    ]
}

/// A point on the curve at `t` in 0..=1.
fn bezier(p: &[Point<Pixels>; 4], t: f32) -> (f32, f32) {
    let u = 1. - t;
    let (a, b, c, d) = (u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t);
    let x = a * f32::from(p[0].x)
        + b * f32::from(p[1].x)
        + c * f32::from(p[2].x)
        + d * f32::from(p[3].x);
    let y = a * f32::from(p[0].y)
        + b * f32::from(p[1].y)
        + c * f32::from(p[2].y)
        + d * f32::from(p[3].y);
    (x, y)
}

/// Distance from `at` to the curve (sampled as a polyline).
pub fn distance_to_curve(p: &[Point<Pixels>; 4], at: Point<Pixels>) -> f32 {
    const STEPS: usize = 24;
    let (ax, ay) = (f32::from(at.x), f32::from(at.y));
    let mut best = f32::MAX;
    let mut last = bezier(p, 0.);
    for step in 1..=STEPS {
        let next = bezier(p, step as f32 / STEPS as f32);
        best = best.min(segment_distance(last, next, (ax, ay)));
        last = next;
    }
    best
}

fn segment_distance(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx * dx + dy * dy;
    let t = if len == 0. {
        0.
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len).clamp(0., 1.)
    };
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// The port nearest `at` (local px) within `PORT_HIT`, on one side.
pub fn port_at(boxes: &[NodeBox], view: View, at: Point<Pixels>, output: bool) -> Option<PortRef> {
    let mut best: Option<(f32, PortRef)> = None;
    for b in boxes {
        let ports = if output { &b.outputs } else { &b.inputs };
        for (ix, port) in ports.iter().enumerate() {
            let (gx, gy) = b.port_point(output, ix);
            let screen = view.to_screen(gx, gy);
            let d =
                (f32::from(screen.x - at.x).powi(2) + f32::from(screen.y - at.y).powi(2)).sqrt();
            if d <= PORT_HIT && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, PortRef::new(b.id.clone(), port.id.clone())));
            }
        }
    }
    best.map(|(_, port)| port)
}

/// A rectangle from two corners (local px), for the marquee.
pub fn rect(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        point(a.x.min(b.x), a.y.min(b.y)),
        point(a.x.max(b.x), a.y.max(b.y)),
    )
}

#[cfg(test)]
mod tests {
    use super::{NodeBox, View, curve, distance_to_curve, port_at, snap};
    use carbon_core::graph::{Node, NodeTypeSpec, PortRef, PortSpec, Pos};
    use gpui_kit::{point, px};

    fn spec() -> NodeTypeSpec {
        NodeTypeSpec {
            graph: "flow".into(),
            id: "agent".into(),
            title: "Agent".into(),
            icon: None,
            category: None,
            description: String::new(),
            max_count: None,
            inputs: vec![PortSpec::new("in")],
            outputs: vec![PortSpec::new("out"), PortSpec::new("err")],
            outputs_from: None,
            fields: Vec::new(),
        }
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let view = View {
            x: 10.,
            y: 20.,
            zoom: 1.,
        };
        let anchor = point(px(300.), px(200.));
        let before = view.to_graph(anchor);
        let zoomed = view.zoom_at(1.5, anchor);
        let after = zoomed.to_graph(anchor);
        assert!((before.0 - after.0).abs() < 0.01 && (before.1 - after.1).abs() < 0.01);
        assert_eq!(view.zoom_at(10., anchor).zoom, super::MAX_ZOOM);
    }

    #[test]
    fn ports_are_found_near_their_points() {
        let spec = spec();
        let b = NodeBox::new("a", &Node::new("agent"), Some(&spec), Pos::new(100., 50.));
        let view = View {
            x: 0.,
            y: 0.,
            zoom: 1.,
        };
        let (x, y) = b.port_point(true, 1);
        let near = point(px(x + 3.), px(y - 2.));
        assert_eq!(
            port_at(std::slice::from_ref(&b), view, near, true),
            Some(PortRef::new("a", "err"))
        );
        assert_eq!(port_at(&[b], view, near, false), None);
    }

    #[test]
    fn arrows_are_hit_near_their_curve_only() {
        let c = curve(point(px(0.), px(0.)), point(px(200.), px(100.)), 1.);
        assert!(distance_to_curve(&c, point(px(100.), px(50.))) < 2.);
        assert!(distance_to_curve(&c, point(px(100.), px(-40.))) > 20.);
    }

    #[test]
    fn positions_snap() {
        assert_eq!(snap(13.), 16.);
        assert_eq!(snap(-3.), -0.);
    }
}
