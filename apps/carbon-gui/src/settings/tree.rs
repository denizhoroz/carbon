//! The Settings dialog's outline, built from the registry and a search
//! result: the category tree on the left and the one long list on the right
//! (a header per category that holds settings, then its rows), in the same
//! order, so a tree node maps to a list position and back.

use carbon_core::settings::Setting;
use carbon_core::settings::search::Hit;

/// One category of the tree.
pub struct Node {
    pub name: String,
    /// Its full path, outermost first.
    pub path: Vec<String>,
    pub depth: usize,
    pub parent: Option<usize>,
    pub has_children: bool,
    /// Settings in it and below it (the count shown while searching).
    pub count: usize,
    /// The list item its part of the list starts at.
    pub first_item: usize,
}

/// One entry of the list.
pub enum Item {
    /// Category title (`Editor › Font`); index into `Outline::nodes`.
    Header(usize),
    /// A setting: index into the registry, and where the search matched.
    Row(usize, Hit),
}

pub struct Outline {
    /// Categories in tree order (depth first).
    pub nodes: Vec<Node>,
    pub items: Vec<Item>,
    /// The node each item belongs to.
    pub item_node: Vec<usize>,
}

#[derive(Default)]
struct Branch {
    name: String,
    children: Vec<Branch>,
    settings: Vec<(usize, Hit)>,
}

impl Branch {
    fn insert(&mut self, path: &[String], setting: (usize, Hit)) {
        let Some((first, rest)) = path.split_first() else {
            self.settings.push(setting);
            return;
        };
        let ix = match self.children.iter().position(|child| &child.name == first) {
            Some(ix) => ix,
            None => {
                self.children.push(Branch {
                    name: first.clone(),
                    ..Default::default()
                });
                self.children.len() - 1
            }
        };
        self.children[ix].insert(rest, setting);
    }

    fn count(&self) -> usize {
        self.settings.len() + self.children.iter().map(Branch::count).sum::<usize>()
    }
}

/// Builds the outline from the settings `found` (registry indices, in
/// registry order). Categories come in the order their first setting was
/// registered; empty ones are left out.
pub fn build(definitions: &[Setting], found: Vec<(usize, Hit)>) -> Outline {
    let mut root = Branch::default();
    for (ix, hit) in found {
        // A setting with no category sits under "Other".
        let path = match definitions[ix].category.as_slice() {
            [] => vec!["Other".to_string()],
            path => path.to_vec(),
        };
        root.insert(&path, (ix, hit));
    }
    let mut outline = Outline {
        nodes: Vec::new(),
        items: Vec::new(),
        item_node: Vec::new(),
    };
    for branch in root.children {
        flatten(branch, &[], None, &mut outline);
    }
    outline
}

fn flatten(branch: Branch, parent_path: &[String], parent: Option<usize>, outline: &mut Outline) {
    let node = outline.nodes.len();
    let mut path = parent_path.to_vec();
    path.push(branch.name.clone());
    outline.nodes.push(Node {
        name: branch.name.clone(),
        path: path.clone(),
        depth: parent_path.len(),
        parent,
        has_children: !branch.children.is_empty(),
        count: branch.count(),
        first_item: outline.items.len(),
    });
    if !branch.settings.is_empty() {
        outline.items.push(Item::Header(node));
        outline.item_node.push(node);
        for (ix, hit) in branch.settings {
            outline.items.push(Item::Row(ix, hit));
            outline.item_node.push(node);
        }
    }
    for child in branch.children {
        flatten(child, &path, Some(node), outline);
    }
}

impl Outline {
    /// Number of settings rows.
    pub fn row_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, Item::Row(..)))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use carbon_core::settings::{Kind, Setting};

    use super::*;

    fn defs() -> Vec<Setting> {
        vec![
            Setting::new("a.mode", &["Appearance", "Theme"], "Mode", Kind::Bool, true),
            Setting::new("e.size", &["Editor", "Font"], "Size", Kind::Bool, true),
            Setting::new("e.wrap", &["Editor"], "Wrap", Kind::Bool, true),
            Setting::new("a.size", &["Appearance", "Text"], "Size", Kind::Bool, true),
            Setting::new("x.delete", &["Explorer"], "Delete", Kind::Bool, true),
        ]
    }

    fn all(defs: &[Setting]) -> Vec<(usize, Hit)> {
        (0..defs.len()).map(|ix| (ix, Hit::default())).collect()
    }

    #[test]
    fn tree_and_list_in_the_same_order() {
        let defs = defs();
        let outline = build(&defs, all(&defs));
        let names: Vec<_> = outline
            .nodes
            .iter()
            .map(|node| (node.name.as_str(), node.depth, node.count))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Appearance", 0, 2),
                ("Theme", 1, 1),
                ("Text", 1, 1),
                ("Editor", 0, 2),
                ("Font", 1, 1),
                ("Explorer", 0, 1),
            ]
        );
        // Appearance holds no settings itself: no header for it; it starts
        // where Theme does.
        assert_eq!(outline.nodes[0].first_item, 0);
        assert!(matches!(outline.items[0], Item::Header(1)));
        // Editor's own setting comes before its Font child.
        let editor = 3;
        let first = outline.nodes[editor].first_item;
        assert!(matches!(outline.items[first], Item::Header(3)));
        assert!(matches!(outline.items[first + 1], Item::Row(2, _)));
        assert_eq!(outline.items.len(), outline.item_node.len());
        assert_eq!(outline.row_count(), 5);
        assert_eq!(outline.nodes[4].parent, Some(editor));
    }

    #[test]
    fn empty_categories_are_left_out() {
        let defs = defs();
        let outline = build(&defs, vec![(4, Hit::default())]);
        assert_eq!(outline.nodes.len(), 1);
        assert_eq!(outline.nodes[0].name, "Explorer");
        assert_eq!(outline.items.len(), 2);
    }
}
