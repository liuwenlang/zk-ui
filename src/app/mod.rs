mod i18n;
mod session;
mod tree_model;
mod view;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::VirtualListScrollHandle;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Pixels, Render, Size, Subscription, Window,
};

use crate::config::Cli;
use crate::db::{ConnProfile, Folder, LocalDb};
use crate::zk::{AclEntry, CreateMode, ZkManager};

use i18n::Lang;
use session::{ConnectState, InspectorTab, NodeDetail, Pending};
use tree_model::{FlatRow, TreeCatalog};

pub struct ZkApp {
    zk: ZkManager,
    db: LocalDb,
    pending: Pending,
    lang: Lang,
    theme_dark: bool,
    show_fps: bool,
    cli_hosts: String,

    hosts: String,
    timeout_ms: i32,
    active_conn_name: String,
    active_conn_id: Option<i64>,
    connect_state: ConnectState,
    pending_auth: Option<(String, String)>,

    root_folders: Vec<Folder>,
    root_connections: Vec<ConnProfile>,
    folder_children: HashMap<i64, (Vec<Folder>, Vec<ConnProfile>)>,
    expanded_folders: HashSet<i64>,
    selected_folder_id: Option<i64>,

    tree: TreeCatalog,
    rows: Vec<FlatRow>,
    row_sizes: Rc<Vec<Size<Pixels>>>,
    search_sizes: Rc<Vec<Size<Pixels>>>,
    tree_scroll: VirtualListScrollHandle,
    scroll_to: Option<usize>,
    pending_reveal: Option<String>,
    focus_child: Option<(String, String)>,
    selected_path: Option<String>,
    detail: Option<NodeDetail>,
    inspector: InspectorTab,
    edit_data: String,
    edit_acl: Vec<AclEntry>,
    data_dirty: bool,
    acl_dirty: bool,
    sync_editor: bool,
    clear_target: Option<String>,
    search_query: String,
    search_results: Vec<String>,
    search_local: Vec<String>,
    search_remote: Vec<String>,
    search_scanned: usize,
    search_in_progress: bool,
    search_generation: u64,
    search_ticket: u64,
    server_output: String,
    export_text: String,
    export_ready: bool,
    status_message: String,
    toast: Option<String>,
    create_mode: CreateMode,
    conn_edit_id: Option<i64>,
    folder_edit_id: Option<i64>,

    search_input: Entity<InputState>,
    data_editor: Entity<TextareaState>,
    conn_name: Entity<InputState>,
    conn_hosts: Entity<InputState>,
    conn_timeout: Entity<InputState>,
    conn_scheme: Entity<InputState>,
    conn_secret: Entity<InputState>,
    node_name: Entity<InputState>,
    node_data: Entity<InputState>,
    folder_name: Entity<InputState>,
    acl_scheme: Entity<InputState>,
    acl_id: Entity<InputState>,
    import_editor: Entity<TextareaState>,
    _subscriptions: Vec<Subscription>,
}

impl ZkApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, cli: Cli) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search znodes"));
        let data_editor = cx.new(|cx| TextareaState::new(window, cx));
        let conn_name = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let conn_hosts = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("127.0.0.1:2181")
                .default_value(cli.connect.clone())
        });
        let conn_timeout =
            cx.new(|cx| InputState::new(window, cx).default_value(cli.timeout.to_string()));
        let conn_scheme = cx.new(|cx| InputState::new(window, cx).placeholder("digest"));
        let conn_secret = cx.new(|cx| InputState::new(window, cx).placeholder("user:password"));
        let node_name = cx.new(|cx| InputState::new(window, cx).placeholder("node"));
        let node_data = cx.new(|cx| InputState::new(window, cx).placeholder(""));
        let folder_name = cx.new(|cx| InputState::new(window, cx).placeholder("Folder"));
        let acl_scheme = cx.new(|cx| InputState::new(window, cx).placeholder("world"));
        let acl_id = cx.new(|cx| InputState::new(window, cx).placeholder("anyone"));
        let import_editor = cx.new(|cx| TextareaState::new(window, cx));

        let search_sub = cx.subscribe_in(&search_input, window, |this, input, event, _, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let value = input.read(cx).value().to_string();
            this.search_ticket = this.search_ticket.wrapping_add(1);
            let ticket = this.search_ticket;
            cx.spawn(async move |this, async_cx| {
                async_cx
                    .background_executor()
                    .timer(Duration::from_millis(220))
                    .await;
                let _ = this.update(async_cx, |this, cx| {
                    if this.search_ticket == ticket {
                        this.start_search(value);
                        cx.notify();
                    }
                });
            })
            .detach();
        });

        let editor_sub = cx.subscribe_in(&data_editor, window, |this, editor, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.edit_data = editor.read(cx).value().to_string();
                this.data_dirty = true;
                cx.notify();
            }
        });

        let mut app = Self {
            zk: ZkManager::new(),
            db: LocalDb::new().expect("open zk-ui database"),
            pending: Pending::default(),
            lang: Lang::Zh,
            theme_dark: false,
            show_fps: true,
            cli_hosts: cli.connect,
            hosts: "127.0.0.1:2181".into(),
            timeout_ms: 5000,
            active_conn_name: String::new(),
            active_conn_id: None,
            connect_state: ConnectState::Disconnected,
            pending_auth: None,
            root_folders: Vec::new(),
            root_connections: Vec::new(),
            folder_children: HashMap::new(),
            expanded_folders: HashSet::new(),
            selected_folder_id: None,
            tree: TreeCatalog::new(),
            rows: Vec::new(),
            row_sizes: Rc::new(Vec::new()),
            search_sizes: Rc::new(Vec::new()),
            tree_scroll: VirtualListScrollHandle::new(),
            scroll_to: None,
            pending_reveal: None,
            focus_child: None,
            selected_path: None,
            detail: None,
            inspector: InspectorTab::Data,
            edit_data: String::new(),
            edit_acl: Vec::new(),
            data_dirty: false,
            acl_dirty: false,
            sync_editor: false,
            clear_target: None,
            search_query: String::new(),
            search_results: Vec::new(),
            search_local: Vec::new(),
            search_remote: Vec::new(),
            search_scanned: 0,
            search_in_progress: false,
            search_generation: 0,
            search_ticket: 0,
            server_output: String::new(),
            export_text: String::new(),
            export_ready: false,
            status_message: "Ready".into(),
            toast: None,
            create_mode: CreateMode::Persistent,
            conn_edit_id: None,
            folder_edit_id: None,
            search_input,
            data_editor,
            conn_name,
            conn_hosts,
            conn_timeout,
            conn_scheme,
            conn_secret,
            node_name,
            node_data,
            folder_name,
            acl_scheme,
            acl_id,
            import_editor,
            _subscriptions: vec![search_sub, editor_sub],
        };
        app.reload_catalog();
        if app.root_connections.is_empty() && app.root_folders.is_empty() {
            let _ = app
                .db
                .add_connection("localhost", "127.0.0.1:2181", 5000, "", "", None);
            app.reload_catalog();
        }

        cx.spawn(async move |this, async_cx| loop {
            let busy = this
                .read_with(async_cx, |app, _| app.has_pending())
                .unwrap_or(false);
            let wait = if busy { 8 } else { 80 };
            async_cx
                .background_executor()
                .timer(Duration::from_millis(wait))
                .await;
            if this
                .update(async_cx, |app, cx| {
                    if app.poll_responses() {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        })
        .detach();

        app
    }
}

impl Render for ZkApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_workspace(window, cx)
    }
}
