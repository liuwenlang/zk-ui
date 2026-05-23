use eframe::egui;

use super::t;
use super::types::{ConnectState, NodeDetail, CHILDREN_BATCH_SIZE};
use super::ZkApp;
use crate::zk::NodeStat;

impl ZkApp {
    // ─── Selection ───

    pub(crate) fn select_node(&mut self, path: &str) {
        self.selected_folder_id = None;
        self.selected_path = Some(path.to_string());
        self.detail = Some(NodeDetail {
            path: path.to_string(),
            data: String::new(),
            data_raw: vec![],
            stat: NodeStat {
                czxid: 0, mzxid: 0, ctime: 0, mtime: 0,
                version: 0, cversion: 0, aversion: 0,
                ephemeral_owner: 0, data_length: 0,
                num_children: 0, pzxid: 0,
            },
            acl: vec![],
        });
        self.editing_data = false;
        self.editing_acl = false;
        self.load_node_detail(path);
    }

    pub(crate) fn select_folder(&mut self, folder_id: i64) {
        self.selected_folder_id = Some(folder_id);
        self.selected_path = None;
        self.detail = None;
        self.confirm_delete = false;
        self.confirm_clear_children = None;
        self.editing_data = false;
        self.editing_acl = false;
    }

    // ─── Layout helpers ───

    pub(crate) fn tree_row_height() -> f32 {
        18.0
    }

    pub(crate) fn tree_row_layout() -> egui::Layout {
        egui::Layout::left_to_right(egui::Align::Center)
    }

