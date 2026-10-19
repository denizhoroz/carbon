//! Fuzzy matching for pickers (command palette, Quick Open): fzf scoring
//! through `nucleo-matcher`, the matcher Helix and Zed use.

use std::ops::Range;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// One haystack that matched a query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Position of the haystack in the input.
    pub index: usize,
    pub score: u32,
    /// Matched characters (char indices, sorted, unique).
    pub positions: Vec<u32>,
}

/// Matches `query` against every haystack, keeping input order (callers sort).
/// Smart case: an uppercase letter in the query makes the match case-sensitive.
/// Whitespace splits the query into words that must all match. `paths` tunes
/// the scoring for file paths (bonus after `/`). An empty query matches
/// everything with score 0.
pub fn match_list<'a>(
    query: &str,
    haystacks: impl IntoIterator<Item = &'a str>,
    paths: bool,
) -> Vec<FuzzyMatch> {
    let config = if paths {
        Config::DEFAULT.match_paths()
    } else {
        Config::DEFAULT
    };
    let mut matcher = Matcher::new(config);
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();
    let mut positions = Vec::new();
    haystacks
        .into_iter()
        .enumerate()
        .filter_map(|(index, haystack)| {
            positions.clear();
            let score = pattern.indices(
                Utf32Str::new(haystack, &mut buf),
                &mut matcher,
                &mut positions,
            )?;
            positions.sort_unstable();
            positions.dedup();
            Some(FuzzyMatch {
                index,
                score,
                positions: positions.clone(),
            })
        })
        .collect()
}

/// Turns char `positions` in `text` into merged byte ranges (what text
/// highlighting takes). Positions past the end are ignored.
pub fn byte_ranges(text: &str, positions: &[u32]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut wanted = positions.iter().copied().peekable();
    for (char_ix, (byte_ix, ch)) in text.char_indices().enumerate() {
        let Some(&next) = wanted.peek() else {
            break;
        };
        if next as usize != char_ix {
            continue;
        }
        wanted.next();
        let end = byte_ix + ch.len_utf8();
        match ranges.last_mut() {
            Some(last) if last.end == byte_ix => last.end = end,
            _ => ranges.push(byte_ix..end),
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_match_words() {
        let matches = match_list("tld", ["Toggle Left Dock", "Close Folder"], false);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].index, 0);
        assert_eq!(matches[0].positions, vec![0, 7, 12]);
    }

    #[test]
    fn smart_case() {
        let haystacks = ["save file", "Save File"];
        assert_eq!(match_list("save", haystacks, false).len(), 2);
        let upper = match_list("Save", haystacks, false);
        assert_eq!(upper.len(), 1);
        assert_eq!(upper[0].index, 1);
    }

    #[test]
    fn empty_query_matches_all_in_order() {
        let matches = match_list("", ["b", "a", "c"], false);
        let order: Vec<usize> = matches.iter().map(|m| m.index).collect();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn words_must_all_match() {
        let matches = match_list(
            "dock left",
            ["Toggle Left Dock", "Toggle Right Dock"],
            false,
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].index, 0);
    }

    #[test]
    fn path_mode_prefers_file_name() {
        let haystacks = ["domain/x.rs", "src/main.rs"];
        let matches = match_list("main", haystacks, true);
        let best = matches.iter().max_by_key(|m| m.score).unwrap();
        assert_eq!(best.index, 1);
    }

    #[test]
    fn byte_ranges_merge_and_handle_multibyte() {
        assert_eq!(byte_ranges("abcd", &[0, 1, 3]), vec![0..2, 3..4]);
        // 'é' is two bytes: chars c=0 a=1 f=2 é=3 x=4.
        assert_eq!(byte_ranges("caféx", &[0, 3, 4]), vec![0..1, 3..6]);
        assert_eq!(byte_ranges("ab", &[5]), Vec::<Range<usize>>::new());
    }
}
