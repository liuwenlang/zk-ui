use std::sync::mpsc;
use std::thread;

use zookeeper::{Acl, CreateMode, ZooKeeper, WatchedEvent, Watcher};

use super::types::{AclEntry, CreateMode as UiCreateMode, NodeStat, bits_to_perm};

#[allow(dead_code)]
pub enum ZkCmd {
    Connect { hosts: String, timeout_ms: i32, resp: mpsc::Sender<ZkResponse> },
    Disconnect,
    GetChildren { path: String, resp: mpsc::Sender<ZkResponse> },
    GetData { path: String, resp: mpsc::Sender<ZkResponse> },
    GetAcl { path: String, resp: mpsc::Sender<ZkResponse> },
    SetAcl { path: String, acl: Vec<AclEntry>, version: i32, resp: mpsc::Sender<ZkResponse> },
    Create { path: String, data: Vec<u8>, acl: Vec<AclEntry>, mode: UiCreateMode, resp: mpsc::Sender<ZkResponse> },
    SetData { path: String, data: Vec<u8>, version: i32, resp: mpsc::Sender<ZkResponse> },
    Delete { path: String, version: i32, resp: mpsc::Sender<ZkResponse> },
    DeleteChildren { path: String, resp: mpsc::Sender<ZkResponse> },
    Exists { path: String, resp: mpsc::Sender<ZkResponse> },
    ExportSubtree { path: String, resp: mpsc::Sender<ZkResponse> },
    ImportSubtree { path: String, data: serde_json::Value, resp: mpsc::Sender<ZkResponse> },
    FourLetterCmd { host: String, cmd: String, resp: mpsc::Sender<ZkResponse> },
    AddAuth { scheme: String, credential: Vec<u8>, resp: mpsc::Sender<ZkResponse> },
    Watch { path: String, watch_type: WatchType, resp: mpsc::Sender<ZkResponse> },
    SearchNodes { query: String, max_results: usize, resp: mpsc::Sender<ZkResponse> },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum WatchType {
    Children,
    Data,
}

#[allow(dead_code)]
pub enum ZkResponse {
    Connected,
    Children(Vec<String>),
    Data { data: Vec<u8>, stat: NodeStat },
    Acl { acl: Vec<AclEntry>, stat: NodeStat },
    Stat(NodeStat),
    Created,
    Deleted,
    ChildrenCleared(usize),
    SetData,
    SetAcl,
    ExportData(serde_json::Value),
    ImportDone,
    FourLetterResult(String),
    AuthAdded,
    Error(String),
    WatchEvent { path: String, event_type: String },
    Disconnected,
    SearchResults(Vec<String>),
}

struct LogWatcher;
impl Watcher for LogWatcher {
    fn handle(&self, _event: WatchedEvent) {}
}

pub struct ZkManager {
    tx: mpsc::Sender<ZkCmd>,
}

impl ZkManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<ZkCmd>();

