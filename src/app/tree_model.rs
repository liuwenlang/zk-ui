use std::collections::HashMap;

pub const SEARCH_MAX_RESULTS: usize = 300;

#[derive(Clone)]
pub struct TreeNode {
    pub name: String,
    pub expanded: bool,
    pub children_loaded: bool,
    pub children: Vec<String>,
    pub num_children: Option<i32>,
}

impl TreeNode {
    pub fn root() -> Self {
        Self {
            name: "/".into(),
            expanded: true,
            children_loaded: false,
            children: Vec::new(),
            num_children: None,
        }
    }

    pub fn new(name: String) -> Self {
        Self {
            name,
            expanded: false,
            children_loaded: false,
            children: Vec::new(),
            num_children: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatRow {
    pub path: String,
    pub name: String,
    pub depth: u32,
    pub expandable: bool,
    pub expanded: bool,
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

pub fn node_expandable(node: &TreeNode) -> bool {
    if let Some(n) = node.num_children {
        return n > 0;
    }
    if node.children_loaded {
        return !node.children.is_empty();
    }
    true
}

pub fn flatten(nodes: &HashMap<String, TreeNode>) -> Vec<FlatRow> {
    let mut rows = Vec::new();
    if nodes.contains_key("/") {
        walk(nodes, "/", 0, &mut rows);
    }
    rows
}

fn walk(nodes: &HashMap<String, TreeNode>, path: &str, depth: u32, out: &mut Vec<FlatRow>) {
    let Some(node) = nodes.get(path) else {
        return;
    };
    let expandable = node_expandable(node);
    out.push(FlatRow {
        path: path.to_string(),
        name: node.name.clone(),
        depth,
        expandable,
        expanded: node.expanded && expandable,
    });
    if !(node.expanded && expandable) {
        return;
    }
    for child in &node.children {
        walk(nodes, &child_path(path, child), depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> HashMap<String, TreeNode> {
        let mut nodes = HashMap::new();
        let mut root = TreeNode::root();
        root.children_loaded = true;
        root.children = vec!["a".into(), "b".into()];
        root.num_children = Some(2);
        nodes.insert("/".into(), root);

        let mut a = TreeNode::new("a".into());
        a.expanded = true;
        a.children_loaded = true;
        a.children = vec!["c".into()];
        a.num_children = Some(1);
        nodes.insert("/a".into(), a);
        nodes.insert("/a/c".into(), TreeNode::new("c".into()));

        let mut b = TreeNode::new("b".into());
        b.num_children = Some(0);
        nodes.insert("/b".into(), b);
        nodes
    }

    #[test]
    fn flatten_follows_expansion() {
        let rows = flatten(&sample());
        let paths: Vec<_> = rows.iter().map(|row| row.path.as_str()).collect();
        assert_eq!(paths, vec!["/", "/a", "/a/c", "/b"]);
        assert!(rows[1].expanded);
        assert!(!rows[3].expandable);
    }

    #[test]
    fn collapsed_branch_hides_descendants() {
        let mut nodes = sample();
        nodes.get_mut("/a").unwrap().expanded = false;
        let paths: Vec<_> = flatten(&nodes).into_iter().map(|row| row.path).collect();
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
}
