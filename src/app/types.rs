use std::collections::HashMap;
use std::sync::mpsc;

use crate::zk::{AclEntry, NodeStat, ZkResponse};

// ──────────────────────────────────────────
// i18n
// ──────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum Lang { En, Zh }

impl Lang {
    pub fn toggle(self) -> Self {
        match self { Lang::En => Lang::Zh, Lang::Zh => Lang::En }
    }
    pub fn label(self) -> &'static str {
        match self { Lang::En => "EN/中", Lang::Zh => "中/EN" }
    }
}

// ──────────────────────────────────────────
// Constants
// ──────────────────────────────────────────

pub const CHILDREN_BATCH_SIZE: usize = 25;
pub const SEARCH_MAX_RESULTS: usize = 300;

// ──────────────────────────────────────────
// Data types
// ──────────────────────────────────────────

#[derive(Clone)]
pub struct TreeNode {
    pub name: String,
    #[allow(dead_code)]
    pub path: String,
    pub expanded: bool,
    pub children_loaded: bool,
    pub children: Vec<String>,
    pub shown_count: usize,
    /// From ZK stat; used for correct folder/document icon before expand.
    pub num_children: Option<i32>,
}

impl TreeNode {
    pub fn root() -> Self {
        Self {
            name: "/".into(),
            path: "/".into(),
            expanded: true,
            children_loaded: false,
            children: vec![],
            shown_count: CHILDREN_BATCH_SIZE,
            num_children: None,
        }
    }

    pub fn new(name: String, parent_path: &str) -> Self {
        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };
        Self {
            name,
            path,
            expanded: false,
            children_loaded: false,
            children: vec![],
            shown_count: CHILDREN_BATCH_SIZE,
            num_children: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ZnodeIconKind {
    Root,
    ZkFolder,
    Document,
}

#[derive(Clone)]
pub struct NodeDetail {
    pub path: String,
    pub data: String,
    pub data_raw: Vec<u8>,
    pub stat: NodeStat,
    pub acl: Vec<AclEntry>,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Tab {
    Data,
    Acl,
    Stat,
}

pub enum ConnectState {
    Disconnected,
    Connecting,
    Connected { #[allow(dead_code)] host: String, conn_id: i64 },
}

pub struct Pending {
    pub connect: Option<mpsc::Receiver<ZkResponse>>,
    pub children: HashMap<String, mpsc::Receiver<ZkResponse>>,
    pub data: HashMap<String, mpsc::Receiver<ZkResponse>>,
    pub acl: HashMap<String, mpsc::Receiver<ZkResponse>>,
    pub action: Option<mpsc::Receiver<ZkResponse>>,
    pub search: Option<(mpsc::Receiver<ZkResponse>, u64)>,
    pub stat: HashMap<String, mpsc::Receiver<ZkResponse>>,
}

impl Default for Pending {
    fn default() -> Self {
        Self {
            connect: None,
            children: HashMap::new(),
            data: HashMap::new(),
            acl: HashMap::new(),
            action: None,
            search: None,
            stat: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DragItemKind {
    Folder,
    Connection,
}

#[derive(Clone, Copy)]
pub struct DragItem {
    pub id: i64,
    pub kind: DragItemKind,
    #[allow(dead_code)]
    pub source_folder_id: Option<i64>,
}

pub fn format_timestamp(ts: i64) -> String {
    if ts == 0 {
        return "N/A".into();
    }
    let dt = chrono::DateTime::from_timestamp_millis(ts);
    match dt {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => format!("{}", ts),
    }
}
