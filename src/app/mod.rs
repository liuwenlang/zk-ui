mod actions;
mod dialogs;
mod detail;
mod icons;
mod respond;
mod sidebar;
mod tree;
mod types;

use std::collections::HashMap;

use eframe::egui;
use egui::epaint::text::{FontData, FontDefinitions, FontFamily};

use crate::config::Cli;
use crate::db::{ConnProfile, LocalDb};
use crate::zk::{AclEntry, CreateMode, ZkManager};

pub(crate) use types::*;

macro_rules! t {
    ($lang:expr, $en:expr, $zh:expr) => {
        match $lang {
            $crate::app::Lang::En => $en,
            $crate::app::Lang::Zh => $zh,
        }
    };
}
pub(crate) use t;

pub struct ZkApp {
    pub(crate) zk_manager: ZkManager,
    pub(crate) pending: Pending,

    pub(crate) connect_state: ConnectState,

    pub(crate) tree_nodes: HashMap<String, TreeNode>,
    pub(crate) selected_path: Option<String>,
    pub(crate) selected_folder_id: Option<i64>,

    pub(crate) detail: Option<NodeDetail>,
    pub(crate) active_tab: Tab,

    pub(crate) edit_data: String,
    pub(crate) editing_data: bool,

    pub(crate) show_create_dialog: bool,
    pub(crate) create_name: String,
    pub(crate) create_data: String,
    pub(crate) create_mode: CreateMode,

    pub(crate) edit_acl: Vec<AclEntry>,
    pub(crate) editing_acl: bool,
    pub(crate) new_acl_scheme: String,
    pub(crate) new_acl_id: String,
    pub(crate) new_acl_perms: u32,

    pub(crate) search_query: String,
    pub(crate) search_results: Vec<String>,
    pub(crate) search_in_progress: bool,
    pub(crate) search_generation: u64,
    pub(crate) search_pending_after: Option<f64>,

    pub(crate) status_message: String,
    pub(crate) error_message: Option<String>,

    pub(crate) confirm_delete: bool,
    pub(crate) confirm_clear_children: Option<String>,
    pub(crate) clear_children_target: Option<String>,
    pub(crate) lang: Lang,

    // Current connection parameters
    pub(crate) hosts: String,
    pub(crate) timeout_ms: i32,
    pub(crate) active_conn_name: String,

    // Local database
    pub(crate) db: LocalDb,

    // Connection manager UI
    pub(crate) edit_conn: Option<ConnProfile>,
    pub(crate) edit_conn_name: String,
    pub(crate) edit_conn_hosts: String,
    pub(crate) edit_conn_timeout: i32,
    pub(crate) edit_conn_auth_scheme: String,
    pub(crate) edit_conn_auth_credential: String,
    pub(crate) edit_conn_folder_id: Option<i64>,
    pub(crate) show_conn_dialog: bool,

    // Folder management
    pub(crate) folder_expanded: std::collections::HashSet<i64>,
    pub(crate) show_folder_dialog: bool,
    pub(crate) new_folder_name: String,
    pub(crate) new_folder_parent_id: Option<i64>,
    pub(crate) rename_folder_id: Option<i64>,

    // Drag and drop
    pub(crate) drag_state: Option<DragItem>,
    pub(crate) active_conn_id: Option<i64>,
}

impl ZkApp {
    pub fn new(cc: &eframe::CreationContext<'_>, _config: Cli) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(13.0, egui::FontFamily::Monospace),
        );
        cc.egui_ctx.set_style(style);
        load_cjk_font(&cc.egui_ctx);

        Self {
            zk_manager: ZkManager::new(),
            pending: Pending::default(),

            connect_state: ConnectState::Disconnected,

            tree_nodes: HashMap::new(),
            selected_path: None,
            selected_folder_id: None,

            detail: None,
            active_tab: Tab::Data,

            edit_data: String::new(),
            editing_data: false,

            show_create_dialog: false,
            create_name: String::new(),
            create_data: String::new(),
            create_mode: CreateMode::Persistent,

            edit_acl: vec![],
            editing_acl: false,
            new_acl_scheme: "world".into(),
            new_acl_id: "anyone".into(),
            new_acl_perms: 1,

            search_query: String::new(),
            search_results: Vec::new(),
            search_in_progress: false,
            search_generation: 0,
            search_pending_after: None,

            status_message: String::new(),
            error_message: None,

            confirm_delete: false,
            confirm_clear_children: None,
            clear_children_target: None,
            lang: Lang::Zh,

            hosts: "127.0.0.1:2181".into(),
            timeout_ms: 5000,
            active_conn_name: String::new(),

            db: LocalDb::new().expect("Failed to open local database"),

            edit_conn: None,
            edit_conn_name: String::new(),
            edit_conn_hosts: "127.0.0.1:2181".into(),
            edit_conn_timeout: 5000,
            edit_conn_auth_scheme: "digest".into(),
            edit_conn_auth_credential: String::new(),
            edit_conn_folder_id: None,
            show_conn_dialog: false,

            folder_expanded: std::collections::HashSet::new(),
            show_folder_dialog: false,
            new_folder_name: String::new(),
            new_folder_parent_id: None,
            rename_folder_id: None,

            drag_state: None,
            active_conn_id: None,
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.horizontal(|ui| {
            if matches!(self.connect_state, ConnectState::Connected { .. }) {
                if ui.button(t!(lang, "R Refresh", "R 刷新")).clicked() {
                    if let Some(path) = self.selected_path.clone() {
                        self.load_children(&path);
                        self.load_node_detail(&path);
                    } else {
                        self.load_children("/");
                    }
                }
                ui.separator();
                if ui.button(t!(lang, "Disconnect", "断开")).clicked() {
                    self.do_disconnect();
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(self.lang.label()).clicked() {
                    self.lang = self.lang.toggle();
                }
                if !self.status_message.is_empty() {
                    ui.label(egui::RichText::new(&self.status_message).weak().size(12.0));
                }
            });
        });
    }
}

impl eframe::App for ZkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_responses(ctx);

        if let Some(deadline) = self.search_pending_after {
            let now = ctx.input(|i| i.time);
            if now >= deadline {
                self.search_pending_after = None;
                self.start_tree_search();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }

        // ── Top panel: toolbar ──
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });

        // ── Bottom: errors ──
        if self.error_message.is_some() {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                if let Some(err) = &self.error_message.clone() {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, format!("! {}", err));
                        if ui.small_button("X").clicked() {
                            self.error_message = None;
                        }
                    });
                }
            });
        }

        // ── Left sidebar: resource manager ──
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(280.0)
            .min_width(180.0)
            .max_width(480.0)
            .show(ctx, |ui| {
                let w = ui.available_width();
                ui.set_max_width(w);
                self.sidebar_panel(ui);
            });

        // ── Central panel ──
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_conn_dialog {
                self.show_conn_dialog_inline(ui);
            } else if self.show_folder_dialog {
                self.show_folder_dialog_inline(ui);
            } else {
                self.node_detail_panel(ui);
            }
        });
    }
}

fn load_cjk_font(ctx: &egui::Context) {
    let font_paths: &[&str] = &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];

    for path in font_paths {
        if let Ok(data) = std::fs::read(path) {
            tracing::info!("Loaded CJK font from {}", path);
            let mut fonts = FontDefinitions::default();
            let font_name = "CJK";
            fonts.font_data.insert(font_name.to_owned(), FontData::from_owned(data));
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                if let Some(family_list) = fonts.families.get_mut(&family) {
                    family_list.push(font_name.to_owned());
                }
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
    tracing::warn!("No CJK font found on this system — Chinese characters may display as boxes");
}
