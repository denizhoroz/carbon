//! What extension code starts that outlives a call (extension API 0.2):
//! jobs in the status bar, graph runs, approval questions, agent sessions and
//! their permission requests, timers. Code reports through `HostEvent`s
//! (applied here); the answers go back as `ExtensionEvent`s, delivered to
//! each extension's `on-event` one at a time and in order.
//!
//! Agent sessions run in `carbon_core::acp::AcpManager`; Carbon alone
//! answers their permission requests (a card the user answers; nothing the
//! extension can grant).

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Duration;

use carbon_core::JobId;
use carbon_core::acp::{AcpEvent, AcpManager, AgentProgram, PermissionOption};
use carbon_core::extension::{AgentEntry, ExtensionEvent, ExtensionId, HostEvent};
use carbon_core::graph::RunState;
use gpui_kit::*;

/// A run of a graph file, as its panel shows it.
#[derive(Clone, Debug)]
pub struct RunEntry {
    pub extension: String,
    pub path: PathBuf,
    pub job: Option<u64>,
    pub state: RunState,
}

/// A question an extension asked (an approval block).
#[derive(Clone, Debug)]
pub struct PendingApproval {
    pub extension: String,
    pub id: u64,
    pub title: String,
    pub body: String,
    /// `(id, label, kind)`.
    pub options: Vec<(String, String, String)>,
    pub run: Option<u64>,
    pub node: Option<String>,
}

/// An agent asking before it acts.
#[derive(Clone, Debug)]
pub struct PendingPermission {
    pub session: u64,
    pub request: u64,
    pub title: String,
    pub kind: String,
    pub options: Vec<PermissionOption>,
}

pub struct AgentsHost {
    pub acp: AcpManager,
    /// Extension job ids -> the core's.
    jobs: HashMap<u64, (String, JobId)>,
    pub runs: HashMap<u64, RunEntry>,
    pub approvals: Vec<PendingApproval>,
    pub permissions: Vec<PendingPermission>,
    /// Events waiting for each extension, and who is being called now.
    queues: HashMap<ExtensionId, VecDeque<ExtensionEvent>>,
    busy: BTreeSet<ExtensionId>,
    _events: Task<()>,
}

impl Global for AgentsHost {}

pub fn init(cx: &mut App) {
    let tools = super::dirs(cx).map(|dirs| dirs.tools_dir().to_path_buf());
    let (acp, receiver) = AcpManager::new(tools);
    let context = super::code::context(cx);
    *context.acp.lock().unwrap() = Some(acp.clone());
    *context.storage.lock().unwrap() = super::dirs(cx).map(|dirs| dirs.root().join("storage"));
    let events = cx.spawn(async move |cx| {
        while let Ok(event) = receiver.recv().await {
            cx.update(|cx| on_acp(event, cx));
        }
    });
    cx.set_global(AgentsHost {
        acp,
        jobs: HashMap::new(),
        runs: HashMap::new(),
        approvals: Vec::new(),
        permissions: Vec::new(),
        queues: HashMap::new(),
        busy: BTreeSet::new(),
        _events: events,
    });
    sync_agents(cx);
    apply_settings(cx);
    cx.observe_global::<crate::settings::AppSettings>(apply_settings)
        .detach();
}

/// Extensions changed: the agent table follows; sessions of extensions that
/// are no longer active end.
pub fn sync_agents(cx: &mut App) {
    let registry = super::registry(cx);
    let entries: Vec<AgentEntry> = registry
        .active()
        .flat_map(|loaded| {
            loaded.def.agents.iter().map(move |agent| AgentEntry {
                extension: loaded.def.id.to_string(),
                id: agent.id.clone(),
                program: AgentProgram {
                    name: agent.name.clone(),
                    command: agent.command.clone(),
                    args: agent.args.clone(),
                    env: agent
                        .env
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                    install_hint: agent.install_hint.clone(),
                    npm: agent.npm.clone(),
                },
            })
        })
        .collect();
    *super::code::context(cx).agents.lock().unwrap() = entries;
    if let Some(host) = cx.try_global::<AgentsHost>() {
        let acp = host.acp.clone();
        let owners: BTreeSet<String> = host
            .runs
            .values()
            .map(|run| run.extension.clone())
            .collect();
        for owner in owners {
            if !registry.is_active(&ExtensionId::new(owner.clone())) {
                acp.close_owner(&owner);
            }
        }
    }
}

