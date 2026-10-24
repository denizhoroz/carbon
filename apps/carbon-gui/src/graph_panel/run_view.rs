//! The Run tab beside the canvas: the newest run of this graph — its state,
//! questions waiting for you (approvals, agents asking permission), each
//! block's state, and the selected block's output and agent transcript.

use carbon_core::acp::TranscriptItem;
use carbon_core::graph::NodeState;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::GraphPanel;
use crate::extensions::host::{self, AgentsHost, PendingApproval, PendingPermission};
use crate::style::fixed;

/// The colour of a node state (with its word, never alone).
pub(super) fn state_color(state: NodeState, cx: &App) -> Option<Hsla> {
    let theme = cx.theme();
    match state {
        NodeState::Idle => None,
        NodeState::Queued => Some(theme.muted_foreground),
        NodeState::Running => Some(theme.ring),
        NodeState::Waiting => Some(theme.warning),
        NodeState::Done => Some(theme.success),
        NodeState::Failed => Some(theme.danger),
        NodeState::Skipped | NodeState::Cancelled => Some(theme.muted_foreground),
    }
}

fn card(cx: &App) -> Div {
    v_flex()
        .gap_2()
        .p_2()
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().warning)
        .bg(cx.theme().warning.opacity(0.08))
}

fn approval_card(approval: &PendingApproval, cx: &mut Context<GraphPanel>) -> AnyElement {
    let id = approval.id;
    card(cx)
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(approval.title.clone()),
        )
        .when_some(approval.node.clone(), |card, node| {
            card.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Block: {node}")),
            )
        })
        .child(
            div()
                .text_sm()
                .whitespace_normal()
                .child(approval.body.clone()),
        )
        .child(
            h_flex()
                .gap_1()
                .flex_wrap()
                .children(approval.options.iter().map(|(option, label, kind)| {
                    let option = option.clone();
                    let button = Button::new(SharedString::from(format!("approval-{id}-{option}")))
                        .label(label.clone())
                        .small();
                    let button = match kind.as_str() {
                        "allow" => button.primary(),
                        "reject" => button.danger(),
                        _ => button,
                    };
                    button.on_click(move |_, _, cx| host::answer_approval(id, option.clone(), cx))
                })),
        )
        .into_any_element()
}

fn permission_card(permission: &PendingPermission, cx: &mut Context<GraphPanel>) -> AnyElement {
    let (session, request) = (permission.session, permission.request);
    card(cx)
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("An agent asks to"),
        )
        .child(
            div()
                .text_sm()
                .child(format!("{} ({})", permission.title, permission.kind)),
        )
        .child(
            h_flex()
                .gap_1()
                .flex_wrap()
                .children(permission.options.iter().map(|option| {
                    let id = option.id.clone();
                    let button = Button::new(SharedString::from(format!(
                        "permission-{session}-{request}-{id}"
                    )))
                    .label(option.name.clone())
                    .small();
                    let button = if option.kind.starts_with("allow") {
                        button.primary()
                    } else {
                        button.danger()
                    };
                    button.on_click(move |_, _, cx| {
                        host::answer_permission(session, request, Some(id.clone()), cx)
                    })
                }))
                .child(
                    Button::new(SharedString::from(format!(
                        "permission-{session}-{request}-no"
                    )))
                    .label("Refuse")
                    .small()
                    .ghost()
                    .on_click(move |_, _, cx| host::answer_permission(session, request, None, cx)),
                ),
        )
        .into_any_element()
}

/// The questions waiting for the user in run `run` (or for no run at all).
pub(super) fn waiting(
    run: Option<u64>,
    cx: &App,
) -> (Vec<PendingApproval>, Vec<PendingPermission>) {
    let Some(host) = cx.try_global::<AgentsHost>() else {
        return (Vec::new(), Vec::new());
    };
    let context = crate::extensions::host_context(cx);
    let nodes = context.session_nodes.lock().unwrap().clone();
    let approvals = host
        .approvals
        .iter()
        .filter(|a| a.run == run)
        .cloned()
        .collect();
    let permissions = host
        .permissions
        .iter()
        .filter(|p| nodes.get(&p.session).map(|(r, _)| *r) == run)
        .cloned()
        .collect();
    (approvals, permissions)
}

