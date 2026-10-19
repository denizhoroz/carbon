//! Command ranking: fuzzy score first, usage breaks ties (Zed's order), so a
//! strong match always beats a heavily used weak one.

use carbon_core::fuzzy;

/// What ranking needs of one command.
pub struct Candidate<'a> {
    pub label: &'a str,
    pub keywords: &'a [&'a str],
    pub usage: u32,
}

/// A command that matched, best first.
#[derive(Debug, PartialEq, Eq)]
pub struct Ranked {
    /// Position in the candidate list.
    pub index: usize,
    /// Matched chars of the label; empty when only a keyword matched.
    pub positions: Vec<u32>,
}

/// Keyword matches count for less than label matches of the same quality.
const KEYWORD_WEIGHT: (u32, u32) = (3, 4);

/// Ranks candidates for `query`. An empty query keeps every candidate,
/// most used first, then by label.
pub fn rank(query: &str, candidates: &[Candidate]) -> Vec<Ranked> {
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| {
        let (a, b) = (&candidates[a], &candidates[b]);
        b.usage.cmp(&a.usage).then_with(|| a.label.cmp(b.label))
    });
    if query.is_empty() {
        return order
            .into_iter()
            .map(|index| Ranked {
                index,
                positions: Vec::new(),
            })
            .collect();
    }

    let labels = fuzzy::match_list(query, order.iter().map(|&ix| candidates[ix].label), false);
    let mut scored: Vec<(u32, usize, Vec<u32>)> = Vec::with_capacity(labels.len());
    let mut matched = vec![false; order.len()];
    for found in labels {
        matched[found.index] = true;
        scored.push((found.score, found.index, found.positions));
    }
    for (slot, &ix) in order.iter().enumerate() {
        if matched[slot] || candidates[ix].keywords.is_empty() {
            continue;
        }
        let best = fuzzy::match_list(query, candidates[ix].keywords.iter().copied(), false)
            .into_iter()
            .map(|found| found.score)
            .max();
        if let Some(score) = best {
            scored.push((
                score * KEYWORD_WEIGHT.0 / KEYWORD_WEIGHT.1,
                slot,
                Vec::new(),
            ));
        }
    }
    // Stable on the usage order: equal scores stay most-used first.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .map(|(_, slot, positions)| Ranked {
            index: order[slot],
            positions,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate<'a>(label: &'a str, usage: u32) -> Candidate<'a> {
        Candidate {
            label,
            keywords: &[],
            usage,
        }
    }

    fn labels<'a>(candidates: &[Candidate<'a>], ranked: &[Ranked]) -> Vec<&'a str> {
        ranked.iter().map(|r| candidates[r.index].label).collect()
    }

    #[test]
    fn empty_query_orders_by_usage_then_label() {
        let candidates = [candidate("b", 0), candidate("a", 0), candidate("c", 5)];
        let ranked = rank("", &candidates);
        assert_eq!(labels(&candidates, &ranked), vec!["c", "a", "b"]);
    }

    #[test]
    fn strong_match_beats_used_weak_match() {
        let candidates = [
            candidate("View: Toggle Bottom Dock", 50),
            candidate("File: Save", 0),
        ];
        let ranked = rank("save", &candidates);
        assert_eq!(labels(&candidates, &ranked)[0], "File: Save");
    }

    #[test]
    fn usage_breaks_ties() {
        let candidates = [
            candidate("View: Toggle Left Dock", 0),
            candidate("View: Toggle Right Dock", 3),
        ];
        let ranked = rank("view", &candidates);
        assert_eq!(
            labels(&candidates, &ranked),
            vec!["View: Toggle Right Dock", "View: Toggle Left Dock"]
        );
    }

    #[test]
    fn keywords_find_without_highlight() {
        let candidates = [
            Candidate {
                label: "File: Exit",
                keywords: &["quit"],
                usage: 0,
            },
            candidate("Edit: Undo", 0),
        ];
        let ranked = rank("quit", &candidates);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].index, 0);
        assert!(ranked[0].positions.is_empty());
    }

    #[test]
    fn label_positions_are_returned() {
        let candidates = [candidate("Toggle Left Dock", 0)];
        let ranked = rank("tld", &candidates);
        assert_eq!(ranked[0].positions, vec![0, 7, 12]);
    }
}
