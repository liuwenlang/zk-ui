# zk-ui

English | [中文](README.md)

A native desktop GUI for browsing and managing [Apache ZooKeeper](https://zookeeper.apache.org/) instances, built with Rust and [egui](https://www.egui.io/).

## Features

- **Tree Explorer** — Browse znodes in a hierarchical tree with lazy-loaded children, batched pagination, and search across the entire tree.
- **Node Management** — Create, delete, edit data, and manage ACLs for any znode. Supports Persistent, Ephemeral, Persistent Sequential, and Ephemeral Sequential create modes.
- **Connection Manager** — Save, organize, and quickly switch between multiple ZooKeeper connections. Connections can be grouped into folders with drag-and-drop reordering.
- **Bilingual UI** — Full English and Chinese (中文) interface, switchable at runtime.
- **Cross-platform** — Runs on macOS, Linux, and Windows with automatic CJK font detection.

## Screenshots

The interface consists of a left sidebar (resource manager with saved connections/folders), a toolbar, and a central detail panel showing node properties, ACLs, or statistics.

## Requirements

- [Rust](https://www.rustup.rs/) 1.70+ (2021 edition)
- A running ZooKeeper instance to connect to

## Build & Run

```bash
# Clone the repository
git clone https://github.com/<your-username>/zk-ui.git
cd zk-ui

# Build and run
cargo run --release

# Or connect to a specific host on launch
cargo run --release -- --connect 192.168.1.100:2181 --timeout 10000
```

### CLI Options

| Option | Default | Description |
|---|---|---|
| `--connect` | `127.0.0.1:2181` | ZooKeeper host:port |
| `--timeout` | `5000` | Connection timeout in ms |

## Project Structure

```
src/
├── main.rs              # Entry point, window setup
├── config.rs            # CLI argument parsing (clap)
├── db.rs                # Local SQLite persistence (connections & folders)
├── zk/
│   ├── mod.rs           # Re-exports
│   ├── types.rs         # ZK data types (NodeStat, AclEntry, CreateMode)
│   └── client.rs        # Background ZK thread manager & command protocol
└── app/
    ├── mod.rs           # ZkApp struct, constructor, main frame layout
    ├── types.rs         # UI types (Lang, TreeNode, Tab, ConnectState, Pending)
    ├── actions.rs       # Connection, data loading, and CRUD operations
    ├── respond.rs       # Async response channel handling
    ├── tree.rs          # ZK tree node rendering and interaction
    ├── icons.rs         # Custom icon painting (folders, documents, connections)
    ├── sidebar.rs       # Sidebar panel (resource manager, search, drag-and-drop)
    ├── dialogs.rs       # Connection and folder dialog modals
    └── detail.rs        # Node detail panel (Properties, ACL, Statistics tabs)
```

## Data Storage

Connection profiles and folders are stored in a local SQLite database at:

- **macOS/Linux**: `~/.local/share/zk-ui/zk-ui.db` (or `$XDG_DATA_HOME/zk-ui/zk-ui.db`)
- **Windows**: `%APPDATA%/zk-ui/zk-ui.db`

## Dependencies

| Crate | Purpose |
|---|---|
| `eframe` / `egui` | Immediate-mode GUI framework |
| `zookeeper` | ZooKeeper client bindings |
| `rusqlite` | Local SQLite database |
| `clap` | CLI argument parsing |
| `serde` / `serde_json` | Serialization |
| `tracing` | Structured logging |
| `chrono` | Timestamp formatting |

## FAQ

### macOS says "zk-ui.app is damaged and can't be opened"

This is caused by macOS Gatekeeper blocking the app because it is not notarized by Apple. Run this command in Terminal to fix it:

```bash
xattr -cr /path/to/zk-ui.app
```

## License

MIT
