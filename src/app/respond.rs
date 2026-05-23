use std::sync::mpsc;

use eframe::egui;

use crate::zk::ZkResponse;

use super::types::{ConnectState, TreeNode};
use super::ZkApp;

impl ZkApp {
    pub(crate) fn handle_responses(&mut self, ctx: &egui::Context) {
        let mut needs_repaint = false;

        // Pending connect
        if let Some(rx) = self.pending.connect.take() {
            match rx.try_recv() {
                Ok(ZkResponse::Connected) => {
                    let conn_id = self.active_conn_id.unwrap_or(0);
                    self.connect_state = ConnectState::Connected { host: self.hosts.clone(), conn_id };
                    self.status_message = format!("Connected to {}", self.hosts);
                    self.tree_nodes.entry("/".to_string()).or_insert_with(TreeNode::root);
                    self.load_children("/");
                    self.select_node("/");
                    needs_repaint = true;
                }
                Ok(ZkResponse::Error(msg)) => {
                    self.connect_state = ConnectState::Disconnected;
                    self.error_message = Some(msg);
                    needs_repaint = true;
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => { self.pending.connect = Some(rx); }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.connect_state = ConnectState::Disconnected;
                    self.error_message = Some("Connection failed".into());
                    needs_repaint = true;
                }
            }
        }

        // Pending children
        let mut done = vec![];
        let mut stat_to_load = Vec::new();
        for (path, rx) in &self.pending.children {
            if let Ok(resp) = rx.try_recv() {
                match resp {
                    ZkResponse::Children(mut children) => {
                        children.sort();
                        let node = self.tree_nodes.entry(path.clone()).or_insert_with(|| {
                            if path == "/" { TreeNode::root() } else {
                                TreeNode::new(
                                    path.rsplit('/').next().unwrap_or(path).to_string(),
                                    path.rsplit_once('/').map(|(p, _)| p).unwrap_or("/"),
                                )
                            }
                        });
                        node.children = children.clone();
                        node.children_loaded = true;
                        node.num_children = Some(children.len() as i32);
                        for child in &children {
                            let child_path = if path == "/" {
                                format!("/{}", child)
                            } else {
                                format!("{}/{}", path, child)
                            };
                            self.tree_nodes
                                .entry(child_path.clone())
                                .or_insert_with(|| TreeNode::new(child.clone(), path));
                            stat_to_load.push(child_path);
                        }
                        needs_repaint = true;
                    }
                    _ => {}
                }
                done.push(path.clone());
            }
        }
        for p in done {
            self.pending.children.remove(&p);
        }
        for child_path in stat_to_load {
            self.load_node_stat(&child_path);
        }

        // Pending stat (numChildren for icons)
        let mut stat_done = vec![];
        for (path, rx) in &self.pending.stat {
            if let Ok(resp) = rx.try_recv() {
                match resp {
                    ZkResponse::Stat(stat) => {
                        if let Some(node) = self.tree_nodes.get_mut(path) {
                            node.num_children = Some(stat.num_children);
                        }
                        needs_repaint = true;
                    }
                    ZkResponse::Error(_) => {
                        if let Some(node) = self.tree_nodes.get_mut(path) {
                            node.num_children = Some(0);
                        }
                    }
                    _ => {}
                }
                stat_done.push(path.clone());
            }
        }
        for p in stat_done {
            self.pending.stat.remove(&p);
        }

        // Pending data
        let mut done = vec![];
        for (path, rx) in &self.pending.data {
            if let Ok(resp) = rx.try_recv() {
                match resp {
                    ZkResponse::Data { data, stat } => {
                        let data_str = String::from_utf8_lossy(&data).to_string();
                        if self.selected_path.as_deref() == Some(path.as_str()) {
                            if let Some(detail) = &mut self.detail {
                                detail.data = data_str.clone();
                                detail.data_raw = data;
                                detail.stat = stat;
                                if !self.editing_data { self.edit_data = data_str; }
                            }
                        }
                        needs_repaint = true;
                    }
                    ZkResponse::Error(e) => { tracing::warn!("GetData error for {}: {}", path, e); }
                    _ => {}
                }
                done.push(path.clone());
            }
        }
        for p in done { self.pending.data.remove(&p); }

        // Pending ACL
        let mut done = vec![];
        for (path, rx) in &self.pending.acl {
            if let Ok(resp) = rx.try_recv() {
                match resp {
                    ZkResponse::Acl { acl, stat } => {
                        if self.selected_path.as_deref() == Some(path.as_str()) {
                            if let Some(detail) = &mut self.detail {
                                detail.acl = acl.clone();
                                detail.stat = stat;
                                if !self.editing_acl { self.edit_acl = acl; }
                            }
                        }
                        needs_repaint = true;
                    }
                    ZkResponse::Error(e) => { tracing::warn!("GetAcl error for {}: {}", path, e); }
                    _ => {}
                }
                done.push(path.clone());
            }
        }
        for p in done { self.pending.acl.remove(&p); }

        // Pending search
        if let Some((rx, gen)) = self.pending.search.take() {
            match rx.try_recv() {
                Ok(ZkResponse::SearchResults(paths)) => {
                    if gen == self.search_generation {
                        self.search_results = paths;
                        self.search_in_progress = false;
                        needs_repaint = true;
                    }
                }
                Ok(ZkResponse::Error(msg)) => {
                    if gen == self.search_generation {
                        self.search_in_progress = false;
                        self.error_message = Some(msg);
                        needs_repaint = true;
                    }
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => {
                    self.pending.search = Some((rx, gen));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if gen == self.search_generation {
                        self.search_in_progress = false;
                    }
                }
            }
        }

        // Pending action
        if let Some(rx) = self.pending.action.take() {
            match rx.try_recv() {
                Ok(resp) => {
                    match resp {
                        ZkResponse::Created => {
                            self.status_message = "Node created".into();
                            if let Some(sel) = self.selected_path.clone() { self.load_children(&sel); }
                        }
                        ZkResponse::Deleted => {
                            self.status_message = "Node deleted".into();
                            if let Some(sel) = self.selected_path.clone() {
                                let parent = sel.rsplit_once('/').map(|(p, _)| if p.is_empty() { "/" } else { p }).unwrap_or("/");
                                self.load_children(parent);
                                self.selected_path = Some(parent.to_string());
                                self.detail = None;
                            }
                        }
                        ZkResponse::ChildrenCleared(count) => {
                            self.status_message = format!("Cleared {} node(s)", count);
                            if let Some(path) = self.clear_children_target.take() {
                                self.purge_tree_under(&path);
                                if let Some(node) = self.tree_nodes.get_mut(&path) {
                                    node.children.clear();
                                    node.shown_count = 0;
                                    node.num_children = Some(0);
                                }
                                self.load_children(&path);
                                if self.selected_path.as_deref() == Some(path.as_str()) {
                                    self.load_node_detail(&path);
                                }
                            }
                        }
                        ZkResponse::SetData => {
                            self.status_message = "Data saved".into();
                            if let Some(path) = &self.selected_path.clone() { self.load_node_detail(path); }
                        }
                        ZkResponse::SetAcl => {
                            self.status_message = "ACL saved".into();
                            if let Some(path) = &self.selected_path.clone() { self.load_node_detail(path); }
                        }
                        ZkResponse::ImportDone => {
                            self.status_message = "Import completed".into();
                            self.load_children("/");
                        }
                        ZkResponse::Error(msg) => { self.error_message = Some(msg); }
                        _ => {}
                    }
                    needs_repaint = true;
                }
                Err(mpsc::TryRecvError::Empty) => { self.pending.action = Some(rx); }
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }

        if needs_repaint {
            ctx.request_repaint();
        }
    }
}