/// Applies what code reported (all but log lines and notifications).
pub fn apply(event: HostEvent, cx: &mut App) {
    match event {
        HostEvent::Log { .. } | HostEvent::Notify { .. } => {}
        HostEvent::JobStart {
            extension,
            job,
            title,
        } => {
            let core = crate::workspace_ops::current_core(cx);
            let id = core.update(cx, |core, cx| {
                cx.notify();
                core.jobs.start(title)
            });
            cx.global_mut::<AgentsHost>()
                .jobs
                .insert(job, (extension, id));
        }
        HostEvent::JobProgress { job, text } => {
            if let Some((_, id)) = cx.global::<AgentsHost>().jobs.get(&job).cloned() {
                let core = crate::workspace_ops::current_core(cx);
                core.update(cx, |core, cx| {
                    core.jobs.set_title(id, text);
                    cx.notify()
                });
            }
        }
        HostEvent::JobFinish { job, error } => {
            if let Some((_, id)) = cx.global_mut::<AgentsHost>().jobs.remove(&job) {
                let core = crate::workspace_ops::current_core(cx);
                core.update(cx, |core, cx| {
                    core.jobs.finish(id, error.map_or(Ok(()), Err));
                    cx.notify()
                });
            }
        }
        HostEvent::RunBegin {
            extension,
            run,
            path,
            job,
        } => {
            cx.global_mut::<AgentsHost>().runs.insert(
                run,
                RunEntry {
                    extension,
                    path,
                    job,
                    state: RunState::new(run),
                },
            );
            notify_panels(cx);
        }
        HostEvent::NodeState {
            run,
            node,
            state,
            badge,
        } => {
            if let Some(entry) = cx.global_mut::<AgentsHost>().runs.get_mut(&run) {
                entry.state.set_state(&node, state, badge);
            }
            notify_panels(cx);
        }
        HostEvent::NodeOutput {
            run,
            node,
            markdown,
        } => {
            if let Some(entry) = cx.global_mut::<AgentsHost>().runs.get_mut(&run) {
                entry.state.set_output(&node, markdown);
            }
            notify_panels(cx);
        }
        HostEvent::RunLog { run, node, line } => {
            if let Some(entry) = cx.global_mut::<AgentsHost>().runs.get_mut(&run) {
                entry.state.log.push((node, line));
            }
            notify_panels(cx);
        }
        HostEvent::RunEnd { run, error } => {
            let host = cx.global_mut::<AgentsHost>();
            if let Some(entry) = host.runs.get_mut(&run) {
                entry.state.finish(error.map_or(Ok(()), Err));
            }
            host.approvals.retain(|a| a.run != Some(run));
            notify_panels(cx);
        }
        HostEvent::Approval {
            extension,
            id,
            title,
            body,
            options,
            run,
            node,
        } => {
            cx.global_mut::<AgentsHost>()
                .approvals
                .push(PendingApproval {
                    extension,
                    id,
                    title,
                    body,
                    options,
                    run,
                    node,
                });
            notify_panels(cx);
        }
        HostEvent::ApprovalWithdraw { id } => {
            cx.global_mut::<AgentsHost>()
                .approvals
                .retain(|a| a.id != id);
            notify_panels(cx);
        }
        HostEvent::Timer {
            extension,
            tag,
            after_ms,
        } => {
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(after_ms as u64))
                    .await;
                cx.update(|cx| {
                    deliver(
                        ExtensionId::new(extension),
                        ExtensionEvent::TimerFired(tag),
                        cx,
                    )
                });
            })
            .detach();
        }
    }
}

