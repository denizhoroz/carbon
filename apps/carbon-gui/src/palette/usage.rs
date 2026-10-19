//! How often and how lately each palette command ran, in `command-usage.json`
//! beside the layouts. Ranks commands ("Recently used", tie-breaks).

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

const STORE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/carbon-gui");
const STORE_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/carbon-gui/command-usage.json"
);
const VERSION: u32 = 1;
/// Most commands remembered; the least recently used go first.
const MAX_ENTRIES: usize = 500;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub count: u32,
    /// Unix seconds.
    pub last_used: u64,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    version: u32,
    commands: HashMap<String, Entry>,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            version: VERSION,
            commands: HashMap::new(),
        }
    }
}

impl Global for Usage {}

impl Usage {
    fn load() -> Self {
        let Ok(text) = std::fs::read_to_string(STORE_FILE) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_else(|err| {
            eprintln!("invalid {STORE_FILE}: {err}; starting empty");
            Self::default()
        })
    }

    fn save(&self) {
        let result = serde_json::to_string_pretty(self)
            .map_err(|err| err.to_string())
            .and_then(|json| {
                std::fs::create_dir_all(STORE_DIR)
                    .and_then(|_| std::fs::write(STORE_FILE, json))
                    .map_err(|err| err.to_string())
            });
        if let Err(err) = result {
            eprintln!("failed to save {STORE_FILE}: {err}");
        }
    }

    pub fn count(&self, id: &str) -> u32 {
        self.commands.get(id).map_or(0, |entry| entry.count)
    }

    /// The `n` most recently used ids, newest first.
    pub fn recent(&self, n: usize) -> Vec<&str> {
        let mut entries: Vec<_> = self.commands.iter().collect();
        entries.sort_by(|a, b| b.1.last_used.cmp(&a.1.last_used).then(a.0.cmp(b.0)));
        entries
            .into_iter()
            .take(n)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    fn record_at(&mut self, id: &str, now: u64) {
        let entry = self.commands.entry(id.to_string()).or_default();
        entry.count = entry.count.saturating_add(1);
        entry.last_used = now;
        if self.commands.len() > MAX_ENTRIES {
            let mut by_age: Vec<_> = self
                .commands
                .iter()
                .map(|(id, entry)| (entry.last_used, id.clone()))
                .collect();
            by_age.sort();
            let excess = self.commands.len() - MAX_ENTRIES;
            for (_, id) in by_age.into_iter().take(excess) {
                self.commands.remove(&id);
            }
        }
    }
}

/// Loads the usage file into the app (once, at startup).
pub fn register(cx: &mut App) {
    cx.set_global(Usage::load());
}

/// The usage counts (empty before `register`).
pub fn get(cx: &App) -> &Usage {
    static EMPTY: std::sync::OnceLock<Usage> = std::sync::OnceLock::new();
    cx.try_global::<Usage>()
        .unwrap_or_else(|| EMPTY.get_or_init(Usage::default))
}

/// Notes that command `id` ran, and saves.
pub fn record(id: &str, cx: &mut App) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let usage = cx.default_global::<Usage>();
    usage.record_at(id, now);
    usage.save();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_counts_and_dates() {
        let mut usage = Usage::default();
        usage.record_at("a", 10);
        usage.record_at("a", 20);
        usage.record_at("b", 15);
        assert_eq!(usage.count("a"), 2);
        assert_eq!(usage.count("missing"), 0);
        assert_eq!(usage.recent(5), vec!["a", "b"]);
        assert_eq!(usage.recent(1), vec!["a"]);
    }

    #[test]
    fn prunes_least_recent_beyond_cap() {
        let mut usage = Usage::default();
        for i in 0..=MAX_ENTRIES as u64 {
            usage.record_at(&format!("c{i}"), i);
        }
        assert_eq!(usage.commands.len(), MAX_ENTRIES);
        assert_eq!(usage.count("c0"), 0);
        assert_eq!(usage.count(&format!("c{MAX_ENTRIES}")), 1);
    }

    #[test]
    fn json_round_trip() {
        let mut usage = Usage::default();
        usage.record_at("carbon::Quit|File: Exit", 42);
        let json = serde_json::to_string(&usage).unwrap();
        let back: Usage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, usage);
        let old: Usage = serde_json::from_str("{}").unwrap();
        assert_eq!(old, Usage::default());
    }
}
