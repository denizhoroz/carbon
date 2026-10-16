//! Type to filter: typing a letter in the tree opens a filter box; rows whose
//! name contains the text stay, with the folders above them. Only loaded
//! (opened) folders are searched, like VS Code's filter on type.

use std::collections::HashSet;
use std::path::PathBuf;

use carbon_core::Row;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;

use super::ExplorerPanel;

pub struct Filter {
    pub input: Entity<InputState>,
    _events: Subscription,
}

impl Filter {
    /// The current text, lower-cased for matching.
    pub fn query(&self, cx: &App) -> String {
        self.input.read(cx).value().trim().to_lowercase()
    }
}

/// Keeps matching rows (name contains `query`, any case) and their folders.
pub fn filter_rows(rows: Vec<Row>, query: &str) -> Vec<Row> {
    let matches: Vec<&Row> = rows
        .iter()
        .filter(|row| row.label.to_lowercase().contains(query))
        .collect();
    let ancestors: HashSet<PathBuf> = matches
        .iter()
        .flat_map(|row| row.entry.path.ancestors().skip(1).map(PathBuf::from))
        .collect();
    let keep: HashSet<PathBuf> = matches.iter().map(|row| row.entry.path.clone()).collect();
    rows.into_iter()
        .filter(|row| keep.contains(&row.entry.path) || ancestors.contains(&row.entry.path))
        .collect()
}

impl ExplorerPanel {
    /// A printable key without Ctrl/Alt/Cmd, pressed in the tree (not in a
    /// name box), opens the filter with that character.
    pub(super) fn type_to_filter(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if self.edit.is_some()
            || self.filter.is_some()
            || modifiers.control
            || modifiers.alt
            || modifiers.platform
            || !self.focus_handle.is_focused(window)
        {
            return;
        }
        let Some(text) = keystroke
            .key_char
            .clone()
            .filter(|text| text.chars().all(|c| !c.is_control()) && !text.trim().is_empty())
        else {
            return;
        };
        cx.stop_propagation();
        self.open_filter(text, window, cx);
    }

    fn open_filter(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Filter")
                .default_value(text)
        });
        let _events = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            match event {
                InputEvent::Change => {
                    this.rebuild_rows(cx);
                    // Keep a selection so Enter / arrows act on a match.
                    if this.selected_ix().is_none() && !this.rows.is_empty() {
                        let first = this.rows.iter().position(|row| {
                            this.filter
                                .as_ref()
                                .is_some_and(|f| row.label.to_lowercase().contains(&f.query(cx)))
                        });
                        this.select(first.unwrap_or(0), cx);
                    }
                    cx.notify();
                }
                // Empty box and focus gone: nothing to keep it open for.
                InputEvent::Blur
                    if this.filter.as_ref().is_some_and(|f| f.query(cx).is_empty()) =>
                {
                    this.close_filter(window, cx)
                }
                _ => {}
            }
        });
        self.filter = Some(Filter {
            input: input.clone(),
            _events,
        });
        self.rebuild_rows(cx);
        cx.notify();
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            let end = input.value().len();
            input.set_selected_range(end..end, cx);
        });
    }

    /// Clears the filter and gives focus back to the tree.
    pub(super) fn close_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter = None;
        self.rebuild_rows(cx);
        if let Some(ix) = self.selected_ix() {
            self.select(ix, cx);
        }
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }
}
