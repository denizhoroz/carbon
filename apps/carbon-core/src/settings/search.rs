//! Settings search: plain words plus `@` filters, as in VS Code.
//!
//! Every word must appear (case-insensitive, anywhere) in the setting's
//! label, description, key, category or keywords. Filters: `@modified` (set
//! in the scope being edited), `@id:<key or key prefix>`, `@ext:<extension>`.

use std::ops::Range;

use super::{Scope, Setting, Settings};

/// A parsed search box text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// Lowercase words, all of which must match.
    pub words: Vec<String>,
    pub modified: bool,
    pub id: Option<String>,
    pub ext: Option<String>,
}

impl Query {
    pub fn parse(text: &str) -> Self {
        let mut query = Query::default();
        for word in text.split_whitespace() {
            let lower = word.to_lowercase();
            if lower == "@modified" {
                query.modified = true;
            } else if let Some(id) = lower.strip_prefix("@id:") {
                query.id = Some(id.to_string());
            } else if let Some(ext) = lower.strip_prefix("@ext:") {
                query.ext = Some(ext.to_string());
            } else {
                query.words.push(lower);
            }
        }
        query
    }

    pub fn is_empty(&self) -> bool {
        *self == Query::default()
    }
}

/// Where the words matched in a setting's visible text (byte ranges), to
/// highlight them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hit {
    pub label: Vec<Range<usize>>,
    pub description: Vec<Range<usize>>,
}

/// The settings `query` keeps, in registry order, with their highlights.
/// `scope` is the scope being edited (for `@modified`).
pub fn search(settings: &Settings, scope: Scope, query: &Query) -> Vec<(usize, Hit)> {
    settings
        .definitions()
        .iter()
        .enumerate()
        .filter_map(|(ix, setting)| {
            if query.modified && settings.value_in(scope, &setting.key).is_none() {
                return None;
            }
            if let Some(id) = &query.id
                && !setting.key.to_lowercase().starts_with(id.as_str())
            {
                return None;
            }
            if let Some(ext) = &query.ext
                && setting.extension.as_deref().map(str::to_lowercase).as_ref() != Some(ext)
            {
                return None;
            }
            matches(setting, &query.words).map(|hit| (ix, hit))
        })
        .collect()
}

fn matches(setting: &Setting, words: &[String]) -> Option<Hit> {
    let mut hit = Hit::default();
    for word in words {
        let label = find_all(&setting.label, word);
        let description = find_all(&setting.description, word);
        let elsewhere = || {
            setting.key.to_lowercase().contains(word.as_str())
                || setting
                    .category
                    .iter()
                    .chain(&setting.keywords)
                    .any(|text| text.to_lowercase().contains(word.as_str()))
        };
        if label.is_empty() && description.is_empty() && !elsewhere() {
            return None;
        }
        hit.label.extend(label);
        hit.description.extend(description);
    }
    hit.label = merge(hit.label);
    hit.description = merge(hit.description);
    Some(hit)
}

/// Byte ranges of every case-insensitive occurrence of `needle` (lowercase).
fn find_all(haystack: &str, needle: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let chars: Vec<(usize, char)> = haystack.char_indices().collect();
    let mut found = Vec::new();
    let mut ix = 0;
    while ix + needle.len() <= chars.len() {
        let same = needle
            .iter()
            .zip(&chars[ix..])
            .all(|(n, (_, c))| c.to_lowercase().eq(n.to_lowercase()));
        if same {
            let start = chars[ix].0;
            let end = chars
                .get(ix + needle.len())
                .map_or(haystack.len(), |(byte, _)| *byte);
            found.push(start..end);
            ix += needle.len();
        } else {
            ix += 1;
        }
    }
    found
}

/// Sorted, with overlapping or touching ranges joined.
fn merge(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::settings::{Kind, Setting};

    fn store() -> (tempfile::TempDir, Settings) {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::new(dir.path().join("settings.json"));
        settings.register(
            Setting::new(
                "editor.font_size",
                &["Editor", "Font"],
                "Font Size",
                Kind::number(6., 72., 1.),
                13,
            )
            .description("Size of code text in pixels."),
        );
        settings.register(
            Setting::new(
                "editor.soft_wrap",
                &["Editor"],
                "Soft Wrap",
                Kind::Bool,
                false,
            )
            .description("Wrap long lines at the editor's width.")
            .keywords(&["word wrap"]),
        );
        (dir, settings)
    }

    #[test]
    fn parses_filters() {
        let query = Query::parse("Font @modified @id:editor. @ext:Foo.Bar");
        assert_eq!(query.words, vec!["font"]);
        assert!(query.modified);
        assert_eq!(query.id.as_deref(), Some("editor."));
        assert_eq!(query.ext.as_deref(), Some("foo.bar"));
        assert!(Query::parse("  ").is_empty());
    }

    #[test]
    fn words_match_anywhere_and_highlight() {
        let (_dir, settings) = store();
        let found = search(&settings, Scope::User, &Query::parse("size"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 0);
        assert_eq!(found[0].1.label, vec![5..9]);
        assert_eq!(found[0].1.description, vec![0..4]);
        // Keyword only: matches, nothing to highlight.
        let found = search(&settings, Scope::User, &Query::parse("word"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, Hit::default());
        // Every word must match.
        assert!(search(&settings, Scope::User, &Query::parse("font wrap")).is_empty());
        // Key and category count.
        assert_eq!(
            search(&settings, Scope::User, &Query::parse("editor")).len(),
            2
        );
        assert_eq!(
            search(&settings, Scope::User, &Query::parse("@id:editor.soft")).len(),
            1
        );
    }

    #[test]
    fn modified_filter() {
        let (_dir, mut settings) = store();
        assert!(search(&settings, Scope::User, &Query::parse("@modified")).is_empty());
        settings
            .set(Scope::User, "editor.soft_wrap", json!(true))
            .unwrap();
        let found = search(&settings, Scope::User, &Query::parse("@modified"));
        assert_eq!(found.iter().map(|(ix, _)| *ix).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn find_all_is_case_insensitive_and_unicode_safe() {
        assert_eq!(find_all("Café CAFÉ", "café"), vec![0..5, 6..11]);
        assert_eq!(merge(vec![4..6, 0..2, 1..3]), vec![0..3, 4..6]);
    }
}
