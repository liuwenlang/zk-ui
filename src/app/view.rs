use gpui_fps::fps_monitor;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::description_list::DescriptionList;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    h_flex, h_resizable, resizable_panel, v_flex, v_virtual_list, ActiveTheme, Disableable,
    IconName, Selectable, Sizable, TitleBar, WindowExt,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, ClipboardItem, Context, InteractiveElement, IntoElement, ParentElement,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::db::{ConnProfile, Folder};
use crate::zk::{perm_string, CreateMode};

use super::i18n::Lang;
use super::session::{self, format_timestamp, ConnectState, InspectorTab};
use super::tree_model::{FlatRow, RowKind, SEARCH_MAX_RESULTS};
use super::ZkApp;

impl ZkApp {
    pub(crate) fn render_workspace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.flush_side_effects(window, cx);

        let sidebar = self.sidebar(cx);
        let browser = self.browser(cx);
        let inspector = self.inspector(window, cx);
        let title = self.title_bar(cx);
        let status = self.status_bar(cx);
        let show_fps = self.show_fps;
        let theme = cx.theme().clone();

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .child(title)
            .child(
                div().flex_1().min_h(px(0.)).w_full().child(
                    h_resizable("workspace")
                        .child(
                            resizable_panel()
                                .size(px(280.))
                                .size_range(px(220.)..px(420.))
                                .child(sidebar),
                        )
                        .child(resizable_panel().child(browser))
                        .child(
                            resizable_panel()
                                .size(px(420.))
                                .size_range(px(320.)..px(680.))
                                .child(inspector),
                        ),
                ),
            )
            .child(status)
            .when(show_fps, |this| this.child(fps_monitor(window, cx)))
    }

    fn flush_side_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync_editor {
            self.sync_editor = false;
            let text = self.edit_data.clone();
            self.data_editor.update(cx, |state, cx| {
                state.set_value(text, window, cx);
            });
        }
        if let Some(message) = self.toast.take() {
            cx.defer_in(window, move |_, window, cx| {
                window.push_notification(Notification::error(message), cx);
            });
        }
        if self.export_ready {
            self.export_ready = false;
            let text = self.export_text.clone();
            cx.defer_in(window, move |_, window, cx| {
                window.open_dialog(cx, move |dialog, _, _| {
                    dialog.title("Export JSON").child(
                        div()
                            .w(px(560.))
                            .max_h(px(420.))
                            .id("export-scroll")
                            .overflow_scroll()
                            .p_3()
                            .font_family("DejaVu Sans Mono")
                            .text_size(px(12.))
                            .child(text.clone()),
                    )
                });
            });
        }
    }

    fn t(&self, en: &'static str, zh: &'static str) -> SharedString {
        self.lang.tr(en, zh)
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.connect_state == ConnectState::Connected;
        let host = if connected {
            self.hosts.clone()
        } else {
            self.t("Not connected", "未连接").to_string()
        };
        let lang = self.lang;
        TitleBar::new().child(
            h_flex()
                .w_full()
                .gap_2()
                .px_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child("zk-ui"),
                )
                .child(
                    div()
                        .px_2()
                        .py(px(2.))
                        .rounded(px(99.))
                        .text_xs()
                        .bg(if connected {
                            cx.theme().success.opacity(0.16)
                        } else {
                            cx.theme().muted
                        })
                        .text_color(if connected {
                            cx.theme().success
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(host),
                )
                .child(div().flex_1())
                .child(
                    Button::new("lang")
                        .ghost()
                        .small()
                        .label(if lang == Lang::Zh { "中文" } else { "EN" })
                        .tooltip(self.t("Switch language", "切换语言"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.lang = this.lang.toggle();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("theme")
                        .ghost()
                        .small()
                        .icon(if self.theme_dark {
                            IconName::Sun
                        } else {
                            IconName::Moon
                        })
                        .tooltip(self.t("Toggle theme", "切换主题"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.theme_dark = !this.theme_dark;
                            let mode = if this.theme_dark {
                                gpui_kit::component::ThemeMode::Dark
                            } else {
                                gpui_kit::component::ThemeMode::Light
                            };
                            gpui_kit::component::Theme::change(mode, Some(window), cx);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("fps")
                        .ghost()
                        .small()
                        .label("FPS")
                        .selected(self.show_fps)
                        .tooltip(self.t(
                            "Frame-time HUD. Release builds follow the display refresh.",
                            "帧时间指示。Release 构建会跟随显示器刷新率。",
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_fps = !this.show_fps;
                            cx.notify();
                        })),
                ),
        )
    }

    fn sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex().gap(px(2.)).p_2();
        let folders = self.root_folders.clone();
        let connections = self.root_connections.clone();
        for folder in folders {
            list = list.child(self.folder_block(&folder, 0, cx));
        }
        for conn in connections {
            list = list.child(self.connection_row(&conn, cx));
        }

        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .child(self.sidebar_header(cx))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .id("sidebar-scroll")
                    .overflow_scroll()
                    .child(list),
            )
    }

    fn sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let quick = self.cli_hosts.clone();
        h_flex()
            .h(px(40.))
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().sidebar_border)
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(self.t("Connections", "连接")),
            )
            .child(
                Button::new("quick-connect")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Network)
                    .tooltip(SharedString::from(format!(
                        "{} {quick}",
                        self.t("Quick connect", "快速连接")
                    )))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.connect_quick(quick.clone());
                        cx.notify();
                    })),
            )
            .child(
                Button::new("new-folder")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Folder)
                    .tooltip(self.t("New folder", "新建文件夹"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_folder_dialog(None, window, cx);
                    })),
            )
            .child(
                Button::new("new-conn")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .tooltip(self.t("New connection", "新建连接"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.conn_edit_id = None;
                        this.open_connection_dialog(window, cx);
                    })),
            )
    }

    fn folder_block(
        &self,
        folder: &Folder,
        depth: u32,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let expanded = self.expanded_folders.contains(&folder.id);
        let selected = self.selected_folder_id == Some(folder.id);
        let id = folder.id;
        let name = folder.name.clone();
        let (subs, conns) = self
            .folder_children
            .get(&folder.id)
            .cloned()
            .unwrap_or_default();
        let mut body = v_flex().gap(px(2.));
        if expanded {
            for sub in subs {
                body = body.child(self.folder_block(&sub, depth + 1, cx));
            }
            for conn in conns {
                body = body.child(self.connection_row(&conn, cx));
            }
        }
        v_flex()
            .child(
                h_flex()
                    .h(px(28.))
                    .pl(px(8. + depth as f32 * 14.))
                    .pr_1()
                    .gap_1()
                    .rounded(px(6.))
                    .bg(if selected {
                        cx.theme().sidebar_accent
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .child(
                        Button::new(format!("folder-{id}"))
                            .ghost()
                            .xsmall()
                            .icon(if expanded {
                                IconName::FolderOpen
                            } else {
                                IconName::Folder
                            })
                            .label(name.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_folder(id);
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(format!("folder-rename-{id}"))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Settings)
                            .tooltip(self.t("Rename folder", "重命名文件夹"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                set_input(&this.folder_name, name.clone(), window, cx);
                                this.open_folder_dialog(Some(id), window, cx);
                            })),
                    )
                    .child(
                        Button::new(format!("folder-del-{id}"))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Delete)
                            .tooltip(self.t("Delete folder", "删除文件夹"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.delete_folder(id);
                                cx.notify();
                            })),
                    ),
            )
            .child(body)
            .into_any_element()
    }

    fn connection_row(&self, conn: &ConnProfile, cx: &mut Context<Self>) -> impl IntoElement {
        let id = conn.id;
        let active = self.active_conn_id == Some(id);
        let name = conn.name.clone();
        let hosts = conn.hosts.clone();
        let profile = conn.clone();
        h_flex()
            .h(px(36.))
            .px_2()
            .gap_1()
            .rounded(px(6.))
            .bg(if active {
                cx.theme().sidebar_accent
            } else {
                gpui_kit::transparent_black()
            })
            .child(div().size(px(8.)).rounded(px(99.)).bg(if active {
                cx.theme().success
            } else {
                cx.theme().muted_foreground.opacity(0.4)
            }))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .id(format!("conn-{id}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.connect_profile(&profile);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(hosts),
                    ),
            )
            .child(
                Button::new(format!("conn-up-{id}"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::ArrowUp)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.move_connection(id, -1);
                        cx.notify();
                    })),
            )
            .child(
                Button::new(format!("conn-edit-{id}"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Settings)
                    .tooltip(self.t("Edit", "编辑"))
                    .on_click({
                        let conn = conn.clone();
                        cx.listener(move |this, _, window, cx| {
                            this.prepare_connection_edit(&conn, window, cx);
                        })
                    }),
            )
            .child(
                Button::new(format!("conn-del-{id}"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Delete)
                    .tooltip(self.t("Delete", "删除"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.confirm_delete_connection(id, window, cx);
                    })),
            )
    }

    fn browser(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.connect_state == ConnectState::Connected;
        let searching = !self.search_query.is_empty();
        let body = if !connected {
            self.empty_state(
                self.t("Pick a connection", "选择一个连接"),
                self.t(
                    "Saved clusters live on the left, the same way Redis Insight and PrettyZoo keep sessions out of the tree.",
                    "已保存的集群在左侧，交互上接近 Redis Insight 和 PrettyZoo：连接和节点树分开。",
                ),
            )
        } else if searching {
            self.search_list(cx)
        } else if self.rows.is_empty() {
            self.empty_state(
                self.t("Loading /", "正在加载 /"),
                self.t(
                    "The root node appears as soon as the session answers.",
                    "会话返回后会显示根节点。",
                ),
            )
        } else {
            self.tree_list(cx)
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.browser_toolbar(cx))
            .child(div().flex_1().min_h(px(0.)).child(body))
    }

    fn browser_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.connect_state == ConnectState::Connected;
        let path = self.selected_path.clone().unwrap_or_else(|| "/".into());
        h_flex()
            .h(px(44.))
            .px_2()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().w(px(220.)).child(Input::new(&self.search_input)))
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(path.clone()),
            )
            .child(
                Button::new("refresh")
                    .ghost()
                    .small()
                    .icon(IconName::RefreshCw)
                    .tooltip(self.t("Refresh", "刷新"))
                    .disabled(!connected)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(path) = this.selected_path.clone() {
                            this.load_children(&path);
                            this.load_node_detail(&path);
                        } else {
                            this.load_children("/");
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("create-node")
                    .small()
                    .icon(IconName::Plus)
                    .label(self.t("New", "新建"))
                    .disabled(!connected)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_create_dialog(window, cx)),
                    ),
            )
            .child(
                Button::new("copy-path")
                    .ghost()
                    .small()
                    .icon(IconName::Copy)
                    .tooltip(self.t("Copy path", "复制路径"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(path) = this.selected_path.clone() {
                            cx.write_to_clipboard(ClipboardItem::new_string(path));
                            this.note("Path copied");
                            cx.notify();
                        }
                    })),
            )
            .child(
                Button::new("export")
                    .ghost()
                    .small()
                    .icon(IconName::FileText)
                    .tooltip(self.t("Export subtree", "导出子树"))
                    .disabled(!connected)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.export_selected();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("import")
                    .ghost()
                    .small()
                    .label(self.t("Import", "导入"))
                    .tooltip(self.t(
                        "Import a subtree from export JSON",
                        "从导出的 JSON 导入子树",
                    ))
                    .disabled(!connected)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_import_dialog(window, cx)),
                    ),
            )
            .child(
                Button::new("clear-children")
                    .ghost()
                    .small()
                    .icon(IconName::Minus)
                    .tooltip(self.t("Delete children", "清空子节点"))
                    .disabled(!connected)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.confirm_clear_children(window, cx)),
                    ),
            )
            .child(
                Button::new("delete-node")
                    .ghost()
                    .small()
                    .icon(IconName::Delete)
                    .tooltip(self.t("Delete node", "删除节点"))
                    .disabled(!connected || self.selected_path.as_deref() == Some("/"))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.confirm_delete_node(window, cx)),
                    ),
            )
    }

    fn tree_list(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if let Some(index) = self.scroll_to.take() {
            self.tree_scroll
                .scroll_to_item(index, ScrollStrategy::Center);
        }
        let sizes = self.row_sizes.clone();
        let view = cx.entity().clone();
        div()
            .size_full()
            .child(
                v_virtual_list(view, "znode-tree", sizes, |this, range, _, cx| {
                    range
                        .filter_map(|index| this.rows.get(index).cloned().map(|row| (index, row)))
                        .map(|(index, row)| this.tree_row(index, &row, cx))
                        .collect()
                })
                .track_scroll(&self.tree_scroll),
            )
            .into_any_element()
    }

    fn tree_row(
        &self,
        index: usize,
        row: &FlatRow,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        match &row.kind {
            RowKind::Loading => self.note_row(
                row.depth,
                self.t("Loading children…", "正在加载子节点…"),
                cx,
            ),
            RowKind::More {
                parent,
                shown,
                total,
            } => self.more_row(index, row.depth, parent, *shown, *total, cx),
            RowKind::Node {
                expandable,
                expanded,
                child_count,
            } => self.node_row(index, row, *expandable, *expanded, *child_count, cx),
        }
    }

    fn node_row(
        &self,
        index: usize,
        row: &FlatRow,
        expandable: bool,
        expanded: bool,
        child_count: Option<usize>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let selected = self.selected_path.as_deref() == Some(row.path.as_str());
        let path = row.path.clone();
        h_flex()
            .id(format!("zrow-{index}"))
            .h(px(28.))
            .w_full()
            .pl(px(8. + row.depth as f32 * 16.))
            .pr_2()
            .gap_1()
            .bg(if selected {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .text_color(if selected {
                cx.theme().accent_foreground
            } else {
                cx.theme().foreground
            })
            .hover(|style| style.bg(cx.theme().muted.opacity(if selected { 1. } else { 0.7 })))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_node(&path);
                cx.notify();
            }))
            .child(if expandable {
                let path = row.path.clone();
                Button::new(format!("expand-{index}"))
                    .ghost()
                    .xsmall()
                    .icon(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_expand(&path);
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                div().w(px(22.)).into_any_element()
            })
            .child(IconName::if_expandable(expandable, expanded).into_any_element())
            .child(div().text_sm().child(row.name.clone()))
            .when(child_count.unwrap_or(0) > 0, |this| {
                this.child(
                    div()
                        .text_xs()
                        .opacity(0.7)
                        .child(child_count.unwrap_or(0).to_string()),
                )
            })
            .into_any_element()
    }

    fn note_row(
        &self,
        depth: u32,
        text: SharedString,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        div()
            .h(px(28.))
            .pl(px(32. + depth as f32 * 16.))
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(text)
            .into_any_element()
    }

    fn more_row(
        &self,
        index: usize,
        depth: u32,
        parent: &str,
        shown: usize,
        total: usize,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let parent_more = parent.to_string();
        let parent_all = parent.to_string();
        h_flex()
            .id(format!("more-{index}"))
            .h(px(28.))
            .pl(px(32. + depth as f32 * 16.))
            .pr_2()
            .gap_2()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!("{shown} / {total}"))
            .child(
                Button::new(format!("more-next-{index}"))
                    .ghost()
                    .xsmall()
                    .label(self.t("Load more", "继续加载"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show_more(&parent_more);
                        cx.notify();
                    })),
            )
            .child(
                Button::new(format!("more-all-{index}"))
                    .ghost()
                    .xsmall()
                    .label(self.t("Show all", "全部显示"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show_all(&parent_all);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn search_list(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let capped = self.search_results.len() >= SEARCH_MAX_RESULTS;
        let progress = if self.search_in_progress {
            self.t("Searching…", "搜索中…")
        } else if capped {
            self.t("Result cap reached", "已到结果上限")
        } else {
            self.t("Search finished", "搜索结束")
        };
        let summary = format!(
            "{} {} · {} {}",
            self.search_results.len(),
            self.t("hits", "条"),
            self.search_scanned,
            progress
        );
        let mut column = v_flex().size_full().child(
            div()
                .h(px(28.))
                .px_3()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(summary),
        );
        if self.search_results.is_empty() {
            column = column.child(self.note_row(
                0,
                if self.search_in_progress {
                    self.t("Searching the cluster…", "正在搜索集群…")
                } else {
                    self.t("No matching znodes", "没有匹配的节点")
                },
                cx,
            ));
        } else {
            let sizes = self.search_sizes.clone();
            let view = cx.entity().clone();
            column = column.child(div().flex_1().min_h(px(0.)).child(v_virtual_list(
                view,
                "search-hits",
                sizes,
                |this, range, _, cx| {
                    range
                        .filter_map(|index| {
                            this.search_results
                                .get(index)
                                .cloned()
                                .map(|path| (index, path))
                        })
                        .map(|(index, path)| this.search_hit(index, path, cx))
                        .collect()
                },
            )));
        }
        column.into_any_element()
    }

    fn search_hit(&self, index: usize, path: String, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_path.as_deref() == Some(path.as_str());
        let target = path.clone();
        h_flex()
            .id(format!("search-{index}"))
            .h(px(28.))
            .px_2()
            .bg(if selected {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.reveal_path(&target);
                this.leave_search();
                set_input(&this.search_input, String::new(), window, cx);
                cx.notify();
            }))
            .child(div().text_sm().child(path))
    }

    fn inspector(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.inspector;
        let connected = self.connect_state == ConnectState::Connected;
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .border_l_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .px_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        TabBar::new("inspector-tabs")
                            .selected_index(match tab {
                                InspectorTab::Data => 0,
                                InspectorTab::Acl => 1,
                                InspectorTab::Stat => 2,
                                InspectorTab::Server => 3,
                            })
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.inspector = match index {
                                    1 => InspectorTab::Acl,
                                    2 => InspectorTab::Stat,
                                    3 => InspectorTab::Server,
                                    _ => InspectorTab::Data,
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label(self.t("Data", "数据")))
                            .child(Tab::new().label("ACL"))
                            .child(Tab::new().label(self.t("Stat", "统计")))
                            .child(Tab::new().label(self.t("Server", "服务器"))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .id("inspector-scroll")
                    .overflow_scroll()
                    .p_3()
                    .child(match tab {
                        InspectorTab::Data => self.data_tab(cx),
                        InspectorTab::Acl => self.acl_tab(cx),
                        InspectorTab::Stat => self.stat_tab(cx),
                        InspectorTab::Server => self.server_tab(connected, cx),
                    }),
            )
    }

    fn data_tab(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(detail) = self.detail.clone() else {
            return self
                .empty_state(
                    self.t("No node selected", "未选择节点"),
                    self.t(
                        "Select a znode to read and edit its data.",
                        "选择一个 znode 查看并编辑数据。",
                    ),
                )
                .into_any_element();
        };
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(detail.path.clone()),
                    )
                    .child(div().flex_1())
                    .when(detail.binary, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().warning)
                                .child(self.t("Binary payload", "二进制数据")),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} B", detail.stat.data_length)),
                    )
                    .child(
                        Button::new("save-data")
                            .primary()
                            .small()
                            .label(self.t("Save", "保存"))
                            .disabled(!self.data_dirty)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_data();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                Textarea::new(&self.data_editor)
                    .h(px(280.))
                    .font_family(cx.theme().mono_font_family.clone()),
            )
            .into_any_element()
    }

    fn acl_tab(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(_) = self.detail.as_ref() else {
            return self
                .empty_state(
                    self.t("No node selected", "未选择节点"),
                    self.t(
                        "ACL appears after a node is selected.",
                        "选中节点后显示 ACL。",
                    ),
                )
                .into_any_element();
        };
        let mut rows = v_flex().gap_2();
        let entries = self.edit_acl.clone();
        for (index, entry) in entries.iter().enumerate() {
            rows = rows.child(self.acl_row(index, entry, cx));
        }
        v_flex()
            .gap_3()
            .child(rows)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(110.)).child(Input::new(&self.acl_scheme)))
                    .child(div().flex_1().child(Input::new(&self.acl_id)))
                    .child(
                        Button::new("add-acl")
                            .small()
                            .label(self.t("Add", "添加"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let scheme = this.acl_scheme.read(cx).value().to_string();
                                let id = this.acl_id.read(cx).value().to_string();
                                if scheme.is_empty() || id.is_empty() {
                                    return;
                                }
                                this.edit_acl.push(crate::zk::AclEntry {
                                    scheme,
                                    id,
                                    perms: 31,
                                });
                                this.acl_dirty = true;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex().child(
                    Button::new("save-acl")
                        .primary()
                        .small()
                        .label(self.t("Save ACL", "保存 ACL"))
                        .disabled(!self.acl_dirty)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.save_acl();
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }

    fn acl_row(
        &self,
        index: usize,
        entry: &crate::zk::AclEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .gap_2()
            .child(
                div()
                    .w(px(160.))
                    .text_sm()
                    .child(format!("{}:{}", entry.scheme, entry.id)),
            )
            .child(div().text_xs().child(perm_string(entry.perms)))
            .children(perm_checks(index, entry.perms, cx))
            .child(
                Button::new(format!("acl-del-{index}"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Minus)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if index < this.edit_acl.len() {
                            this.edit_acl.remove(index);
                            this.acl_dirty = true;
                        }
                        cx.notify();
                    })),
            )
    }

    fn stat_tab(&self, _cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(detail) = self.detail.clone() else {
            return self
                .empty_state(
                    self.t("No node selected", "未选择节点"),
                    self.t("Statistics load with the node.", "统计信息随节点一起加载。"),
                )
                .into_any_element();
        };
        let stat = detail.stat;
        DescriptionList::new()
            .columns(1)
            .bordered(true)
            .item("path", detail.path, 1)
            .item("czxid", stat.czxid.to_string(), 1)
            .item("mzxid", stat.mzxid.to_string(), 1)
            .item("ctime", format_timestamp(stat.ctime), 1)
            .item("mtime", format_timestamp(stat.mtime), 1)
            .item("version", stat.version.to_string(), 1)
            .item("cversion", stat.cversion.to_string(), 1)
            .item("aversion", stat.aversion.to_string(), 1)
            .item("ephemeralOwner", stat.ephemeral_owner.to_string(), 1)
            .item("dataLength", stat.data_length.to_string(), 1)
            .item("numChildren", stat.num_children.to_string(), 1)
            .item("pzxid", stat.pzxid.to_string(), 1)
            .into_any_element()
    }

    fn server_tab(&self, connected: bool, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.t(
                        "Four-letter words, the same server console PrettyZoo exposes.",
                        "四字命令，和 PrettyZoo 的服务器控制台同一类操作。",
                    )),
            )
            .child(
                h_flex().gap_2().children(
                    ["stat", "srvr", "mntr", "conf", "envi"]
                        .into_iter()
                        .map(|cmd| {
                            let command = cmd.to_string();
                            Button::new(format!("four-letter-{cmd}"))
                                .small()
                                .outline()
                                .label(cmd)
                                .disabled(!connected)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.run_four_letter(&command);
                                    cx.notify();
                                }))
                        }),
                ),
            )
            .child(
                div()
                    .w_full()
                    .min_h(px(240.))
                    .p_3()
                    .rounded(px(8.))
                    .bg(cx.theme().muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(12.))
                    .child(if self.server_output.is_empty() {
                        self.t("No command yet", "还没有执行命令").to_string()
                    } else {
                        self.server_output.clone()
                    }),
            )
            .into_any_element()
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = match self.connect_state {
            ConnectState::Disconnected => self.t("disconnected", "未连接"),
            ConnectState::Connecting => self.t("connecting", "连接中"),
            ConnectState::Connected => self.t("connected", "已连接"),
        };
        h_flex()
            .h(px(28.))
            .px_3()
            .gap_3()
            .text_xs()
            .bg(cx.theme().muted)
            .text_color(cx.theme().muted_foreground)
            .border_t_1()
            .border_color(cx.theme().border)
            .child(state)
            .child(self.active_conn_name.clone())
            .child(div().flex_1().child(self.status_message.clone()))
            .child(format!(
                "{} {} · {} {}",
                self.rows.len(),
                self.t("rows", "行"),
                self.tree.loaded_names(),
                self.t("names", "名称")
            ))
            .child(self.t("GPU · display refresh", "GPU · 跟随显示器刷新率"))
    }

    fn empty_state(&self, title: SharedString, body: SharedString) -> gpui_kit::AnyElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .p_6()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .max_w(px(360.))
                    .text_center()
                    .text_sm()
                    .text_color(gpui_kit::hsla(0., 0., 0.45, 1.))
                    .child(body),
            )
            .into_any_element()
    }

    fn prepare_connection_edit(
        &mut self,
        conn: &ConnProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.conn_edit_id = Some(conn.id);
        self.selected_folder_id = conn.folder_id;
        set_input(&self.conn_name, conn.name.clone(), window, cx);
        set_input(&self.conn_hosts, conn.hosts.clone(), window, cx);
        set_input(&self.conn_timeout, conn.timeout_ms.to_string(), window, cx);
        set_input(&self.conn_scheme, conn.auth_scheme.clone(), window, cx);
        set_input(&self.conn_secret, conn.auth_credential.clone(), window, cx);
        self.open_connection_dialog(window, cx);
    }

    fn open_connection_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.conn_edit_id.is_none() {
            set_input(&self.conn_name, String::new(), window, cx);
            set_input(&self.conn_scheme, String::new(), window, cx);
            set_input(&self.conn_secret, String::new(), window, cx);
        }
        let name = self.conn_name.clone();
        let hosts = self.conn_hosts.clone();
        let timeout = self.conn_timeout.clone();
        let scheme = self.conn_scheme.clone();
        let secret = self.conn_secret.clone();
        let title = if self.conn_edit_id.is_some() {
            self.t("Edit connection", "编辑连接")
        } else {
            self.t("New connection", "新建连接")
        };
        let save = self.t("Save", "保存");
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            dialog.title(title.clone()).w(px(440.)).child(
                v_flex()
                    .gap_2()
                    .p_1()
                    .child(labeled(cx, "Name", Input::new(&name)))
                    .child(labeled(cx, "Hosts", Input::new(&hosts)))
                    .child(labeled(cx, "Timeout (ms)", Input::new(&timeout)))
                    .child(labeled(cx, "Auth scheme", Input::new(&scheme)))
                    .child(labeled(cx, "Auth credential", Input::new(&secret)))
                    .child(
                        h_flex().justify_end().child(
                            Button::new("save-connection")
                                .primary()
                                .label(save.clone())
                                .on_click(move |_, window, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        let timeout = this
                                            .conn_timeout
                                            .read(cx)
                                            .value()
                                            .parse()
                                            .unwrap_or(5000);
                                        this.save_connection(
                                            this.conn_edit_id,
                                            this.conn_name.read(cx).value().to_string(),
                                            this.conn_hosts.read(cx).value().to_string(),
                                            timeout,
                                            this.conn_scheme.read(cx).value().to_string(),
                                            this.conn_secret.read(cx).value().to_string(),
                                        );
                                        cx.notify();
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                    ),
            )
        });
    }

    fn open_folder_dialog(
        &mut self,
        edit_id: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.folder_edit_id = edit_id;
        if edit_id.is_none() {
            set_input(&self.folder_name, String::new(), window, cx);
        }
        let title = if edit_id.is_some() {
            self.t("Rename folder", "重命名文件夹")
        } else {
            self.t("New folder", "新建文件夹")
        };
        let folder = self.folder_name.clone();
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog.title(title.clone()).child(
                v_flex()
                    .gap_2()
                    .w(px(360.))
                    .child(Input::new(&folder))
                    .child(Button::new("save-folder").primary().label("OK").on_click(
                        move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                let name = this.folder_name.read(cx).value().to_string();
                                if let Some(id) = this.folder_edit_id {
                                    this.rename_folder(id, name);
                                } else {
                                    this.create_folder(name);
                                }
                                cx.notify();
                            });
                            window.close_dialog(cx);
                        },
                    )),
            )
        });
    }

    fn open_import_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import_editor
            .update(cx, |state, cx| state.set_value(String::new(), window, cx));
        let editor = self.import_editor.clone();
        let title = self.t("Import subtree", "导入子树");
        let hint = self.t(
            "Paste export JSON. The path inside the file is the import root.",
            "粘贴导出的 JSON。文件里的 path 会作为导入根节点。",
        );
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            dialog.title(title.clone()).child(
                v_flex()
                    .gap_2()
                    .w(px(520.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(hint.clone()),
                    )
                    .child(Textarea::new(&editor).h(px(240.)))
                    .child(Button::new("do-import").primary().label("Import").on_click(
                        move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                let text = this.import_editor.read(cx).value().to_string();
                                this.import_json(text);
                                cx.notify();
                            });
                            window.close_dialog(cx);
                        },
                    )),
            )
        });
    }

    fn open_create_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        set_input(&self.node_name, String::new(), window, cx);
        set_input(&self.node_data, String::new(), window, cx);
        let name = self.node_name.clone();
        let data = self.node_data.clone();
        let title = self.t("Create node", "创建节点");
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let mode = view
                .read_with(cx, |this, _| session::mode_index(this.create_mode))
                .unwrap_or(0);
            let view_change = view.clone();
            let view_create = view.clone();
            dialog.title(title.clone()).child(
                v_flex()
                    .gap_2()
                    .w(px(420.))
                    .child(Input::new(&name))
                    .child(Input::new(&data))
                    .child(
                        RadioGroup::new("create-mode")
                            .selected_index(Some(mode))
                            .children([
                                Radio::new("persistent").label(CreateMode::Persistent.label()),
                                Radio::new("ephemeral").label(CreateMode::Ephemeral.label()),
                                Radio::new("pseq").label(CreateMode::PersistentSequential.label()),
                                Radio::new("eseq").label(CreateMode::EphemeralSequential.label()),
                            ])
                            .on_change(move |index, _, cx| {
                                let _ = view_change.update(cx, |this, cx| {
                                    this.create_mode = session::mode_from_index(*index);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(Button::new("do-create").primary().label("Create").on_click(
                        move |_, window, cx| {
                            let _ = view_create.update(cx, |this, cx| {
                                let name = this.node_name.read(cx).value().to_string();
                                let data = this.node_data.read(cx).value().to_string();
                                let mode = this.create_mode;
                                this.create_node(name, data, mode);
                                cx.notify();
                            });
                            window.close_dialog(cx);
                        },
                    )),
            )
        });
    }

    fn confirm_delete_node(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.selected_path.clone().unwrap_or_default();
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            alert
                .title(format!("Delete {path}?"))
                .description("This removes the znode. Children must already be gone.")
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.delete_selected();
                        cx.notify();
                    });
                    true
                })
        });
    }

    fn confirm_delete_connection(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            alert
                .title("Delete connection?")
                .description("The saved profile is removed from this machine.")
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.delete_connection(id);
                        cx.notify();
                    });
                    true
                })
        });
    }

    fn confirm_clear_children(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            alert
                .title("Delete every child?")
                .description("The selected node stays. Its descendants are removed.")
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.clear_children();
                        cx.notify();
                    });
                    true
                })
        });
    }
}

