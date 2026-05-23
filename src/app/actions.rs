use std::sync::mpsc;

use crate::zk::{AclEntry, ZkCmd};

use super::{ConnectState, TreeNode, ZkApp, ZnodeIconKind, SEARCH_MAX_RESULTS};

impl ZkApp {
    // ─── Connection actions ───

    pub(crate) fn do_connect(&mut self, conn_id: i64) {
        let (tx, rx) = mpsc::channel();
        self.zk_manager.send(ZkCmd::Connect {
            hosts: self.hosts.clone(),
            timeout_ms: self.timeout_ms,
            resp: tx,
        });
        self.connect_state = ConnectState::Connecting;
        self.active_conn_id = Some(conn_id);
        self.status_message = format!("Connecting to {}...", self.hosts);
        self.pending.connect = Some(rx);
    }

    pub(crate) fn do_disconnect(&mut self) {
        self.zk_manager.send(ZkCmd::Disconnect);
        self.connect_state = ConnectState::Disconnected;
        self.active_conn_id = None;
        self.detail = None;
        self.selected_path = None;
        self.selected_folder_id = None;
        self.tree_nodes.clear();
        self.search_query.clear();
        self.search_results.clear();
        self.search_in_progress = false;
        self.pending.search = None;
        self.search_pending_after = None;
        self.status_message = "Disconnected".into();
    }

    // ─── Data loading ───

