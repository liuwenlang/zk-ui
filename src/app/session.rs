use std::collections::HashMap;
use std::sync::mpsc;

use crate::db::{ConnProfile, Folder};
use crate::zk::{AclEntry, CreateMode, NodeStat, ZkCmd, ZkResponse};

use super::tree_model::{self, TreeNode, SEARCH_MAX_RESULTS};
use super::ZkApp;

pub struct Pending {
    pub connect: Option<mpsc::Receiver<ZkResponse>>,
    pub auth: Option<mpsc::Receiver<ZkResponse>>,
    pub children: HashMap<String, mpsc::Receiver<ZkResponse>>,
    pub data: HashMap<String, mpsc::Receiver<ZkResponse>>,
    pub acl: HashMap<String, mpsc::Receiver<ZkResponse>>,
    pub action: Option<mpsc::Receiver<ZkResponse>>,
    pub search: Option<(mpsc::Receiver<ZkResponse>, u64)>,
    pub stat: HashMap<String, mpsc::Receiver<ZkResponse>>,
    pub four_letter: Option<mpsc::Receiver<ZkResponse>>,
}

impl Default for Pending {
    fn default() -> Self {
        Self {
            connect: None,
            auth: None,
            children: HashMap::new(),
            data: HashMap::new(),
            acl: HashMap::new(),
            action: None,
            search: None,
            stat: HashMap::new(),
            four_letter: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Data,
    Acl,
    Stat,
    Server,
}

impl ZkApp {
    pub(crate) fn reload_catalog(&mut self) {
        self.root_folders = self.db.get_subfolders(None).unwrap_or_default();
        self.root_connections = self.db.get_connections_in_folder(None).unwrap_or_default();
        self.folder_children.clear();
        let mut stack: Vec<Folder> = self.root_folders.clone();
        while let Some(folder) = stack.pop() {
            let subs = self.db.get_subfolders(Some(folder.id)).unwrap_or_default();
            let conns = self
                .db
                .get_connections_in_folder(Some(folder.id))
                .unwrap_or_default();
            stack.extend(subs.iter().cloned());
            self.folder_children.insert(folder.id, (subs, conns));
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.pending.connect.is_some()
            || self.pending.auth.is_some()
            || self.pending.action.is_some()
            || self.pending.search.is_some()
            || self.pending.four_letter.is_some()
            || !self.pending.children.is_empty()
            || !self.pending.data.is_empty()
            || !self.pending.acl.is_empty()
            || !self.pending.stat.is_empty()
    }

    pub(crate) fn note(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }

    pub(crate) fn fail(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status_message = message.clone();
        self.toast = Some(message);
    }

    pub(crate) fn connect_profile(&mut self, conn: &ConnProfile) {
        self.hosts = conn.hosts.clone();
        self.timeout_ms = conn.timeout_ms;
        self.active_conn_name = conn.name.clone();
        self.pending_auth = if conn.auth_scheme.is_empty() {
            None
        } else {
            Some((conn.auth_scheme.clone(), conn.auth_credential.clone()))
        };
        self.begin_connect(Some(conn.id));
        let _ = self.db.touch_connection(conn.id);
    }

    pub(crate) fn connect_quick(&mut self, hosts: String) {
        self.hosts = hosts;
        self.timeout_ms = 5000;
        self.active_conn_name = self.hosts.clone();
        self.pending_auth = None;
        self.begin_connect(None);
    }

    fn begin_connect(&mut self, conn_id: Option<i64>) {
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::Connect {
            hosts: self.hosts.clone(),
            timeout_ms: self.timeout_ms,
            resp: tx,
        });
        self.connect_state = ConnectState::Connecting;
        self.active_conn_id = conn_id;
        self.note(format!("Connecting to {}…", self.hosts));
        self.pending.connect = Some(rx);
    }

    pub(crate) fn disconnect(&mut self) {
        self.zk.send(ZkCmd::Disconnect);
        self.connect_state = ConnectState::Disconnected;
        self.active_conn_id = None;
        self.detail = None;
        self.selected_path = None;
        self.tree_nodes.clear();
        self.rows.clear();
        self.search_results.clear();
        self.search_in_progress = false;
        self.pending = Pending::default();
        self.note("Disconnected");
    }

    fn finish_connected(&mut self) {
        self.connect_state = ConnectState::Connected;
        self.note(format!("Connected to {}", self.hosts));
        self.tree_nodes.clear();
        self.tree_nodes.insert("/".into(), TreeNode::root());
        self.load_children("/");
        self.select_node("/");
    }

    pub(crate) fn load_children(&mut self, path: &str) {
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::GetChildren {
            path: path.to_string(),
            resp: tx,
        });
        self.pending.children.insert(path.to_string(), rx);
    }

    pub(crate) fn load_node_stat(&mut self, path: &str) {
        if self.pending.stat.contains_key(path) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::Exists {
            path: path.to_string(),
            resp: tx,
        });
        self.pending.stat.insert(path.to_string(), rx);
    }