        thread::spawn(move || {
            let mut zk: Option<ZooKeeper> = None;

            loop {
                let cmd = match rx.recv() {
                    Ok(cmd) => cmd,
                    Err(_) => break,
                };

                match cmd {
                    ZkCmd::Connect { hosts, timeout_ms, resp } => {
                        match ZooKeeper::connect(&hosts, std::time::Duration::from_millis(timeout_ms as u64), LogWatcher) {
                            Ok(conn) => {
                                zk = Some(conn);
                                let _ = resp.send(ZkResponse::Connected);
                            }
                            Err(e) => {
                                let _ = resp.send(ZkResponse::Error(format!("Connect failed: {}", e)));
                            }
                        }
                    }
                    ZkCmd::Disconnect => { zk = None; }
                    ZkCmd::GetChildren { path, resp } => {
                        match &zk {
                            Some(z) => match z.get_children(&path, false) {
                                Ok(children) => { let _ = resp.send(ZkResponse::Children(children)); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::GetData { path, resp } => {
                        match &zk {
                            Some(z) => match z.get_data(&path, false) {
                                Ok((data, stat)) => { let _ = resp.send(ZkResponse::Data { data, stat: stat.into() }); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::GetAcl { path, resp } => {
                        match &zk {
                            Some(z) => match z.get_acl(&path) {
                                Ok((acl, stat)) => {
                                    let entries: Vec<AclEntry> = acl.iter().map(|a| a.into()).collect();
                                    let _ = resp.send(ZkResponse::Acl { acl: entries, stat: stat.into() });
                                }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::SetAcl { path, acl, version, resp } => {
                        match &zk {
                            Some(z) => {
                                let zk_acl: Vec<Acl> = acl.iter().map(|a| Acl::new(
                                    bits_to_perm(a.perms), a.scheme.clone(), a.id.clone(),
                                )).collect();
                                match z.set_acl(&path, zk_acl, Some(version)) {
                                    Ok(_) => { let _ = resp.send(ZkResponse::SetAcl); }
                                    Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                                }
                            }
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::Create { path, data, acl, mode, resp } => {
                        match &zk {
                            Some(z) => {
                                let zk_acl: Vec<Acl> = acl.iter().map(|a| Acl::new(
                                    bits_to_perm(a.perms), a.scheme.clone(), a.id.clone(),
                                )).collect();
                                let flag = match mode {
                                    UiCreateMode::Persistent => CreateMode::Persistent,
                                    UiCreateMode::Ephemeral => CreateMode::Ephemeral,
                                    UiCreateMode::PersistentSequential => CreateMode::PersistentSequential,
                                    UiCreateMode::EphemeralSequential => CreateMode::EphemeralSequential,
                                };
                                match z.create(&path, data, zk_acl, flag) {
                                    Ok(_) => { let _ = resp.send(ZkResponse::Created); }
                                    Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                                }
                            }
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::SetData { path, data, version, resp } => {
                        match &zk {
                            Some(z) => match z.set_data(&path, data, Some(version)) {
                                Ok(_) => { let _ = resp.send(ZkResponse::SetData); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::Delete { path, version, resp } => {
                        match &zk {
                            Some(z) => match z.delete(&path, Some(version)) {
                                Ok(_) => { let _ = resp.send(ZkResponse::Deleted); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::DeleteChildren { path, resp } => {
                        match &zk {
                            Some(z) => match clear_children(z, &path) {
                                Ok(count) => { let _ = resp.send(ZkResponse::ChildrenCleared(count)); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(e)); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::Exists { path, resp } => {
                        match &zk {
                            Some(z) => match z.exists(&path, false) {
                                Ok(Some(s)) => { let _ = resp.send(ZkResponse::Stat(s.into())); }
                                Ok(None) => { let _ = resp.send(ZkResponse::Error("Node does not exist".into())); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::ExportSubtree { path, resp } => {
                        match &zk {
                            Some(z) => match export_node(z, &path) {
                                Ok(val) => { let _ = resp.send(ZkResponse::ExportData(val)); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(e)); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::ImportSubtree { path, data, resp } => {
                        match &zk {
                            Some(z) => match import_node(z, &path, &data) {
                                Ok(_) => { let _ = resp.send(ZkResponse::ImportDone); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(e)); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::FourLetterCmd { host, cmd, resp } => {
                        match send_four_letter_cmd(&host, &cmd) {
                            Ok(result) => { let _ = resp.send(ZkResponse::FourLetterResult(result)); }
                            Err(e) => { let _ = resp.send(ZkResponse::Error(e)); }
                        }
                    }
                    ZkCmd::AddAuth { scheme, credential, resp } => {
                        match &zk {
                            Some(z) => match z.add_auth(&scheme, credential) {
                                Ok(_) => { let _ = resp.send(ZkResponse::AuthAdded); }
                                Err(e) => { let _ = resp.send(ZkResponse::Error(format!("{}", e))); }
                            },
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                    ZkCmd::Watch { path, watch_type: _, resp } => {
                        use std::sync::Arc;
                        let resp = Arc::new(resp);
                        let path_arc: Arc<str> = Arc::from(path.as_str());
                        if let Some(z) = &zk {
                            let resp_c = resp.clone();
                            let p = path_arc.clone();
                            let _ = z.get_children_w(&path, move |event: WatchedEvent| {
                                let _ = resp_c.send(ZkResponse::WatchEvent {
                                    path: (*p).to_string(),
                                    event_type: format!("{:?}", event.event_type),
                                });
                            });
                            let resp_c = resp.clone();
                            let p = path_arc.clone();
                            let _ = z.get_data_w(&path, move |event: WatchedEvent| {
                                let _ = resp_c.send(ZkResponse::WatchEvent {
                                    path: (*p).to_string(),
                                    event_type: format!("{:?}", event.event_type),
                                });
                            });
                        }
                    }
                    ZkCmd::SearchNodes { query, max_results, resp } => {
                        match &zk {
                            Some(z) => {
                                let mut results = Vec::new();
                                let q = query.to_lowercase();
                                if !q.is_empty() {
                                    search_nodes(z, "/", &q, &mut results, max_results);
                                }
                                let _ = resp.send(ZkResponse::SearchResults(results));
                            }
                            None => { let _ = resp.send(ZkResponse::Error("Not connected".into())); }
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    pub fn send(&self, cmd: ZkCmd) {
        let _ = self.tx.send(cmd);
    }
}

fn child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", parent, name)
    }
}

fn delete_subtree(zk: &ZooKeeper, path: &str) -> Result<(), String> {
    let children = zk.get_children(path, false).map_err(|e| format!("{}", e))?;
    for child in children {
        delete_subtree(zk, &child_path(path, &child))?;
    }
    zk.delete(path, Some(-1)).map_err(|e| format!("{}", e))?;
    Ok(())
}

fn count_subtree(zk: &ZooKeeper, path: &str) -> Result<usize, String> {
    let mut count = 1;
    let children = zk.get_children(path, false).map_err(|e| format!("{}", e))?;
    for child in children {
        count += count_subtree(zk, &child_path(path, &child))?;
    }
    Ok(count)
}

fn clear_children(zk: &ZooKeeper, path: &str) -> Result<usize, String> {
    let children = zk.get_children(path, false).map_err(|e| format!("{}", e))?;
    let mut deleted = 0usize;
    for child in children {
        let child_path = child_path(path, &child);
        deleted += count_subtree(zk, &child_path)?;
        delete_subtree(zk, &child_path)?;
    }
    Ok(deleted)
}

fn path_matches_query(path: &str, query: &str) -> bool {
    if path.to_lowercase().contains(query) {
        return true;
    }
    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map_or(false, |name| name.to_lowercase().contains(query))
}

fn search_nodes(
    zk: &ZooKeeper,
    path: &str,
    query: &str,
    results: &mut Vec<String>,
    max_results: usize,
) {
    if results.len() >= max_results {
        return;
    }
    if path != "/" && path_matches_query(path, query) {
        results.push(path.to_string());
        if results.len() >= max_results {
            return;
        }
    }
    let children = match zk.get_children(path, false) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut children: Vec<_> = children.into_iter().collect();
    children.sort();
    for child in children {
        let child_path = if path == "/" {
            format!("/{}", child)
        } else {
            format!("{}/{}", path, child)
        };
        search_nodes(zk, &child_path, query, results, max_results);
        if results.len() >= max_results {
            break;
        }
    }
}

fn export_node(zk: &ZooKeeper, path: &str) -> Result<serde_json::Value, String> {
    let (data, _stat) = zk.get_data(path, false).map_err(|e| format!("{}", e))?;
    let children = zk.get_children(path, false).map_err(|e| format!("{}", e))?;

    let mut node = serde_json::Map::new();
    node.insert("path".into(), serde_json::Value::String(path.to_string()));
    node.insert("data".into(), serde_json::Value::String(String::from_utf8_lossy(&data).to_string()));
    node.insert("dataBytes".into(), serde_json::Value::Number(serde_json::Number::from(data.len())));

    let mut children_arr = serde_json::Map::new();
    for child in children {
        let child_path = if path == "/" { format!("/{}", child) } else { format!("{}/{}", path, child) };
        children_arr.insert(child, export_node(zk, &child_path)?);
    }
    node.insert("children".into(), serde_json::Value::Object(children_arr));

    Ok(serde_json::Value::Object(node))
}

fn import_node(zk: &ZooKeeper, path: &str, data: &serde_json::Value) -> Result<(), String> {
    let node_data = data.get("data").and_then(|v| v.as_str()).unwrap_or("");
    let acl = vec![Acl::new(zookeeper::Permission::ALL, "world", "anyone")];

    let exists = zk.exists(path, false).map_err(|e| format!("{}", e))?;
    if exists.is_none() {
        zk.create(path, node_data.as_bytes().to_vec(), acl, CreateMode::Persistent)
            .map_err(|e| format!("{}", e))?;
    } else {
        zk.set_data(path, node_data.as_bytes().to_vec(), None)
            .map_err(|e| format!("{}", e))?;
    }

    if let Some(children) = data.get("children").and_then(|v| v.as_object()) {
        for (child_name, child_data) in children {
            let child_path = if path == "/" { format!("/{}", child_name) } else { format!("{}/{}", path, child_name) };
            import_node(zk, &child_path, child_data)?;
        }
    }

    Ok(())
}

fn send_four_letter_cmd(host: &str, cmd: &str) -> Result<String, String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let mut stream = TcpStream::connect(host).map_err(|e| format!("TCP connect failed: {}", e))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("set timeout failed: {}", e))?;

    stream.write_all(format!("{}\n", cmd).as_bytes()).map_err(|e| format!("write failed: {}", e))?;

    let mut response = String::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => response.push_str(&String::from_utf8_lossy(&buf[..n])),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(format!("read failed: {}", e)),
        }
    }

    Ok(response)
}
