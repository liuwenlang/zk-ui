use std::collections::{HashMap, HashSet};

/// Child names admitted into the render list the first time a node is expanded.
/// The catalog still keeps every name returned by ZooKeeper.
pub const PAGE_SIZE: usize = 200;
pub const SEARCH_MAX_RESULTS: usize = 500;

/// Fetched znode data. Expansion and paging live outside this struct.
#[derive(Clone)]
pub struct NodeData {
    pub name: String,
    pub children: Vec<String>,
    pub children_loaded: bool,
}

impl NodeData {
    fn new(name: String) -> Self {
        Self {
            name,
            children: Vec::new(),
            children_loaded: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    Node {
        expandable: bool,
        expanded: bool,
        child_count: Option<usize>,
    },
    Loading,
    More {
        parent: String,
        shown: usize,
        total: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatRow {
    pub path: String,
    pub name: String,
    pub depth: u32,
    pub kind: RowKind,
}

/// ZooKeeper catalog plus the view window that decides which names become rows.
#[derive(Clone)]
pub struct TreeCatalog {
    nodes: HashMap<String, NodeData>,
    expanded: HashSet<String>,
    windows: HashMap<String, usize>,
}

impl Default for TreeCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeCatalog {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            expanded: HashSet::new(),
            windows: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.expanded.clear();
        self.windows.clear();
    }

    pub fn ensure(&mut self, path: &str) {
        self.nodes
            .entry(path.to_string())
            .or_insert_with(|| NodeData::new(node_name(path)));
    }

    pub fn is_loaded(&self, path: &str) -> bool {
        self.nodes
            .get(path)
            .map(|node| node.children_loaded)
            .unwrap_or(false)
    }

    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    pub fn is_expandable(&self, path: &str) -> bool {
        match self.nodes.get(path) {
            Some(node) if node.children_loaded => !node.children.is_empty(),
            _ => true,
        }
    }

    pub fn expand(&mut self, path: &str) {
        self.ensure(path);
        self.expanded.insert(path.to_string());
    }

    pub fn collapse(&mut self, path: &str) {
        self.expanded.remove(path);
    }

    pub fn set_children(&mut self, path: &str, mut children: Vec<String>) {
        children.sort();
        children.dedup();
        self.ensure(path);
        let stale = {
            let node = self.nodes.get_mut(path).expect("node just ensured");
            let fresh: HashSet<&str> = children.iter().map(String::as_str).collect();
            let stale = node
                .children
                .iter()
                .filter(|name| !fresh.contains(name.as_str()))
                .map(|name| child_path(path, name))
                .collect::<Vec<_>>();
            node.children = children;
            node.children_loaded = true;
            stale
        };
        for path in stale {
            self.forget(&path);
        }
    }

    pub fn grow_window(&mut self, path: &str, by: usize) {
        let total = self
            .nodes
            .get(path)
            .map(|node| node.children.len())
            .unwrap_or(0);
        let shown = self.window_of(path);
        self.windows
            .insert(path.to_string(), shown.saturating_add(by).min(total));
    }

    pub fn show_all(&mut self, path: &str) {
        self.windows.insert(path.to_string(), usize::MAX);
    }

    /// Pull the page forward until `child_name` is inside it.
    pub fn ensure_page_includes(&mut self, parent: &str, child_name: &str) {
        let Some(index) = self
            .nodes
            .get(parent)
            .and_then(|node| node.children.iter().position(|name| name == child_name))
        else {
            return;
        };
        let need = index + 1;
        if need > self.window_of(parent) {
            let pages = need.div_ceil(PAGE_SIZE);
            self.windows.insert(parent.to_string(), pages * PAGE_SIZE);
        }
    }

    pub fn forget(&mut self, path: &str) {
        if path == "/" {
            self.clear();
            return;
        }
        let prefix = format!("{path}/");
        self.nodes
            .retain(|key, _| key != path && !key.starts_with(&prefix));
        self.expanded
            .retain(|key| key != path && !key.starts_with(&prefix));
        self.windows
            .retain(|key, _| key != path && !key.starts_with(&prefix));
    }

    pub fn drop_descendants(&mut self, path: &str) {
        if path == "/" {
            let mut root = self
                .nodes
                .remove("/")
                .unwrap_or_else(|| NodeData::new("/".into()));
            root.children.clear();
            root.children_loaded = true;
            self.clear();
            self.nodes.insert("/".into(), root);
            self.expanded.insert("/".into());
            return;
        }
        let prefix = format!("{path}/");
        self.nodes
            .retain(|key, _| key == path || !key.starts_with(&prefix));
        self.expanded
            .retain(|key| key == path || !key.starts_with(&prefix));
        self.windows
            .retain(|key, _| key == path || !key.starts_with(&prefix));
        if let Some(node) = self.nodes.get_mut(path) {
            node.children.clear();
            node.children_loaded = true;
        }
    }

    pub fn remove_child_name(&mut self, parent: &str, name: &str) {
        if let Some(node) = self.nodes.get_mut(parent) {
            node.children.retain(|child| child != name);
        }
    }

    pub fn loaded_names(&self) -> usize {
        self.nodes.values().map(|node| node.children.len()).sum()
    }

    pub fn catalog_matches(&self, query: &str, max: usize) -> Vec<String> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || max == 0 {
            return Vec::new();
        }
        let mut parents: Vec<&str> = self.nodes.keys().map(String::as_str).collect();
        parents.sort_unstable();
        let mut out = Vec::new();
        for parent in parents {
            let Some(node) = self.nodes.get(parent) else {
                continue;
            };
            if !node.children_loaded {
                continue;
            }
            for name in &node.children {
                let path = child_path(parent, name);
                if path.to_lowercase().contains(&query) {
                    out.push(path);
                    if out.len() >= max {
                        return out;
                    }
                }
            }
        }
        out
    }

    pub fn flatten(&self) -> Vec<FlatRow> {
        let mut rows = Vec::new();
        if self.nodes.contains_key("/") {
            self.walk("/", 0, &mut rows);
        }
        rows
    }

    fn window_of(&self, path: &str) -> usize {
        self.windows.get(path).copied().unwrap_or(PAGE_SIZE)
    }

    fn walk(&self, path: &str, depth: u32, out: &mut Vec<FlatRow>) {
        let Some(node) = self.nodes.get(path) else {
            return;
        };
        let expandable = !node.children_loaded || !node.children.is_empty();
        let expanded = expandable && self.expanded.contains(path);
        let child_count = node.children_loaded.then_some(node.children.len());
        out.push(FlatRow {
            path: path.to_string(),
            name: node.name.clone(),
            depth,
            kind: RowKind::Node {
                expandable,
                expanded,
                child_count,
            },
        });
        if !expanded {
            return;
        }
        if !node.children_loaded {
            out.push(FlatRow {
                path: format!("{path}\0loading"),
                name: String::new(),
                depth: depth + 1,
                kind: RowKind::Loading,
            });
            return;
        }
        let total = node.children.len();
        let shown = self.window_of(path).min(total);
        for name in node.children.iter().take(shown) {
            let child = child_path(path, name);
            if self.nodes.contains_key(&child) {
                self.walk(&child, depth + 1, out);
            } else {
                out.push(FlatRow {
                    path: child,
                    name: name.clone(),
                    depth: depth + 1,
                    kind: RowKind::Node {
                        expandable: true,
                        expanded: false,
                        child_count: None,
                    },
                });
            }
        }
        if shown < total {
            out.push(FlatRow {
                path: format!("{path}\0more"),
                name: String::new(),
                depth: depth + 1,
                kind: RowKind::More {
                    parent: path.to_string(),
                    shown,
                    total,
                },
            });
        }
    }
}

pub fn child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

pub fn parent_path(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    match path.rsplit_once('/') {
        Some(("", _)) => "/",
        Some((parent, _)) => parent,
        None => "/",
    }
}

pub fn node_name(path: &str) -> String {
    if path == "/" {
        "/".into()
    } else {
        path.rsplit('/').next().unwrap_or(path).to_string()
    }
}

pub fn row_index(rows: &[FlatRow], path: &str) -> Option<usize> {
    rows.iter()
        .position(|row| matches!(row.kind, RowKind::Node { .. }) && row.path == path)
}

pub fn merge_search(local: &[String], remote: &[String], max: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in local.iter().chain(remote.iter()) {
        if seen.insert(path.clone()) {
            out.push(path.clone());
        }
        if out.len() >= max {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TreeCatalog {
        let mut tree = TreeCatalog::new();
        tree.ensure("/");
        tree.set_children("/", vec!["a".into(), "b".into()]);
        tree.expand("/");
        tree.ensure("/a");
        tree.set_children("/a", vec!["c".into()]);
        tree.expand("/a");
        tree.ensure("/b");
        tree.set_children("/b", vec![]);
        tree
    }

    #[test]
    fn flatten_follows_expansion() {
        let rows = sample().flatten();
        let paths: Vec<_> = rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::Node { .. }))
            .map(|row| row.path.as_str())
            .collect();
        assert_eq!(paths, vec!["/", "/a", "/a/c", "/b"]);
        assert!(matches!(rows[1].kind, RowKind::Node { expanded: true, .. }));
        assert!(matches!(
            rows[3].kind,
            RowKind::Node {
                expandable: false,
                ..
            }
        ));
    }

    #[test]
    fn collapsed_branch_hides_descendants() {
        let mut tree = sample();
        tree.collapse("/a");
        let paths: Vec<_> = tree
            .flatten()
            .into_iter()
            .filter(|row| matches!(row.kind, RowKind::Node { .. }))
            .map(|row| row.path)
            .collect();
        assert_eq!(paths, vec!["/", "/a", "/b"]);
    }

    #[test]
    fn parent_and_child_paths() {
        assert_eq!(child_path("/", "a"), "/a");
        assert_eq!(child_path("/a", "c"), "/a/c");
        assert_eq!(parent_path("/a/c"), "/a");
        assert_eq!(parent_path("/a"), "/");
        assert_eq!(parent_path("/"), "/");
    }

    #[test]
    fn page_window_limits_render_rows() {
        let mut tree = TreeCatalog::new();
        tree.ensure("/");
        let names: Vec<String> = (0..PAGE_SIZE + 5).map(|i| format!("n{i:04}")).collect();
        tree.set_children("/", names);
        tree.expand("/");
        let rows = tree.flatten();
        assert_eq!(rows.len(), 1 + PAGE_SIZE + 1);
        assert!(matches!(
            rows.last().map(|row| &row.kind),
            Some(RowKind::More {
                shown: PAGE_SIZE,
                total,
                ..
            }) if *total == PAGE_SIZE + 5
        ));
        assert_eq!(tree.loaded_names(), PAGE_SIZE + 5);
        assert_eq!(tree.nodes.len(), 1);

        tree.grow_window("/", PAGE_SIZE);
        let rows = tree.flatten();
        assert_eq!(rows.len(), 1 + PAGE_SIZE + 5);
        assert!(rows
            .iter()
            .all(|row| !matches!(row.kind, RowKind::More { .. })));
    }

    #[test]
    fn ensure_page_includes_deep_child() {
        let mut tree = TreeCatalog::new();
        tree.ensure("/");
        let names: Vec<String> = (0..PAGE_SIZE + 10).map(|i| format!("n{i:04}")).collect();
        tree.set_children("/", names);
        tree.windows.insert("/".into(), 2);
        tree.ensure_page_includes("/", "n0009");
        tree.expand("/");
        let paths: Vec<_> = tree.flatten().into_iter().map(|row| row.path).collect();
        assert!(paths.iter().any(|path| path == "/n0009"));
        assert!(paths.iter().any(|path| path == "/n0199"));
        assert!(!paths.iter().any(|path| path == "/n0200"));
    }

    #[test]
    fn set_children_drops_stale_descendants() {
        let mut tree = TreeCatalog::new();
        tree.ensure("/");
        tree.set_children("/", vec!["a".into()]);
        tree.ensure("/a");
        tree.set_children("/a", vec!["c".into()]);
        tree.expand("/a");
        tree.set_children("/", vec!["b".into()]);
        assert!(tree.nodes.get("/a").is_none());
        assert!(tree.nodes.get("/a/c").is_none());
        assert!(!tree.is_expanded("/a"));
        assert_eq!(tree.nodes["/"].children, vec!["b".to_string()]);
    }

    #[test]
    fn unloaded_child_is_a_row_without_a_catalog_entry() {
        let mut tree = TreeCatalog::new();
        tree.ensure("/");
        tree.set_children("/", vec!["a".into()]);
        tree.expand("/");
        let rows = tree.flatten();
        assert_eq!(rows.len(), 2);
        assert!(matches!(
            rows[1].kind,
            RowKind::Node {
                expandable: true,
                expanded: false,
                child_count: None,
            }
        ));
        assert!(tree.nodes.get("/a").is_none());
    }

    #[test]
    fn merge_search_keeps_local_hits_ahead_of_the_cap() {
        let local = vec!["/local".into()];
        let remote = vec!["/local".into(), "/remote".into(), "/extra".into()];
        assert_eq!(
            merge_search(&local, &remote, 2),
            vec!["/local".to_string(), "/remote".to_string()]
        );
    }
}