/// Graph panels redraw (run state, cards).
fn notify_panels(cx: &mut App) {
    cx.update_global::<AgentsHost, _>(|_, _| {});
}

/// What a session did, for its owner (and the cards).
fn on_acp(event: AcpEvent, cx: &mut App) {
    let owner = ExtensionId::new(event.owner());
    let session = event.session();
    let streamed = super::code::context(cx)
        .streamed
        .lock()
        .unwrap()
        .contains(&session);
    let forward = match event {
        AcpEvent::Starting { .. } | AcpEvent::Closed { .. } => None,
        AcpEvent::Ready { .. } => Some(ExtensionEvent::SessionReady(session)),
        AcpEvent::Failed { kind, message, .. } => Some(ExtensionEvent::SessionFailed {
            session,
            kind: kind.as_str().into(),
            message,
        }),
        AcpEvent::Text { text, .. } => {
            streamed.then_some(ExtensionEvent::AgentText { session, text })
        }
        AcpEvent::ToolCall { title, status, .. } => Some(ExtensionEvent::ToolCall {
            session,
            title,
            status,
        }),
        AcpEvent::Usage { usage, .. } => Some(ExtensionEvent::UsageUpdate { session, usage }),
        AcpEvent::Permission {
            request,
            title,
            kind,
            options,
            ..
        } => {
            cx.global_mut::<AgentsHost>()
                .permissions
                .push(PendingPermission {
                    session,
                    request,
                    title,
                    kind,
                    options,
                });
            None
        }
        AcpEvent::PromptFinished {
            stop_reason,
            text,
            truncated,
            usage,
            ..
        } => Some(ExtensionEvent::PromptFinished {
            session,
            stop_reason,
            text,
            truncated,
            usage,
        }),
    };
    if let Some(event) = forward {
        deliver(owner, event, cx);
    }
    notify_panels(cx);
}

/// Queues `event` for extension `id`'s code; calls run one at a time.
pub fn deliver(id: ExtensionId, event: ExtensionEvent, cx: &mut App) {
    cx.global_mut::<AgentsHost>()
        .queues
        .entry(id.clone())
        .or_default()
        .push_back(event);
    pump(id, cx);
}

fn pump(id: ExtensionId, cx: &mut App) {
    let host = cx.global_mut::<AgentsHost>();
    if host.busy.contains(&id) {
        return;
    }
    let Some(event) = host.queues.get_mut(&id).and_then(VecDeque::pop_front) else {
        return;
    };
    host.busy.insert(id.clone());
    let task = super::code::on_event(id.clone(), event, cx);
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            if let Err(err) = result {
                eprintln!("[{id}] on-event: {err}");
            }
            cx.global_mut::<AgentsHost>().busy.remove(&id);
            pump(id, cx);
        });
    })
    .detach();
}

/// The user answered an approval question.
pub fn answer_approval(id: u64, option: String, cx: &mut App) {
    let host = cx.global_mut::<AgentsHost>();
    let Some(ix) = host.approvals.iter().position(|a| a.id == id) else {
        return;
    };
    let approval = host.approvals.remove(ix);
    deliver(
        ExtensionId::new(approval.extension),
        ExtensionEvent::ApprovalAnswered { id, option },
        cx,
    );
}

/// The user answered an agent's permission request (`None`: refused).
pub fn answer_permission(session: u64, request: u64, option: Option<String>, cx: &mut App) {
    let host = cx.global_mut::<AgentsHost>();
    host.permissions
        .retain(|p| !(p.session == session && p.request == request));
    host.acp.answer_permission(session, request, option);
    notify_panels(cx);
}

/// Stop on a job: its agent sessions are cancelled now, and its extension
/// is told (it ends the job).
pub fn stop_job(job: u64, cx: &mut App) {
    let context = super::code::context(cx);
    let sessions: Vec<u64> = context
        .session_jobs
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, j)| **j == job)
        .map(|(s, _)| *s)
        .collect();
    let host = cx.global::<AgentsHost>();
    for session in sessions {
        host.acp.cancel(session);
    }
    let Some((extension, _)) = host.jobs.get(&job).cloned() else {
        return;
    };
    deliver(
        ExtensionId::new(extension),
        ExtensionEvent::JobCancelRequested(job),
        cx,
    );
}