    pub(crate) fn load_node_detail(&mut self, path: &str) {
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::GetData {
            path: path.to_string(),
            resp: tx,
        });
        self.pending.data.insert(path.to_string(), rx);

        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::GetAcl {
            path: path.to_string(),
            resp: tx,
        });
        self.pending.acl.insert(path.to_string(), rx);
    }

    pub(crate) fn select_node(&mut self, path: &str) {
        self.selected_path = Some(path.to_string());
        self.detail = Some(NodeDetail::empty(path));
        self.data_dirty = false;
        self.acl_dirty = false;
        self.sync_editor = true;
        self.edit_data.clear();
        self.edit_acl.clear();
        self.load_node_detail(path);
        self.rebuild_rows();
    }

    pub(crate) fn toggle_expand(&mut self, path: &str) {
        let Some(node) = self.tree_nodes.get_mut(path) else {
            return;
        };
        if !tree_model::node_expandable(node) && node.children_loaded {
            return;
        }
        node.expanded = !node.expanded;
        let should_load = node.expanded && !node.children_loaded;
        if should_load {
            self.load_children(path);
        }
        self.rebuild_rows();
    }

    pub(crate) fn reveal_path(&mut self, path: &str) {
        if path == "/" {
            self.select_node("/");
            return;
        }
        if let Some(root) = self.tree_nodes.get_mut("/") {
            root.expanded = true;
            if !root.children_loaded {
                self.load_children("/");
            }
        }
        let mut current = "/".to_string();
        for seg in path.trim_start_matches('/').split('/') {
            let child = tree_model::child_path(&current, seg);
            self.tree_nodes
                .entry(child.clone())
                .or_insert_with(|| TreeNode::new(seg.to_string()));
            if let Some(node) = self.tree_nodes.get_mut(&child) {
                node.expanded = true;
                if !node.children_loaded {
                    self.load_children(&child);
                }
            }
            current = child;
        }
        self.select_node(path);
    }

    pub(crate) fn start_search(&mut self, query: String) {
        let query = query.trim().to_string();
        self.search_query = query.clone();
        if query.is_empty() {
            self.search_results.clear();
            self.search_in_progress = false;
            self.pending.search = None;
            self.rebuild_rows();
            return;
        }
        if self.connect_state != ConnectState::Connected {
            return;
        }
        self.search_in_progress = true;
        self.search_generation = self.search_generation.wrapping_add(1);
        let generation = self.search_generation;
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::SearchNodes {
            query,
            max_results: SEARCH_MAX_RESULTS,
            resp: tx,
        });
        self.pending.search = Some((rx, generation));
    }

    pub(crate) fn create_node(&mut self, name: String, data: String, mode: CreateMode) {
        let Some(parent) = self.selected_path.clone() else {
            self.fail("Select a parent node first");
            return;
        };
        if name.is_empty() || name.contains('/') {
            self.fail("Node name must be a single path segment");
            return;
        }
        let path = tree_model::child_path(&parent, &name);
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::Create {
            path,
            data: data.into_bytes(),
            acl: vec![world_acl()],
            mode,
            resp: tx,
        });
        self.pending.action = Some(rx);
    }

    pub(crate) fn delete_selected(&mut self) {
        let Some(path) = self.selected_path.clone() else {
            return;
        };
        if path == "/" {
            self.fail("Cannot delete the root node");
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::Delete {
            path,
            version: -1,
            resp: tx,
        });
        self.pending.action = Some(rx);
    }

    pub(crate) fn clear_children(&mut self) {
        let Some(path) = self.selected_path.clone() else {
            return;
        };
        self.clear_target = Some(path.clone());
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::DeleteChildren { path, resp: tx });
        self.pending.action = Some(rx);
    }

    pub(crate) fn save_data(&mut self) {
        let Some(detail) = self.detail.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::SetData {
            path: detail.path,
            data: self.edit_data.as_bytes().to_vec(),
            version: detail.stat.version,
            resp: tx,
        });
        self.pending.action = Some(rx);
        self.data_dirty = false;
    }

    pub(crate) fn save_acl(&mut self) {
        let Some(detail) = self.detail.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::SetAcl {
            path: detail.path,
            acl: self.edit_acl.clone(),
            version: detail.stat.aversion,
            resp: tx,
        });
        self.pending.action = Some(rx);
        self.acl_dirty = false;
    }

    pub(crate) fn run_four_letter(&mut self, cmd: &str) {
        if self.connect_state != ConnectState::Connected {
            self.fail("Not connected");
            return;
        }
        let host = self
            .hosts
            .split(',')
            .next()
            .unwrap_or(self.hosts.as_str())
            .trim()
            .to_string();
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::FourLetterCmd {
            host,
            cmd: cmd.to_string(),
            resp: tx,
        });
        self.pending.four_letter = Some(rx);
        self.server_output = format!("$ {cmd}\n…");
        self.inspector = InspectorTab::Server;
    }

    pub(crate) fn export_selected(&mut self) {
        let Some(path) = self.selected_path.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.zk.send(ZkCmd::ExportSubtree { path, resp: tx });
        self.pending.action = Some(rx);
        self.note("Exporting subtree…");
    }

    pub(crate) fn save_connection(
        &mut self,
        id: Option<i64>,
        name: String,
        hosts: String,
        timeout_ms: i32,
        auth_scheme: String,
        auth_credential: String,
    ) {
        let folder_id = self.selected_folder_id;
        let result = if let Some(id) = id {
            self.db.update_connection(
                id,
                &name,
                &hosts,
                timeout_ms,
                &auth_scheme,
                &auth_credential,
                folder_id,
            )
        } else {
            self.db
                .add_connection(
                    &name,
                    &hosts,
                    timeout_ms,
                    &auth_scheme,
                    &auth_credential,
                    folder_id,
                )
                .map(|_| ())
        };
        match result {
            Ok(()) => {
                self.reload_catalog();
                self.note("Connection saved");
            }
            Err(err) => self.fail(err.to_string()),
        }
    }

    pub(crate) fn delete_connection(&mut self, id: i64) {
        if self.active_conn_id == Some(id) {
            self.disconnect();
        }
        if let Err(err) = self.db.delete_connection(id) {
            self.fail(err.to_string());
            return;
        }
        self.reload_catalog();
    }

    pub(crate) fn create_folder(&mut self, name: String) {
        if name.trim().is_empty() {
            return;
        }
        if let Err(err) = self.db.create_folder(name.trim(), self.selected_folder_id) {
            self.fail(err.to_string());
            return;
        }
        self.reload_catalog();
    }

    pub(crate) fn delete_folder(&mut self, id: i64) {
        if let Err(err) = self.db.delete_folder(id) {
            self.fail(err.to_string());
            return;
        }
        self.expanded_folders.remove(&id);
        if self.selected_folder_id == Some(id) {
            self.selected_folder_id = None;
        }
        self.reload_catalog();
    }

    pub(crate) fn toggle_folder(&mut self, id: i64) {
        if !self.expanded_folders.remove(&id) {
            self.expanded_folders.insert(id);
        }
        self.selected_folder_id = Some(id);
    }

    pub(crate) fn move_connection(&mut self, id: i64, direction: i32) {
        let folder_id = self
            .root_connections
            .iter()
            .find(|conn| conn.id == id)
            .map(|_| None)
            .or_else(|| {
                self.folder_children
                    .iter()
                    .find_map(|(folder, (_, conns))| {
                        conns
                            .iter()
                            .any(|conn| conn.id == id)
                            .then_some(Some(*folder))
                    })
            })
            .unwrap_or(None);
        let siblings = if let Some(folder) = folder_id {
            self.folder_children
                .get(&folder)
                .map(|(_, conns)| conns.clone())
                .unwrap_or_default()
        } else {
            self.root_connections.clone()
        };
        let Some(index) = siblings.iter().position(|conn| conn.id == id) else {
            return;
        };
        let target = index as i32 + direction;
        if target < 0 || target as usize >= siblings.len() {
            return;
        }
        let before = if direction < 0 {
            Some(siblings[target as usize].id)
        } else {
            siblings.get(target as usize + 1).map(|conn| conn.id)
        };
        if let Err(err) = self.db.reorder_connection(id, before, folder_id) {
            self.fail(err.to_string());
            return;
        }
        self.reload_catalog();
    }

    pub(crate) fn rebuild_rows(&mut self) {
        self.rows = tree_model::flatten(&self.tree_nodes);
    }

    pub(crate) fn poll_responses(&mut self) -> bool {
        let mut changed = false;

        if let Some(rx) = self.pending.connect.take() {
            match rx.try_recv() {
                Ok(ZkResponse::Connected) => {
                    if let Some((scheme, credential)) = self.pending_auth.clone() {
                        let (tx, auth_rx) = mpsc::channel();
                        self.zk.send(ZkCmd::AddAuth {
                            scheme,
                            credential: credential.into_bytes(),
                            resp: tx,
                        });
                        self.pending.auth = Some(auth_rx);
                    } else {
                        self.finish_connected();
                    }
                    changed = true;
                }
                Ok(ZkResponse::Error(message)) => {
                    self.connect_state = ConnectState::Disconnected;
                    self.fail(message);
                    changed = true;
                }
                Ok(_) => changed = true,
                Err(mpsc::TryRecvError::Empty) => self.pending.connect = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.connect_state = ConnectState::Disconnected;
                    self.fail("Connection failed");
                    changed = true;
                }
            }
        }

        if let Some(rx) = self.pending.auth.take() {
            match rx.try_recv() {
                Ok(ZkResponse::AuthAdded) => {
                    self.pending_auth = None;
                    self.finish_connected();
                    changed = true;
                }
                Ok(ZkResponse::Error(message)) => {
                    self.connect_state = ConnectState::Disconnected;
                    self.zk.send(ZkCmd::Disconnect);
                    self.fail(message);
                    changed = true;
                }
                Ok(_) => changed = true,
                Err(mpsc::TryRecvError::Empty) => self.pending.auth = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.fail("Authentication failed");
                    changed = true;
                }
            }
        }

        let mut stats = Vec::new();
        let children_pending = std::mem::take(&mut self.pending.children);
        let mut children_keep = HashMap::new();
        for (path, rx) in children_pending {
            match rx.try_recv() {
                Ok(ZkResponse::Children(mut children)) => {
                    children.sort();
                    let node = self.tree_nodes.entry(path.clone()).or_insert_with(|| {
                        if path == "/" {
                            TreeNode::root()
                        } else {
                            TreeNode::new(
                                path.rsplit('/').next().unwrap_or(path.as_str()).to_string(),
                            )
                        }
                    });
                    node.children = children.clone();
                    node.children_loaded = true;
                    node.num_children = Some(children.len() as i32);
                    node.expanded = true;
                    for child in &children {
                        let child_path = tree_model::child_path(&path, child);
                        self.tree_nodes
                            .entry(child_path.clone())
                            .or_insert_with(|| TreeNode::new(child.clone()));
                        stats.push(child_path);
                    }
                    changed = true;
                }
                Ok(ZkResponse::Error(message)) => {
                    self.fail(message);
                    changed = true;
                }
                Ok(_) => changed = true,
                Err(mpsc::TryRecvError::Empty) => {
                    children_keep.insert(path, rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => changed = true,
            }
        }
        self.pending.children = children_keep;
        for path in stats {
            self.load_node_stat(&path);
        }

        let mut done = Vec::new();
        for (path, rx) in &self.pending.stat {
            if let Ok(resp) = rx.try_recv() {
                match resp {
                    ZkResponse::Stat(stat) => {
                        if let Some(node) = self.tree_nodes.get_mut(path) {
                            node.num_children = Some(stat.num_children);
                        }
                    }
                    ZkResponse::Error(_) => {
                        if let Some(node) = self.tree_nodes.get_mut(path) {
                            node.num_children = Some(0);
                        }
                    }
                    _ => {}
                }
                done.push(path.clone());
                changed = true;
            }
        }
        for path in done {
            self.pending.stat.remove(&path);
        }

        let mut done = Vec::new();
        for (path, rx) in &self.pending.data {
            if let Ok(resp) = rx.try_recv() {
                if let ZkResponse::Data { data, stat } = resp {
                    if self.selected_path.as_deref() == Some(path.as_str()) {
                        let text = String::from_utf8_lossy(&data).to_string();
                        let binary = std::str::from_utf8(&data).is_err();
                        if let Some(detail) = &mut self.detail {
                            detail.data = text.clone();
                            detail.data_raw = data;
                            detail.stat = stat;
                            detail.binary = binary;
                            if !self.data_dirty {
                                self.edit_data = text;
                                self.sync_editor = true;
                            }
                        }
                    }
                }
                done.push(path.clone());
                changed = true;
            }
        }
        for path in done {
            self.pending.data.remove(&path);
        }

        let mut done = Vec::new();
        for (path, rx) in &self.pending.acl {
            if let Ok(resp) = rx.try_recv() {
                if let ZkResponse::Acl { acl, stat } = resp {
                    if self.selected_path.as_deref() == Some(path.as_str()) {
                        if let Some(detail) = &mut self.detail {
                            detail.acl = acl.clone();
                            detail.stat = stat;
                            if !self.acl_dirty {
                                self.edit_acl = acl;
                            }
                        }
                    }
                }
                done.push(path.clone());
                changed = true;
            }
        }
        for path in done {
            self.pending.acl.remove(&path);
        }

        if let Some((rx, generation)) = self.pending.search.take() {
            match rx.try_recv() {
                Ok(ZkResponse::SearchResults(paths)) => {
                    if generation == self.search_generation {
                        self.search_results = paths;
                        self.search_in_progress = false;
                        changed = true;
                    }
                }
                Ok(ZkResponse::Error(message)) => {
                    if generation == self.search_generation {
                        self.search_in_progress = false;
                        self.fail(message);
                        changed = true;
                    }
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => self.pending.search = Some((rx, generation)),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.search_in_progress = false;
                    changed = true;
                }
            }
        }

        if let Some(rx) = self.pending.four_letter.take() {
            match rx.try_recv() {
                Ok(ZkResponse::FourLetterResult(text)) => {
                    self.server_output = text;
                    changed = true;
                }
                Ok(ZkResponse::Error(message)) => {
                    self.server_output = message.clone();
                    self.fail(message);
                    changed = true;
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => self.pending.four_letter = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => changed = true,
            }
        }

        if let Some(rx) = self.pending.action.take() {
            match rx.try_recv() {
                Ok(resp) => {
                    self.apply_action(resp);
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => self.pending.action = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }

        if changed {
            self.rebuild_rows();
        }
        changed
    }

    fn apply_action(&mut self, resp: ZkResponse) {
        match resp {
            ZkResponse::Created => {
                self.note("Node created");
                if let Some(path) = self.selected_path.clone() {
                    self.load_children(&path);
                }
            }
            ZkResponse::Deleted => {
                self.note("Node deleted");
                if let Some(path) = self.selected_path.clone() {
                    let parent = tree_model::parent_path(&path).to_string();
                    self.tree_nodes.remove(&path);
                    self.load_children(&parent);
                    self.select_node(&parent);
                }
            }
            ZkResponse::ChildrenCleared(count) => {
                self.note(format!("Cleared {count} node(s)"));
                if let Some(path) = self.clear_target.take() {
                    let prefix = if path == "/" {
                        "/".to_string()
                    } else {
                        format!("{path}/")
                    };
                    self.tree_nodes.retain(|key, _| {
                        key == &path || (path != "/" && !key.starts_with(&prefix))
                    });
                    if let Some(node) = self.tree_nodes.get_mut(&path) {
                        node.children.clear();
                        node.num_children = Some(0);
                        node.children_loaded = true;
                    }
                    self.load_children(&path);
                    self.load_node_detail(&path);
                }
            }
            ZkResponse::SetData => {
                self.note("Data saved");
                if let Some(path) = self.selected_path.clone() {
                    self.data_dirty = false;
                    self.load_node_detail(&path);
                }
            }
            ZkResponse::SetAcl => {
                self.note("ACL saved");
                if let Some(path) = self.selected_path.clone() {
                    self.acl_dirty = false;
                    self.load_node_detail(&path);
                }
            }
            ZkResponse::ExportData(value) => {
                self.export_text = serde_json::to_string_pretty(&value).unwrap_or_default();
                self.export_ready = true;
                self.note("Export ready");
            }
            ZkResponse::Error(message) => self.fail(message),
            _ => {}
        }
    }
}

fn world_acl() -> AclEntry {
    AclEntry {
        scheme: "world".into(),
        id: "anyone".into(),
        perms: 31,
    }
}

#[derive(Clone)]
pub struct NodeDetail {
    pub path: String,
    pub data: String,
    pub data_raw: Vec<u8>,
    pub stat: NodeStat,
    pub acl: Vec<AclEntry>,
    pub binary: bool,
}

impl NodeDetail {
    fn empty(path: &str) -> Self {
        Self {
            path: path.to_string(),
            data: String::new(),
            data_raw: Vec::new(),
            stat: NodeStat {
                czxid: 0,
                mzxid: 0,
                ctime: 0,
                mtime: 0,
                version: 0,
                cversion: 0,
                aversion: 0,
                ephemeral_owner: 0,
                data_length: 0,
                num_children: 0,
                pzxid: 0,
            },
            acl: Vec::new(),
            binary: false,
        }
    }
}

pub fn format_timestamp(ts: i64) -> String {
    if ts == 0 {
        return "—".into();
    }
    chrono::DateTime::from_timestamp_millis(ts)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| ts.to_string())
}

pub fn mode_index(mode: CreateMode) -> usize {
    match mode {
        CreateMode::Persistent => 0,
        CreateMode::Ephemeral => 1,
        CreateMode::PersistentSequential => 2,
        CreateMode::EphemeralSequential => 3,
    }
}

pub fn mode_from_index(index: usize) -> CreateMode {
    match index {
        1 => CreateMode::Ephemeral,
        2 => CreateMode::PersistentSequential,
        3 => CreateMode::EphemeralSequential,
        _ => CreateMode::Persistent,
    }
}