fn perm_checks(index: usize, perms: u32, cx: &mut Context<ZkApp>) -> Vec<gpui_kit::AnyElement> {
    [("r", 1u32), ("w", 2), ("c", 4), ("d", 8), ("a", 16)]
        .into_iter()
        .map(|(label, bit)| {
            Checkbox::new(format!("perm-{index}-{bit}"))
                .label(label)
                .checked(perms & bit != 0)
                .on_change(cx.listener(move |this, checked, _, cx| {
                    if let Some(entry) = this.edit_acl.get_mut(index) {
                        if *checked {
                            entry.perms |= bit;
                        } else {
                            entry.perms &= !bit;
                        }
                        this.acl_dirty = true;
                    }
                    cx.notify();
                }))
                .into_any_element()
        })
        .collect()
}

fn set_input(
    input: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    value: String,
    window: &mut Window,
    cx: &mut Context<ZkApp>,
) {
    input.update(cx, |state, cx| state.set_value(value, window, cx));
}

fn labeled(
    cx: &mut gpui_kit::App,
    label: &'static str,
    field: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .gap(px(4.))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(field)
}

trait IconKind {
    fn if_expandable(expandable: bool, expanded: bool) -> Self;
}

impl IconKind for IconName {
    fn if_expandable(expandable: bool, expanded: bool) -> Self {
        if !expandable {
            IconName::File
        } else if expanded {
            IconName::FolderOpen
        } else {
            IconName::FolderClosed
        }
    }
}
