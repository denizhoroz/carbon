//! Overlays (`overlay/<id>.toml`): a small patch the user keeps for a built-in
//! workflow. The built-in file is never edited; deleting the overlay resets it.
//!
//! Allowed: `name`, `icon`, `order`, `hidden`, per region `panels` (replace)
//! or `add` / `remove`, `size`, `open`, and new `[panels]` entries. Anything
//! else (`id`, `capabilities`, `requires`, ...) is an error.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{PanelRule, REGIONS, Region, WorkflowDef};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub order: Option<i32>,
    pub hidden: Option<bool>,
    /// Region name (`REGIONS`) -> patch.
    #[serde(default)]
    pub layout: BTreeMap<String, RegionPatch>,
    /// New panel references; existing ones cannot be redefined.
    #[serde(default)]
    pub panels: BTreeMap<String, PanelRule>,
}

/// Changes to one region. `panels` replaces the list; then `remove`, then
/// `add` apply.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionPatch {
    pub panels: Option<Vec<String>>,
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub remove: Vec<String>,
    pub size: Option<f32>,
    pub open: Option<bool>,
}

impl Overlay {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(super::strip_bom(text)).map_err(|err| err.to_string())
    }

    /// The base with this patch applied, plus notices for steps that did
    /// nothing (removing a panel that is not there). Errors: unknown region,
    /// redefined panel. The result still needs `WorkflowDef::validate`.
    pub fn apply(&self, base: &WorkflowDef) -> Result<(WorkflowDef, Vec<String>), Vec<String>> {
        let mut def = base.clone();
        let mut problems = Vec::new();
        let mut notices = Vec::new();
        if let Some(name) = &self.name {
            def.name = name.clone();
        }
        if let Some(icon) = &self.icon {
            def.icon = Some(icon.clone());
        }
        if let Some(order) = self.order {
            def.order = order;
        }
        if let Some(hidden) = self.hidden {
            def.hidden = hidden;
        }
        for (name, rule) in &self.panels {
            if def.panels.contains_key(name) {
                problems.push(format!("panel `{name}` already exists; use a new name"));
            } else {
                def.panels.insert(name.clone(), rule.clone());
            }
        }
        for (region_name, patch) in &self.layout {
            let Some(slot) = def.layout.region_mut(region_name) else {
                problems.push(format!(
                    "layout.{region_name}: not a region ({})",
                    REGIONS.join(", ")
                ));
                continue;
            };
            let region = slot.get_or_insert_with(|| Region {
                panels: Vec::new(),
                size: None,
                open: true,
            });
            if let Some(panels) = &patch.panels {
                region.panels = panels.clone();
            }
            for name in &patch.remove {
                let before = region.panels.len();
                region.panels.retain(|panel| panel != name);
                if region.panels.len() == before {
                    notices.push(format!(
                        "layout.{region_name}: remove `{name}` did nothing (not in the layout)"
                    ));
                }
            }
            for name in &patch.add {
                if !region.panels.contains(name) {
                    region.panels.push(name.clone());
                }
            }
            if patch.size.is_some() {
                region.size = patch.size;
            }
            if let Some(open) = patch.open {
                region.open = open;
            }
        }
        if problems.is_empty() {
            Ok((def, notices))
        } else {
            Err(problems)
        }
    }
}
