//! The workflow registry as the GUI sees it (built-ins only for now; user
//! workflows come in phase 2, `project/not_built/workflow-system.md`), the
//! panel kinds manifests may name, and workflow icons.

use std::sync::OnceLock;

use carbon_core::workflow::{Registry, WorkflowDef};
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{AssetSource, Global};

use crate::app_assets::AppAssets;

/// Panel kinds a manifest may name; `layouts::PanelKind` builds them.
pub const PANEL_KINDS: [&str; 5] = ["editor", "explorer", "terminal", "chat", "placeholder"];

/// Started with `--safe-mode`: saved layouts are neither loaded nor
/// overwritten; every workflow opens with its default layout.
pub struct SafeMode(pub bool);

impl Global for SafeMode {}

/// The loaded workflows. Problems are printed once; a built-in that fails is
/// a bug the core tests catch.
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let registry = Registry::builtin(&PANEL_KINDS);
        for rejected in registry.rejected() {
            eprintln!(
                "workflow {} not loaded: {}",
                rejected.source,
                rejected.problems.join("; ")
            );
        }
        for (source, warning) in registry.warnings() {
            eprintln!("workflow {source}: {warning}");
        }
        registry
    })
}

/// The workflow's Lucide icon if the app embeds it, else a generic one.
pub fn icon(def: &WorkflowDef) -> Icon {
    let path = def.icon.as_ref().map(|name| format!("icons/{name}.svg"));
    match path {
        Some(path) if matches!(AppAssets.load(&path), Ok(Some(_))) => Icon::empty().path(path),
        _ => Icon::new(IconName::LayoutDashboard),
    }
}
