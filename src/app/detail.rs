use eframe::egui;

use crate::zk::{AclEntry, CreateMode, perm_string};

use super::t;
use super::types::{ConnectState, NodeDetail, Tab};
use super::ZkApp;

impl ZkApp {
    pub(crate) fn node_detail_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        if let Some(path) = &self.selected_path.clone() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(path).strong().size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(t!(lang, "Copy Path", "复制路径")).clicked() {
                        ui.output_mut(|o| o.copied_text = path.clone());
                    }
                });
            });

            if self.confirm_delete {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::RED, t!(lang, "Confirm delete?", "确认删除?"));
                    if ui.small_button(t!(lang, "Yes", "是")).clicked() {
                        self.do_delete_node();
                    }
                    if ui.small_button(t!(lang, "No", "否")).clicked() {
                        self.confirm_delete = false;
                    }
                });
            }

            if self.confirm_clear_children.as_deref() == Some(path.as_str()) {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 140, 50),
                        t!(lang, "Clear all child nodes?", "确认清空所有子节点?"),
                    );
                    if ui.small_button(t!(lang, "Yes", "是")).clicked() {
                        self.do_clear_children();
                    }
                    if ui.small_button(t!(lang, "No", "否")).clicked() {
                        self.confirm_clear_children = None;
                    }
                });
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Data, t!(lang, "Properties", "属性"));
                ui.selectable_value(&mut self.active_tab, Tab::Acl, "ACL");
                ui.selectable_value(&mut self.active_tab, Tab::Stat, t!(lang, "Statistics", "统计"));
            });

            ui.separator();

            if let Some(detail) = self.detail.clone() {
                match self.active_tab {
                    Tab::Data => self.data_tab(ui, &detail),
                    Tab::Acl => self.acl_tab(ui, &detail),
                    Tab::Stat => self.stat_tab(ui, &detail),
                }
            }
        } else if matches!(self.connect_state, ConnectState::Connected { .. }) {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(egui::RichText::new(t!(lang, "Select a node from the explorer", "从浏览器中选择一个节点")).size(15.0).weak());
                ui.add_space(8.0);
                ui.label(egui::RichText::new(t!(lang, "Right-click for context menu", "右键点击显示上下文菜单")).size(12.0).weak());
            });
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(egui::RichText::new(t!(lang, "Connect to a ZooKeeper instance", "连接到 ZooKeeper 实例")).size(15.0).weak());
                ui.add_space(8.0);
                ui.label(egui::RichText::new(t!(lang, "Click + in the sidebar to add a connection", "点击侧边栏 + 添加连接")).size(12.0).weak());
            });
        }

        if self.show_create_dialog {
            ui.separator();
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.set_min_width(300.0);
                        ui.label(egui::RichText::new(t!(lang, "Create Node", "创建节点")).strong().size(16.0));
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(t!(lang, "Name:", "名称:"));
                            ui.text_edit_singleline(&mut self.create_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label(t!(lang, "Data:", "数据:"));
                            ui.text_edit_singleline(&mut self.create_data);
                        });
                        ui.horizontal(|ui| {
                            ui.label(t!(lang, "Mode:", "模式:"));
                            egui::ComboBox::from_id_salt("create_mode")
                                .selected_text(self.create_mode.label())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.create_mode, CreateMode::Persistent, t!(lang, "Persistent", "持久"));
                                    ui.selectable_value(&mut self.create_mode, CreateMode::Ephemeral, t!(lang, "Ephemeral", "临时"));
                                    ui.selectable_value(&mut self.create_mode, CreateMode::PersistentSequential, t!(lang, "Persistent Sequential", "持久顺序"));
                                    ui.selectable_value(&mut self.create_mode, CreateMode::EphemeralSequential, t!(lang, "Ephemeral Sequential", "临时顺序"));
                                });
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui.button(t!(lang, "Create", "创建")).clicked() {
                                self.do_create_node();
                            }
                            if ui.button(t!(lang, "Cancel", "取消")).clicked() {
                                self.show_create_dialog = false;
                            }
                        });
                    });
            });
        }
    }

    pub(crate) fn detail_node_action_buttons(&mut self, ui: &mut egui::Ui, path: &str) {
        let lang = self.lang;
        if !matches!(self.connect_state, ConnectState::Connected { .. }) {
            return;
        }
        ui.separator();
        if ui
            .button(t!(lang, "+ New", "+ 新建"))
            .on_hover_text(t!(lang, "Create child node", "创建子节点"))
            .clicked()
        {
            self.show_create_dialog = true;
        }
        if path != "/" {
            if ui.button(t!(lang, "X Delete", "X 删除")).clicked() {
                self.confirm_delete = true;
            }
        }
    }

    pub(crate) fn data_tab(&mut self, ui: &mut egui::Ui, detail: &NodeDetail) {
        let lang = self.lang;
        ui.horizontal(|ui| {
            ui.label(format!("{} {} bytes", t!(lang, "Size:", "大小:"), detail.data_raw.len()));
            ui.separator();
            ui.label(format!("{} {}", t!(lang, "Version:", "版本:"), detail.stat.version));
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if self.editing_data {
                if ui.button(t!(lang, "Save", "保存")).clicked() {
                    self.do_save_data();
                }
                if ui.button(t!(lang, "Cancel", "取消")).clicked() {
                    self.edit_data = detail.data.clone();
                    self.editing_data = false;
                }
            } else {
                if ui.button(t!(lang, "Edit", "编辑")).clicked() {
                    self.edit_data = detail.data.clone();
                    self.editing_data = true;
                }
            }
            self.detail_node_action_buttons(ui, &detail.path);
        });

        if self.editing_data {
            ui.add(
                egui::TextEdit::multiline(&mut self.edit_data)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(15),
            );
        } else {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&detail.data)
                                .monospace()
                                .size(13.0),
                        ).wrap(),
                    );
                });
        }
    }

    pub(crate) fn acl_tab(&mut self, ui: &mut egui::Ui, detail: &NodeDetail) {
        let lang = self.lang;
        ui.horizontal(|ui| {
            if self.editing_acl {
                if ui.button(t!(lang, "Save", "保存")).clicked() {
                    self.do_save_acl();
                }
                if ui.button(t!(lang, "Cancel", "取消")).clicked() {
                    self.edit_acl = detail.acl.clone();
                    self.editing_acl = false;
                }
            } else {
                if ui.button(t!(lang, "Edit", "编辑")).clicked() {
                    self.edit_acl = detail.acl.clone();
                    self.editing_acl = true;
                }
            }
            self.detail_node_action_buttons(ui, &detail.path);
        });

        let acl = if self.editing_acl { &mut self.edit_acl } else { &mut self.edit_acl.clone() };

        let mut to_remove = None;
        for (i, entry) in acl.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("[{}:{}] {}", entry.scheme, entry.id, perm_string(entry.perms)));
                if self.editing_acl {
                    if ui.small_button("x").clicked() {
                        to_remove = Some(i);
                    }
                }
            });
        }
        if let Some(i) = to_remove {
            if self.editing_acl {
                self.edit_acl.remove(i);
            }
        }

        if self.editing_acl {
            ui.separator();
            ui.label(t!(lang, "Add ACL entry:", "添加 ACL 条目:"));
            ui.horizontal(|ui| {
                ui.label(t!(lang, "Scheme:", "方案:"));
                ui.text_edit_singleline(&mut self.new_acl_scheme);
                ui.label("ID:");
                ui.text_edit_singleline(&mut self.new_acl_id);
            });
            ui.horizontal(|ui| {
                let mut r = self.new_acl_perms & 1 != 0;
                let mut w = self.new_acl_perms & 2 != 0;
                let mut c = self.new_acl_perms & 4 != 0;
                let mut d = self.new_acl_perms & 8 != 0;
                let mut a = self.new_acl_perms & 16 != 0;
                ui.checkbox(&mut r, t!(lang, "Read", "读取"));
                ui.checkbox(&mut w, t!(lang, "Write", "写入"));
                ui.checkbox(&mut c, t!(lang, "Create", "创建"));
                ui.checkbox(&mut d, t!(lang, "Delete", "删除"));
                ui.checkbox(&mut a, t!(lang, "Admin", "管理"));
                self.new_acl_perms = 0;
                if r { self.new_acl_perms |= 1; }
                if w { self.new_acl_perms |= 2; }
                if c { self.new_acl_perms |= 4; }
                if d { self.new_acl_perms |= 8; }
                if a { self.new_acl_perms |= 16; }
            });
            if ui.button(t!(lang, "Add", "添加")).clicked() {
                self.edit_acl.push(AclEntry {
                    scheme: self.new_acl_scheme.clone(),
                    id: self.new_acl_id.clone(),
                    perms: self.new_acl_perms,
                });
                self.new_acl_scheme = "world".into();
                self.new_acl_id = "anyone".into();
                self.new_acl_perms = 1;
            }
        }
    }

    pub(crate) fn stat_tab(&mut self, ui: &mut egui::Ui, detail: &NodeDetail) {
        self.detail_node_action_buttons(ui, &detail.path);
        ui.add_space(4.0);
        let s = &detail.stat;
        egui::Grid::new("stat_grid")
            .num_columns(2)
            .spacing([20.0, 4.0])
            .show(ui, |ui| {
                ui.label("czxid:"); ui.label(format!("0x{:x}", s.czxid)); ui.end_row();
                ui.label("mzxid:"); ui.label(format!("0x{:x}", s.mzxid)); ui.end_row();
                ui.label("ctime:"); ui.label(super::types::format_timestamp(s.ctime)); ui.end_row();
                ui.label("mtime:"); ui.label(super::types::format_timestamp(s.mtime)); ui.end_row();
                ui.label("version:"); ui.label(format!("{}", s.version)); ui.end_row();
                ui.label("cversion:"); ui.label(format!("{}", s.cversion)); ui.end_row();
                ui.label("aversion:"); ui.label(format!("{}", s.aversion)); ui.end_row();
                ui.label("ephemeralOwner:"); ui.label(format!("0x{:x}", s.ephemeral_owner)); ui.end_row();
                ui.label("dataLength:"); ui.label(format!("{}", s.data_length)); ui.end_row();
                ui.label("numChildren:"); ui.label(format!("{}", s.num_children)); ui.end_row();
                ui.label("pzxid:"); ui.label(format!("0x{:x}", s.pzxid)); ui.end_row();
            });
    }
}