    pub(crate) fn load_children(&mut self, path: &str) {
        let (tx, rx) = mpsc::channel();
        self.zk_manager.send(ZkCmd::GetChildren {
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
        self.zk_manager.send(ZkCmd::Exists {
            path: path.to_string(),
            resp: tx,
        });
        self.pending.stat.insert(path.to_string(), rx);
    }

    pub(crate) fn znode_icon_kind(node: &TreeNode, path: &str) -> ZnodeIconKind {
        if path == "/" {
            return ZnodeIconKind::Root;
        }
        if let Some(n) = node.num_children {
            return if n > 0 {
                ZnodeIconKind::ZkFolder
            } else {
                ZnodeIconKind::Document
            };
        }
        if node.children_loaded {
            if node.children.is_empty() {
                ZnodeIconKind::Document
            } else {
                ZnodeIconKind::ZkFolder
            }
        } else {
            ZnodeIconKind::Document
        }
    }

    pub(crate) fn znode_has_children(node: &TreeNode) -> bool {
        if let Some(n) = node.num_children {
            return n > 0;
        }
        !node.children_loaded || !node.children.is_empty()
    }

    pub(crate) fn start_tree_search(&mut self) {
        let q = self.search_query.trim().to_string();
        if q.is_empty() {
            self.search_results.clear();
            self.search_in_progress = false;
            self.pending.search = None;
            return;
        }
        if !matches!(self.connect_state, ConnectState::Connected { .. }) {
            return;
        }
        self.search_in_progress = true;
        self.search_generation = self.search_generation.wrapping_add(1);
        let gen = self.search_generation;
        let (tx, rx) = mpsc::channel();
        self.zk_manager.send(ZkCmd::SearchNodes {
            query: q,
            max_results: SEARCH_MAX_RESULTS,
            resp: tx,
        });
        self.pending.search = Some((rx, gen));
    }

    pub(crate) fn reveal_path(&mut self, path: &str) {
        if path == "/" {
            if let Some(root) = self.tree_nodes.get_mut("/") {
                root.expanded = true;
            }
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
            let child_path = if current == "/" {
                format!("/{}", seg)
            } else {
                format!("{}/{}", current, seg)
            };
            self.tree_nodes
                .entry(child_path.clone())
                .or_insert_with(|| TreeNode::new(seg.to_string(), &current));
            if let Some(n) = self.tree_nodes.get_mut(&child_path) {
                n.expanded = true;
                if !n.children_loaded {
                    self.load_children(&child_path);
                }
            }
            current = child_path;
        }
        self.select_node(path);
    }

    pub(crate) fn load_node_detail(&mut self, path: &str) {
        let (tx1, rx1) = mpsc::channel();
        self.zk_manager.send(ZkCmd::GetData {
            path: path.to_string(),
            resp: tx1,
        });
        self.pending.data.insert(path.to_string(), rx1);

        let (tx2, rx2) = mpsc::channel();
        self.zk_manager.send(ZkCmd::GetAcl {
            path: path.to_string(),
            resp: tx2,
        });
        self.pending.acl.insert(path.to_string(), rx2);
    }

    // ─── CRUD actions ───

    pub(crate) fn do_create_node(&mut self) {
        if self.selected_path.is_none() || self.create_name.is_empty() {
            return;
        }
        let parent = self.selected_path.as_ref().unwrap();
        let path = if parent == "/" {
            format!("/{}", self.create_name)
        } else {
            format!("{}/{}", parent, self.create_name)
        };
        let acl = vec![AclEntry {
            scheme: "world".into(),
            id: "anyone".into(),
            perms: 31,
        }];
        let (tx, rx) = mpsc::channel();
        self.zk_manager.send(ZkCmd::Create {
            path: path.clone(),
            data: self.create_data.as_bytes().to_vec(),
            acl,
            mode: self.create_mode,
            resp: tx,
        });
        self.pending.action = Some(rx);
        self.show_create_dialog = false;
        self.create_name.clear();
        self.create_data.clear();
    }

    pub(crate) fn do_delete_node(&mut self) {
        if let Some(path) = &self.selected_path {
            if path == "/" {
                self.error_message = Some("Cannot delete root node".into());
                return;
            }
            let (tx, rx) = mpsc::channel();
            self.zk_manager.send(ZkCmd::Delete {
                path: path.clone(),
                version: -1,
                resp: tx,
            });
            self.pending.action = Some(rx);
            self.confirm_delete = false;
        }
    }

    pub(crate) fn do_clear_children(&mut self) {
        let Some(path) = self.confirm_clear_children.take().or_else(|| self.selected_path.clone()) else {
            return;
        };
        self.clear_children_target = Some(path.clone());
        let (tx, rx) = mpsc::channel();
        self.zk_manager.send(ZkCmd::DeleteChildren {
            path,
            resp: tx,
        });
        self.pending.action = Some(rx);
    }

    pub(crate) fn purge_tree_under(&mut self, parent: &str) {
        self.tree_nodes.retain(|path, _| {
            if path == parent {
                return true;
            }
            if parent == "/" {
                return false;
            }
            let prefix = format!("{}/", parent);
            !path.starts_with(&prefix)
        });
    }

    pub(crate) fn do_save_data(&mut self) {
        if let Some(detail) = &self.detail {
            let (tx, rx) = mpsc::channel();
            self.zk_manager.send(ZkCmd::SetData {
                path: detail.path.clone(),
                data: self.edit_data.as_bytes().to_vec(),
                version: detail.stat.version,
                resp: tx,
            });
            self.pending.action = Some(rx);
            self.editing_data = false;
        }
    }

    pub(crate) fn do_save_acl(&mut self) {
        if let Some(detail) = &self.detail {
            let (tx, rx) = mpsc::channel();
            self.zk_manager.send(ZkCmd::SetAcl {
                path: detail.path.clone(),
                acl: self.edit_acl.clone(),
                version: detail.stat.version,
                resp: tx,
            });
            self.pending.action = Some(rx);
            self.editing_acl = false;
        }
    }

    pub(crate) fn connect_from_profile(&mut self, conn: &crate::db::ConnProfile) {
        self.hosts = conn.hosts.clone();
        self.timeout_ms = conn.timeout_ms;
        self.active_conn_name = conn.name.clone();
        self.do_connect(conn.id);
        let _ = self.db.touch_connection(conn.id);
    }
}