/// The newest run of the graph file at `path`.
pub fn latest_run(path: &Path, cx: &App) -> Option<RunEntry> {
    let wanted = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    cx.try_global::<AgentsHost>()?
        .runs
        .values()
        .filter(|run| dunce::canonicalize(&run.path).unwrap_or_else(|_| run.path.clone()) == wanted)
        .max_by_key(|run| run.state.id)
        .cloned()
}

/// The newest agent session working for `node` of `run`.
pub fn session_for(run: u64, node: &str, cx: &App) -> Option<u64> {
    super::code::context(cx)
        .session_nodes
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, (r, n))| *r == run && n == node)
        .map(|(session, _)| *session)
        .max()
}

/// Runs still going.
pub fn running_runs(cx: &App) -> usize {
    cx.try_global::<AgentsHost>()
        .map(|host| host.runs.values().filter(|r| r.state.is_running()).count())
        .unwrap_or_default()
}

/// Stops every job extension code started and closes agent sessions.
pub fn stop_all(cx: &mut App) {
    let jobs: Vec<u64> = cx.global::<AgentsHost>().jobs.keys().copied().collect();
    for job in jobs {
        stop_job(job, cx);
    }
    let host = cx.global::<AgentsHost>();
    let owners: BTreeSet<String> = host.runs.values().map(|r| r.extension.clone()).collect();
    for owner in owners {
        host.acp.close_owner(&owner);
    }
}

/// What waits for the user across all runs: a label, and the graph file
/// of the first question (to open it).
pub struct Waiting {
    pub label: String,
    pub path: Option<PathBuf>,
}

pub fn waiting_summary(cx: &App) -> Option<Waiting> {
    let host = cx.try_global::<AgentsHost>()?;
    let count = host.approvals.len() + host.permissions.len();
    if count == 0 {
        return None;
    }
    let nodes = super::code::context(cx)
        .session_nodes
        .lock()
        .unwrap()
        .clone();
    let run = host.approvals.iter().find_map(|a| a.run).or_else(|| {
        host.permissions
            .iter()
            .find_map(|p| nodes.get(&p.session).map(|(run, _)| *run))
    });
    Some(Waiting {
        label: format!("{count} waiting"),
        path: run
            .and_then(|run| host.runs.get(&run))
            .map(|r| r.path.clone()),
    })
}

/// Carbon's own agent settings.
pub const MAX_PERMISSION_MODE: &str = "agents.max_permission_mode";

pub fn settings() -> Vec<carbon_core::settings::Setting> {
    use carbon_core::settings::{Kind, Setting};
    vec![
        Setting::new(
            MAX_PERMISSION_MODE,
            &["Agents"],
            "Most an Agent May Do Unasked",
            Kind::choice(&[
                ("ask", "Ask for everything"),
                ("allow-edits", "Edit files in the folder"),
                ("allow-all", "Everything"),
            ]),
            "allow-edits",
        )
        .description(
            "Agent flows choose per block what an agent may do without asking you; they never get more than this.",
        )
        .keywords(&["permissions", "approval", "autopilot", "claude", "acp"])
        .user_only(),
    ]
}

/// Applies the setting to the session manager.
fn apply_settings(cx: &mut App) {
    let mode = crate::settings::get(cx)
        .get_str(MAX_PERMISSION_MODE)
        .to_string();
    if let (Some(mode), Some(host)) = (
        carbon_core::acp::PermissionMode::parse(&mode),
        cx.try_global::<AgentsHost>(),
    ) {
        host.acp.set_max_mode(mode);
    }
}

/// The extension job id of the core job `id` (for Stop in the status bar).
pub fn job_for(id: JobId, cx: &App) -> Option<u64> {
    cx.try_global::<AgentsHost>()?
        .jobs
        .iter()
        .find(|(_, (_, core))| *core == id)
        .map(|(job, _)| *job)
}
