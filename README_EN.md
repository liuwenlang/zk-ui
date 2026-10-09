# zk-ui

English | [中文](README.md)

A native desktop GUI for browsing and managing [Apache ZooKeeper](https://zookeeper.apache.org/) instances, built with Rust and [GPUI Kit](https://gpui-kit.com/). The workspace follows the three-pane layout used by PrettyZoo and Redis Insight, and the GPU renderer presents on the display refresh rate.

## Features

- **Three-pane workspace** — Connection explorer, virtualized znode tree, and an inspector for data, ACL, stat, and four-letter commands.
- **Node management** — Create, delete, clear children, edit data, manage ACLs, and import or export a subtree. Supports Persistent, Ephemeral, Persistent Sequential, and Ephemeral Sequential create modes.
- **Large trees** — Child names stay in a catalog, separate from the render list. An expanded node shows 200 rows first, then load more or show all. The virtual list paints visible rows only and does not stat every child.
- **Connection manager** — Save clusters, group them in folders, rename folders, store digest auth, and reorder profiles.
- **Search** — Match names already loaded, then walk the cluster in batches on the background thread. A hit expands the path and scrolls it into view without blocking connect, refresh, or edits.
- **High refresh** — GPUI presents when the UI changes, at the display refresh rate. The tree paints only visible rows. Toggle the frame-time HUD from the title bar. Use `cargo run --release` to judge smoothness.
- **Language and theme** — English and Chinese at runtime, plus light and dark themes.

## Screenshots

The left column holds saved connections. The center is the znode tree, paged into a virtual list as you expand it. The right inspector edits data and ACLs, shows stat, and runs `stat`, `srvr`, `mntr`, `conf`, and `envi`.

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
├── main.rs              # GPUI Kit entry, display-refresh window
├── config.rs            # CLI argument parsing (clap)
├── db.rs                # Local SQLite persistence (connections & folders)
├── zk/
│   ├── mod.rs           # Re-exports
│   ├── types.rs         # ZK data types (NodeStat, AclEntry, CreateMode)
│   └── client.rs        # Background ZK thread manager & command protocol
└── app/
    ├── mod.rs           # ZkApp state and inputs
    ├── view.rs          # Three-pane workspace
    ├── session.rs       # Connect, tree, CRUD, and response polling
    ├── tree_model.rs    # Visible tree rows (tested)
    └── i18n.rs          # English and Chinese
```

## Data Storage

Connection profiles and folders are stored in a local SQLite database at:

- **macOS/Linux**: `~/.local/share/zk-ui/zk-ui.db` (or `$XDG_DATA_HOME/zk-ui/zk-ui.db`)
- **Windows**: `%APPDATA%/zk-ui/zk-ui.db`

## Dependencies

| Crate | Purpose |
|---|---|
| `gpui-kit` | GPU desktop UI, components, theme, and windowing |
| `gpui-fps` | Frame-time HUD for checking the display refresh |
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