pub(super) fn render_run(panel: &GraphPanel, cx: &mut Context<GraphPanel>) -> Vec<AnyElement> {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let Some(run) = panel.run.clone() else {
        return vec![
            div()
                .text_sm()
                .text_color(muted)
                .child("Not run yet. Run starts it from the toolbar.")
                .into_any_element(),
        ];
    };
    let state = &run.state;
    let (status, color) = match &state.outcome {
        None => ("Running".to_string(), theme.ring),
        Some(Ok(())) => ("Finished".to_string(), theme.success),
        Some(Err(err)) => (format!("Failed: {err}"), theme.danger),
    };
    let stop = run.job.filter(|_| state.is_running());
    let mut out = vec![
        h_flex()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Run {}", state.id)),
            )
            .child(div().flex_1().text_sm().text_color(color).child(status))
            .when_some(stop, |row, job| {
                row.child(
                    Button::new("run-stop")
                        .label("Stop")
                        .small()
                        .danger()
                        .on_click(move |_, _, cx| host::stop_job(job, cx)),
                )
            })
            .into_any_element(),
    ];
    let (approvals, permissions) = waiting(Some(state.id), cx);
    out.extend(approvals.iter().map(|a| approval_card(a, cx)));
    out.extend(permissions.iter().map(|p| permission_card(p, cx)));

    // Each block's state.
    if let Some(doc) = &panel.doc {
        let mut rows = v_flex().gap_1();
        for (id, node) in &doc.nodes {
            let node_run = state.nodes.get(id);
            let node_state = node_run.map(|n| n.state).unwrap_or_default();
            if node_state == NodeState::Idle {
                continue;
            }
            let title = node.title.clone().unwrap_or_else(|| id.clone());
            let color = state_color(node_state, cx).unwrap_or(muted);
            let selected = panel.selected.contains(id);
            let id = id.clone();
            rows = rows.child(
                h_flex()
                    .id(SharedString::from(format!("run-node-{id}")))
                    .gap_2()
                    .px_1()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(cx.theme().list_active))
                    .hover(|row| row.bg(cx.theme().list_hover))
                    .child(div().flex_1().text_sm().child(title))
                    .when_some(node_run.and_then(|n| n.badge.clone()), |row, badge| {
                        row.child(div().text_xs().text_color(muted).child(badge))
                    })
                    .child(div().text_xs().text_color(color).child(node_state.label()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_nodes([id.clone()].into(), cx)
                    })),
            );
        }
        out.push(rows.into_any_element());
    }

    // The selected block: output and the agent's transcript.
    if let [id] = panel.selected.iter().collect::<Vec<_>>()[..] {
        if let Some(output) = state.nodes.get(id).and_then(|n| n.output.clone()) {
            out.push(section("Output", cx));
            out.push(
                div()
                    .text_sm()
                    .whitespace_normal()
                    .child(output)
                    .into_any_element(),
            );
        }
        if let Some(session) = host::session_for(state.id, id, cx) {
            let transcript = cx.global::<AgentsHost>().acp.transcript(session);
            out.push(section("Agent", cx));
            out.extend(
                transcript
                    .iter()
                    .rev()
                    .take(60)
                    .rev()
                    .map(|item| transcript_item(item, cx)),
            );
        }
    }

    if !state.log.is_empty() {
        out.push(section("Log", cx));
        out.extend(state.log.iter().rev().take(40).rev().map(|(node, line)| {
            let text = match node {
                Some(node) => format!("{node}: {line}"),
                None => line.clone(),
            };
            div()
                .text_xs()
                .text_color(muted)
                .whitespace_normal()
                .child(text)
                .into_any_element()
        }));
    }
    out
}

fn section(title: &'static str, cx: &App) -> AnyElement {
    div()
        .pt(fixed::GRAPH_INSPECTOR_GAP)
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.theme().foreground)
        .child(title)
        .into_any_element()
}

fn transcript_item(item: &TranscriptItem, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    match item {
        TranscriptItem::Prompt(text) => div()
            .text_xs()
            .p_1()
            .rounded(px(4.))
            .bg(theme.muted)
            .whitespace_normal()
            .child(text.clone())
            .into_any_element(),
        TranscriptItem::Agent(text) => div()
            .text_sm()
            .whitespace_normal()
            .child(text.clone())
            .into_any_element(),
        TranscriptItem::Tool { title, status, .. } => div()
            .text_xs()
            .text_color(muted)
            .child(format!("Tool: {title} — {status}"))
            .into_any_element(),
        TranscriptItem::Permission { title, answer } => div()
            .text_xs()
            .text_color(theme.warning)
            .child(format!("Asked: {title} — {answer}"))
            .into_any_element(),
        TranscriptItem::Note(text) => div()
            .text_xs()
            .text_color(muted)
            .italic()
            .child(text.clone())
            .into_any_element(),
    }
}
