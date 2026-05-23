use eframe::egui;

use crate::db::{ConnProfile, Folder};

use super::t;
use super::types::{ConnectState, DragItem, DragItemKind, SEARCH_MAX_RESULTS};
use super::ZkApp;

impl ZkApp {
    pub(crate) fn render_search_results(&mut self, ui: &mut egui::Ui, content_w: f32) {
        let lang = self.lang;
        if self.search_in_progress {
            ui.horizontal(|ui| {
                ui.set_max_width(content_w);
                ui.spinner();
                ui.label(
                    egui::RichText::new(t!(lang, "Searching entire tree...", "正在搜索全部节点..."))
                        .weak()
                        .size(11.0),
                );
            });
        }
        if self.search_results.is_empty() && !self.search_in_progress {
            return;
        }
        let count = self.search_results.len();
        let capped = count >= SEARCH_MAX_RESULTS;
        let header = if capped {
            t!(
                lang,
                &format!("{}+ matches (capped)", count),
                &format!("{}+ 条匹配（已达上限）", count)
            )
        } else {
            t!(
                lang,
                &format!("{} matches", count),
                &format!("{} 条匹配", count)
            )
        };
        ui.label(egui::RichText::new(header).weak().size(11.0));

        let results: Vec<String> = self.search_results.clone();
        let mut clicked_path: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt("search_results")
            .max_height(160.0)
            .max_width(content_w)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(content_w);
                for hit_path in &results {
                    let name = hit_path.rsplit('/').next().unwrap_or(hit_path);
                    let is_sel = self.selected_path.as_deref() == Some(hit_path.as_str());
                    let row_w = ui.available_width();
                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(row_w, Self::tree_row_height()),
                        egui::Sense::hover(),
                    );
                    Self::paint_tree_row_highlight(ui, row_rect, is_sel);
                    ui.allocate_new_ui(
                        egui::UiBuilder::new()
                            .max_rect(row_rect)
                            .layout(Self::tree_row_layout()),
                        |ui| {
                            ui.set_height(Self::tree_row_height());
                            Self::paint_znode_document_icon(ui);
                            let mut name_rt = egui::RichText::new(name).size(12.0);
                            if is_sel {
                                name_rt = name_rt.strong();
                            }
                            Self::tree_label(ui, name_rt);
                        },
                    );
                    let row_interact = Self::tree_row_pointer(
                        ui,
                        row_rect,
                        ui.id().with(("search_hit", hit_path)),
                        false,
                    );
                    if row_interact.clicked() {
                        clicked_path = Some(hit_path.clone());
                    }
                    let _ = row_interact.on_hover_text(hit_path.as_str());
                }
            });
        if let Some(path) = clicked_path {
            self.reveal_path(&path);
        }
    }

    pub(crate) fn sidebar_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let content_w = ui.available_width();

        // Disable drag-to-select on tree labels; TextEdit in search box still selects normally.
        ui.style_mut().interaction.selectable_labels = false;

        ui.horizontal(|ui| {
            ui.set_max_width(content_w);
            ui.label(egui::RichText::new(t!(lang, "Resource Manager", "资源管理器")).strong().size(14.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("+").on_hover_text(t!(lang, "New Connection", "新建连接")).clicked() {
                    self.open_conn_dialog(None);
                }
                if ui.small_button(t!(lang, "New Folder", "新建文件夹")).on_hover_text(t!(lang, "New Folder", "新建文件夹")).clicked() {
                    self.new_folder_name.clear();
                    self.new_folder_parent_id = None;
                    self.rename_folder_id = None;
                    self.show_folder_dialog = true;
                }
            });
        });
        // Search box (visible when connected)
        if matches!(self.connect_state, ConnectState::Connected { .. }) {
            ui.horizontal(|ui| {
                ui.set_max_width(content_w);
                ui.label(
                    egui::RichText::new(t!(lang, "Search:", "搜索:"))
                        .size(12.0)
                        .weak(),
                );
                if !self.search_query.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new("X").frame(false).small())
                            .on_hover_text(t!(lang, "Clear", "清除"))
                            .clicked()
                        {
                            self.search_query.clear();
                            self.search_results.clear();
                            self.search_in_progress = false;
                            self.pending.search = None;
                            self.search_pending_after = None;
                        }
                    });
                }
            });
            let edit_h = ui.spacing().interact_size.y;
            let resp = ui.add_sized(
                egui::vec2(content_w, edit_h),
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text(t!(lang, "name or path", "节点名或路径"))
                    .clip_text(true),
            );
            if resp.changed() {
                let now = ui.input(|i| i.time);
                self.search_pending_after = Some(now + 0.35);
            }
            if !self.search_query.trim().is_empty() {
                self.render_search_results(ui, content_w);
            }
        }
        ui.separator();

        let mut drop_target: Option<(Option<i64>, Option<i64>, Option<i64>)> = None; // (before_folder_id, before_conn_id, target_folder_id)

        egui::ScrollArea::vertical()
            .id_salt("sidebar_tree")
            .auto_shrink([false, false])
            .max_width(content_w)
            .show(ui, |ui| {
                ui.set_max_width(content_w);
                let has_root = matches!(&self.connect_state, ConnectState::Connected { .. }) && self.tree_nodes.contains_key("/");

                // Root folders
                let root_folders = self.db.get_subfolders(None).unwrap_or_default();
                for folder in &root_folders {
                    if let Some(dt) = self.render_folder_node(ui, folder) {
                        drop_target = Some(dt);
                    }
                }

                // Root connections (not in any folder)
                if let Ok(conns) = self.db.get_connections_in_folder(None) {
                    for conn in &conns {
                        if let Some(dt) = self.render_connection_row(ui, conn) {
                            drop_target = Some(dt);
                        }
                    }
                }

                // Drop zone at the end of root list
                if self.drag_state.is_some() {
                    let rect = ui.min_rect();
                    let drop_zone = egui::Rect::from_min_size(
                        egui::pos2(rect.left(), rect.bottom()),
                        egui::vec2(rect.width(), 12.0),
                    );
                    if ui.rect_contains_pointer(drop_zone) {
                        drop_target = Some((None, None, None)); // end of root
                        ui.painter().hline(
                            drop_zone.x_range(),
                            drop_zone.top(),
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 255)),
                        );
                    }
                }

                // Empty state
                if !has_root {
                    let all_conns = self.db.get_connections_in_folder(None).unwrap_or_default();
                    let all_folders = self.db.get_subfolders(None).unwrap_or_default();
                    if all_conns.is_empty() && all_folders.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(t!(lang, "No connections", "暂无连接")).weak());
                            if ui.link(t!(lang, "Create one", "创建一个")).clicked() {
                                self.open_conn_dialog(None);
                            }
                        });
                    }
                }
            });

        // Handle drop
        if self.drag_state.is_some() && ui.input(|i| i.pointer.any_released()) {
            if let (Some(drag), Some((before_fid, before_cid, target_fid))) = (self.drag_state.take(), drop_target) {
                match drag.kind {
                    DragItemKind::Folder => {
                        let _ = self.db.reorder_folder(drag.id, before_fid, target_fid);
                    }
                    DragItemKind::Connection => {
                        let _ = self.db.reorder_connection(drag.id, before_cid, target_fid);
                    }
                }
            }
            self.drag_state = None;
        }
    }

    pub(crate) fn render_folder_node(&mut self, ui: &mut egui::Ui, folder: &Folder) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
        let lang = self.lang;
        let expanded = self.folder_expanded.contains(&folder.id);
        let is_selected = self.selected_folder_id == Some(folder.id);
        let is_dragging = self.drag_state.is_some();
        let is_this_dragged = self.drag_state.map_or(false, |d| d.kind == DragItemKind::Folder && d.id == folder.id);
        let mut drop_target = None;

        // Drop zone above this folder
        if is_dragging && !is_this_dragged {
            let pre_rect = ui.min_rect();
            let drop_pre = egui::Rect::from_min_size(
                egui::pos2(pre_rect.left(), pre_rect.top() - 2.0),
                egui::vec2(pre_rect.width(), 8.0),
            );
            if ui.rect_contains_pointer(drop_pre) {
                let target_folder_id = folder.parent_id;
                drop_target = Some((Some(folder.id), None, target_folder_id));
            }
        }

        let row_width = ui.available_width();
        let (row_rect, _) = ui.allocate_exact_size(
            egui::vec2(row_width, Self::tree_row_height()),
            egui::Sense::hover(),
        );
        Self::paint_tree_row_highlight(ui, row_rect, is_selected);

        let row_interact =
            Self::tree_row_pointer(ui, row_rect, ui.id().with(("folder", folder.id)), true);

        let mut icon_clicked = false;
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(row_rect)
                .layout(Self::tree_row_layout()),
            |ui| {
                ui.set_height(Self::tree_row_height());
                if is_this_dragged {
                    ui.painter().rect_filled(
                        ui.available_rect_before_wrap(),
                        2.0,
                        egui::Color32::from_rgba_premultiplied(100, 180, 255, 40),
                    );
                }

                let icon_resp = Self::paint_resource_folder_icon(ui, expanded);
                if icon_resp.clicked() {
                    icon_clicked = true;
                    self.toggle_resource_folder(folder.id);
                }
                Self::tree_name_label(ui, &folder.name, is_selected);
            },
        );

        if row_interact.drag_started() && self.drag_state.is_none() {
            self.drag_state = Some(DragItem {
                id: folder.id,
                kind: DragItemKind::Folder,
                source_folder_id: folder.parent_id,
            });
        }
        row_interact.context_menu(|ui| {
            if ui.button(t!(lang, "New Subfolder...", "新建子文件夹...")).clicked() {
                self.new_folder_name.clear();
                self.new_folder_parent_id = Some(folder.id);
                self.rename_folder_id = None;
                self.show_folder_dialog = true;
                ui.close_menu();
            }
            if ui.button(t!(lang, "New Connection...", "新建连接...")).clicked() {
                self.open_conn_dialog(Some(folder.id));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(t!(lang, "Rename", "重命名")).clicked() {
                self.new_folder_name = folder.name.clone();
                self.new_folder_parent_id = folder.parent_id;
                self.rename_folder_id = Some(folder.id);
                self.show_folder_dialog = true;
                ui.close_menu();
            }
            if ui.button(t!(lang, "Delete Folder", "删除文件夹")).clicked() {
                let _ = self.db.delete_folder(folder.id);
                if self.selected_folder_id == Some(folder.id) {
                    self.selected_folder_id = None;
                }
                ui.close_menu();
            }
        });

        if row_interact.double_clicked() {
            self.toggle_resource_folder(folder.id);
        } else if row_interact.clicked() && !icon_clicked {
            self.select_folder(folder.id);
        }

        // Show drop indicator line
        if let Some((Some(_), _, _)) = &drop_target {
            let r = ui.min_rect();
            ui.painter().hline(
                r.x_range(),
                r.top(),
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 255)),
            );
        }

        if expanded {
            ui.push_id(folder.id, |ui| {
                ui.indent("folder_indent", |ui| {
                    let subfolders = self.db.get_subfolders(Some(folder.id)).unwrap_or_default();
                    for sub in &subfolders {
                        if let Some(dt) = self.render_folder_node(ui, sub) {
                            drop_target = Some(dt);
                        }
                    }
                    if let Ok(conns) = self.db.get_connections_in_folder(Some(folder.id)) {
                        for conn in &conns {
                            if let Some(dt) = self.render_connection_row(ui, conn) {
                                drop_target = Some(dt);
                            }
                        }
                    }
                });
            });
        }

        drop_target
    }

    pub(crate) fn render_connection_row(&mut self, ui: &mut egui::Ui, conn: &ConnProfile) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
        let lang = self.lang;
        let connected = matches!(&self.connect_state, ConnectState::Connected { conn_id, .. } if *conn_id == conn.id);
        let is_active = connected && self.tree_nodes.contains_key("/");
        let is_dragging = self.drag_state.is_some();
        let is_this_dragged = self.drag_state.map_or(false, |d| d.kind == DragItemKind::Connection && d.id == conn.id);
        let mut drop_target = None;

        // Drop zone above this connection
        if is_dragging && !is_this_dragged {
            let pre_rect = ui.min_rect();
            let drop_pre = egui::Rect::from_min_size(
                egui::pos2(pre_rect.left(), pre_rect.top() - 2.0),
                egui::vec2(pre_rect.width(), 8.0),
            );
            if ui.rect_contains_pointer(drop_pre) {
                drop_target = Some((None, Some(conn.id), conn.folder_id));
            }
        }

        let label = conn.name.clone();
        let is_root_selected = self.is_connection_root_selected(conn.id);

        let row_width = ui.available_width();
        let (row_rect, _) = ui.allocate_exact_size(
            egui::vec2(row_width, Self::tree_row_height()),
            egui::Sense::hover(),
        );
        Self::paint_tree_row_highlight(ui, row_rect, is_root_selected);

        let row_interact =
            Self::tree_row_pointer(ui, row_rect, ui.id().with(("conn", conn.id)), true);

        let mut icon_clicked = false;
        ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(row_rect)
                .layout(Self::tree_row_layout()),
            |ui| {
                ui.set_height(Self::tree_row_height());
                if is_this_dragged {
                    ui.painter().rect_filled(
                        ui.available_rect_before_wrap(),
                        2.0,
                        egui::Color32::from_rgba_premultiplied(100, 180, 255, 40),
                    );
                }

                let icon_resp = Self::paint_zk_connection_icon(ui, connected);
                if icon_resp.clicked() {
                    icon_clicked = true;
                    if connected && is_active {
                        self.toggle_connection_tree_expand();
                    } else if !connected {
                        self.connect_from_profile(conn);
                    }
                }

                Self::tree_name_label(ui, &label, is_root_selected);
            },
        );

        if row_interact.drag_started() && self.drag_state.is_none() {
            self.drag_state = Some(DragItem {
                id: conn.id,
                kind: DragItemKind::Connection,
                source_folder_id: conn.folder_id,
            });
        }
        row_interact.context_menu(|ui| {
            if !connected {
                if ui.button(t!(lang, "Connect", "连接")).clicked() {
                    self.connect_from_profile(conn);
                    ui.close_menu();
                }
            } else if is_active {
                if ui.button(t!(lang, "Disconnect", "断开")).clicked() {
                    self.do_disconnect();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(t!(lang, "Refresh Children", "刷新子节点")).clicked() {
                    self.load_children("/");
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(t!(lang, "New Child Node...", "新建子节点...")).clicked() {
                    self.select_node("/");
                    self.show_create_dialog = true;
                    ui.close_menu();
                }
                if ui.button(t!(lang, "Clear Children...", "清空子节点...")).clicked() {
                    self.select_node("/");
                    self.confirm_clear_children = Some("/".to_string());
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button(t!(lang, "Edit...", "编辑...")).clicked() {
                self.open_conn_dialog_for_edit(conn);
                ui.close_menu();
            }
            if ui.button(t!(lang, "Delete", "删除")).clicked() {
                let _ = self.db.delete_connection(conn.id);
                ui.close_menu();
            }
        });
        if row_interact.double_clicked() {
            if connected && is_active {
                self.toggle_connection_tree_expand();
            } else if !connected {
                self.connect_from_profile(conn);
            }
        } else if connected && is_active && row_interact.clicked() && !icon_clicked {
            self.select_node("/");
        }
        let _ = row_interact.on_hover_text(format!("{}\n{}", conn.name, conn.hosts));

        // Show drop indicator line
        if let Some((_, Some(_), _)) = &drop_target {
            let r = ui.min_rect();
            ui.painter().hline(
                r.x_range(),
                r.top(),
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 255)),
            );
        }

        if connected && is_active {
            if self.tree_nodes.get("/").map_or(false, |n| n.expanded) {
                ui.indent("zk_tree", |ui| {
                    self.render_zk_children(ui, "/", 0);
                });
            }
        }

        drop_target
    }
}