    /// One fixed-height row: icon + label vertically centered on the same baseline.
    pub(crate) fn tree_row_scope<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(w, Self::tree_row_height()),
            Self::tree_row_layout(),
            |ui| {
                ui.set_height(Self::tree_row_height());
                add(ui)
            },
        )
        .inner
    }

    /// Tree row label — display only; row-level interact handles clicks.
    pub(crate) fn tree_label(ui: &mut egui::Ui, text: egui::RichText) -> egui::Response {
        ui.add(egui::Label::new(text).selectable(false))
    }

    pub(crate) fn tree_name_label(ui: &mut egui::Ui, label: &str, emphasize: bool) -> egui::Response {
        let mut text = egui::RichText::new(label).size(13.0);
        if emphasize {
            text = text.strong();
        }
        Self::tree_label(ui, text)
    }

    pub(crate) fn paint_tree_row_highlight(ui: &mut egui::Ui, row_rect: egui::Rect, is_selected: bool) {
        if !ui.is_rect_visible(row_rect) {
            return;
        }
        if is_selected {
            ui.painter().rect_filled(
                row_rect,
                4.0,
                egui::Color32::from_rgba_premultiplied(70, 130, 220, 55),
            );
        } else if row_rect.contains(
            ui.input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(-1.0, -1.0))),
        ) {
            ui.painter().rect_filled(
                row_rect,
                4.0,
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 12),
            );
        }
    }

    pub(crate) fn tree_row_pointer(ui: &mut egui::Ui, row_rect: egui::Rect, id: egui::Id, draggable: bool) -> egui::Response {
        let sense = if draggable {
            egui::Sense::click() | egui::Sense::drag()
        } else {
            egui::Sense::click()
        };
        ui.interact(row_rect, id, sense)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    // ─── Interaction ───

    pub(crate) fn toggle_znode_expand(&mut self, path: &str) {
        if let Some(n) = self.tree_nodes.get_mut(path) {
            n.expanded = !n.expanded;
            if !n.children_loaded {
                self.load_children(path);
            }
        }
    }

    pub(crate) fn toggle_resource_folder(&mut self, folder_id: i64) {
        if self.folder_expanded.contains(&folder_id) {
            self.folder_expanded.remove(&folder_id);
        } else {
            self.folder_expanded.insert(folder_id);
        }
    }

    pub(crate) fn toggle_connection_tree_expand(&mut self) {
        if let Some(root) = self.tree_nodes.get_mut("/") {
            root.expanded = !root.expanded;
            if root.expanded && !root.children_loaded {
                self.load_children("/");
            }
        }
    }

    pub(crate) fn is_connection_root_selected(&self, conn_id: i64) -> bool {
        matches!(
            &self.connect_state,
            ConnectState::Connected { conn_id: id, .. } if *id == conn_id
        ) && self.selected_path.as_deref() == Some("/")
    }

    // ─── Rendering ───

    pub(crate) fn render_tree_node(&mut self, ui: &mut egui::Ui, path: &str, depth: usize) {
        let node_data = self.tree_nodes.get(path).cloned();
        if let Some(node) = node_data {
            let is_selected = self.selected_path.as_deref() == Some(path);
            let icon_kind = Self::znode_icon_kind(&node, path);
            let has_children = Self::znode_has_children(&node);

            let row_width = ui.available_width();
            let (row_rect, _) = ui.allocate_exact_size(
                egui::vec2(row_width, Self::tree_row_height()),
                egui::Sense::hover(),
            );

            Self::paint_tree_row_highlight(ui, row_rect, is_selected);

            let row_id = ui.id().with(("zk_node", path));
            let row_interact = Self::tree_row_pointer(ui, row_rect, row_id, false);

            let mut icon_clicked = false;
            ui.allocate_new_ui(
                egui::UiBuilder::new()
                    .max_rect(row_rect)
                    .layout(Self::tree_row_layout()),
                |ui| {
                    ui.set_height(Self::tree_row_height());
                    ui.add_space((depth as f32) * 14.0);

                    let expanded = node.expanded;
                    if let Some(icon_resp) = Self::paint_znode_icon(ui, icon_kind, expanded) {
                        if icon_resp.clicked() {
                            icon_clicked = true;
                            self.toggle_znode_expand(path);
                        }
                    }

                    let mut text = egui::RichText::new(&node.name).size(13.0);
                    if is_selected {
                        text = text.strong();
                    }
                    Self::tree_label(ui, text);
                },
            );

            row_interact.context_menu(|ui| {
                if ui.button(t!(self.lang, "Refresh Children", "刷新子节点")).clicked() {
                    self.load_children(path);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(t!(self.lang, "New Child Node...", "新建子节点...")).clicked() {
                    self.select_node(path);
                    self.show_create_dialog = true;
                    ui.close_menu();
                }
                if has_children {
                    if ui.button(t!(self.lang, "Clear Children...", "清空子节点...")).clicked() {
                        self.select_node(path);
                        self.confirm_clear_children = Some(path.to_string());
                        ui.close_menu();
                    }
                }
                if path != "/" {
                    ui.separator();
                    if ui.button(t!(self.lang, "Delete Node", "删除节点")).clicked() {
                        self.select_node(path);
                        self.confirm_delete = true;
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button(t!(self.lang, "Copy Path", "复制路径")).clicked() {
                    ui.output_mut(|o| o.copied_text = path.to_string());
                    ui.close_menu();
                }
            });
            if row_interact.double_clicked() && has_children {
                self.toggle_znode_expand(path);
            } else if row_interact.clicked() && !icon_clicked {
                self.select_node(path);
            }
            let _ = row_interact.on_hover_text(path);

            if node.expanded && has_children {
                self.render_zk_children(ui, path, depth + 1);
            }
        }
    }

    pub(crate) fn render_zk_children(&mut self, ui: &mut egui::Ui, parent_path: &str, depth: usize) {
        let Some(node) = self.tree_nodes.get(parent_path).cloned() else {
            return;
        };
        let (total, visible_children) = {
            let end = node.shown_count.min(node.children.len());
            (node.children.len(), node.children[..end].to_vec())
        };

        for child in &visible_children {
            let child_path = if parent_path == "/" {
                format!("/{}", child)
            } else {
                format!("{}/{}", parent_path, child)
            };
            self.render_tree_node(ui, &child_path, depth);
        }

        if visible_children.len() < total {
            Self::tree_row_scope(ui, |ui| {
                ui.add_space((depth as f32) * 14.0 + 28.0);
                let remaining = total - visible_children.len();
                let label = format!("... {} more ({} total)", remaining, total);
                if ui
                    .add(egui::Button::new(egui::RichText::new(label).size(11.0).weak()).frame(false))
                    .clicked()
                {
                    if let Some(n) = self.tree_nodes.get_mut(parent_path) {
                        n.shown_count += CHILDREN_BATCH_SIZE;
                    }
                }
            });
        }
    }
}
