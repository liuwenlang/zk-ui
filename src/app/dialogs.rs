use eframe::egui;

use crate::db::{ConnProfile, LocalDb};

use super::t;
use super::ZkApp;

impl ZkApp {
    pub(crate) fn open_conn_dialog(&mut self, folder_id: Option<i64>) {
        self.edit_conn = None;
        self.edit_conn_name.clear();
        self.edit_conn_hosts = "127.0.0.1:2181".into();
        self.edit_conn_timeout = 5000;
        self.edit_conn_auth_scheme = "digest".into();
        self.edit_conn_auth_credential.clear();
        self.edit_conn_folder_id = folder_id;
        self.show_conn_dialog = true;
    }

    pub(crate) fn open_conn_dialog_for_edit(&mut self, conn: &ConnProfile) {
        self.edit_conn = Some(conn.clone());
        self.edit_conn_name = conn.name.clone();
        self.edit_conn_hosts = conn.hosts.clone();
        self.edit_conn_timeout = conn.timeout_ms;
        self.edit_conn_auth_scheme = conn.auth_scheme.clone();
        self.edit_conn_auth_credential = conn.auth_credential.clone();
        self.edit_conn_folder_id = conn.folder_id;
        self.show_conn_dialog = true;
    }

    pub(crate) fn show_conn_dialog_inline(&mut self, ui: &mut egui::Ui) {
        if !self.show_conn_dialog { return; }
        let lang = self.lang;
        let is_edit = self.edit_conn.is_some();
        let title = if is_edit { t!(lang, "Edit Connection", "编辑连接") } else { t!(lang, "New Connection", "新建连接") };
        let mut should_save = false;
        let mut should_save_connect = false;

        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_min_width(300.0);
                    ui.label(egui::RichText::new(title).strong().size(16.0));
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(t!(lang, "Name:", "名称:"));
                        ui.text_edit_singleline(&mut self.edit_conn_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label(t!(lang, "Hosts:", "地址:"));
                        ui.text_edit_singleline(&mut self.edit_conn_hosts);
                    });
                    ui.horizontal(|ui| {
                        ui.label(t!(lang, "Timeout(ms):", "超时(ms):"));
                        ui.add(egui::DragValue::new(&mut self.edit_conn_timeout).range(1000..=60000));
                    });
                    // Folder picker
                    let mut folder_list: Vec<(i64, String)> = Vec::new();
                    Self::build_folder_list(&self.db, None, 0, &mut folder_list);
                    ui.horizontal(|ui| {
                        ui.label(t!(lang, "Folder:", "文件夹:"));
                        let current_label = if let Some(fid) = self.edit_conn_folder_id {
                            self.find_folder_name(fid)
                        } else {
                            t!(lang, "(None)", "(无)").to_string()
                        };
                        egui::ComboBox::from_id_salt("conn_folder")
                            .selected_text(&current_label)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.edit_conn_folder_id, None, t!(lang, "(None)", "(无)"));
                                Self::show_folder_options(ui, &folder_list, &mut self.edit_conn_folder_id);
                            });
                    });
                    ui.collapsing(t!(lang, "Authentication", "认证"), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(t!(lang, "Scheme:", "方案:"));
                            ui.text_edit_singleline(&mut self.edit_conn_auth_scheme);
                        });
                        ui.horizontal(|ui| {
                            ui.label(t!(lang, "Credential:", "凭证:"));
                            ui.text_edit_singleline(&mut self.edit_conn_auth_credential);
                        });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(t!(lang, "Save", "保存")).clicked() {
                            should_save = true;
                        }
                        if ui.button(t!(lang, "Save & Connect", "保存并连接")).clicked() {
                            should_save = true;
                            should_save_connect = true;
                        }
                        if ui.button(t!(lang, "Cancel", "取消")).clicked() {
                            self.show_conn_dialog = false;
                        }
                    });
                });
        });
        if should_save {
            if self.edit_conn_name.is_empty() {
                self.edit_conn_name = self.edit_conn_hosts.clone();
            }
            let saved_id = if let Some(conn) = &self.edit_conn {
                let _ = self.db.update_connection(
                    conn.id, &self.edit_conn_name, &self.edit_conn_hosts,
                    self.edit_conn_timeout, &self.edit_conn_auth_scheme,
                    &self.edit_conn_auth_credential, self.edit_conn_folder_id,
                );
                conn.id
            } else {
                self.db.add_connection(
                    &self.edit_conn_name, &self.edit_conn_hosts,
                    self.edit_conn_timeout, &self.edit_conn_auth_scheme,
                    &self.edit_conn_auth_credential, self.edit_conn_folder_id,
                ).unwrap_or(0)
            };
            self.show_conn_dialog = false;
            if should_save_connect {
                self.hosts = self.edit_conn_hosts.clone();
                self.timeout_ms = self.edit_conn_timeout;
                self.active_conn_name = self.edit_conn_name.clone();
                self.do_connect(saved_id);
            }
        }
    }

    pub(crate) fn find_folder_name(&self, id: i64) -> String {
        fn search(db: &LocalDb, parent_id: Option<i64>, target: i64, prefix: &str) -> Option<String> {
            for f in db.get_subfolders(parent_id).unwrap_or_default() {
                let name = if prefix.is_empty() { f.name.clone() } else { format!("{}/{}", prefix, f.name) };
                if f.id == target {
                    return Some(name);
                }
                if let Some(r) = search(db, Some(f.id), target, &name) {
                    return Some(r);
                }
            }
            None
        }
        search(&self.db, None, id, "").unwrap_or_else(|| format!("#{}", id))
    }

    pub(crate) fn build_folder_list(db: &LocalDb, parent_id: Option<i64>, depth: usize, out: &mut Vec<(i64, String)>) {
        let folders = db.get_subfolders(parent_id).unwrap_or_default();
        for f in folders {
            let indent = "  ".repeat(depth);
            out.push((f.id, format!("{}{}", indent, f.name)));
            Self::build_folder_list(db, Some(f.id), depth + 1, out);
        }
    }

    pub(crate) fn show_folder_options(ui: &mut egui::Ui, folders: &[(i64, String)], selected: &mut Option<i64>) {
        for (id, label) in folders {
            ui.selectable_value(selected, Some(*id), label);
        }
    }

    pub(crate) fn show_folder_dialog_inline(&mut self, ui: &mut egui::Ui) {
        if !self.show_folder_dialog { return; }
        let lang = self.lang;
        let is_rename = self.rename_folder_id.is_some();
        let title = if is_rename { t!(lang, "Rename Folder", "重命名文件夹") } else { t!(lang, "New Folder", "新建文件夹") };
        let mut should_save = false;
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_min_width(250.0);
                    ui.label(egui::RichText::new(title).strong().size(16.0));
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(t!(lang, "Name:", "名称:"));
                        ui.text_edit_singleline(&mut self.new_folder_name);
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(t!(lang, "OK", "确定")).clicked() && !self.new_folder_name.is_empty() {
                            should_save = true;
                        }
                        if ui.button(t!(lang, "Cancel", "取消")).clicked() {
                            self.show_folder_dialog = false;
                        }
                    });
                });
        });
        if should_save {
            if let Some(rid) = self.rename_folder_id {
                let _ = self.db.rename_folder(rid, &self.new_folder_name);
            } else {
                let _ = self.db.create_folder(&self.new_folder_name, self.new_folder_parent_id);
            }
            self.show_folder_dialog = false;
            self.rename_folder_id = None;
        }
    }
}
