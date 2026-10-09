# zk-ui

[English](README_EN.md) | 中文

一个基于 Rust 和 [GPUI Kit](https://gpui-kit.com/) 构建的 [Apache ZooKeeper](https://zookeeper.apache.org/) 原生桌面可视化管理工具。界面按 PrettyZoo / Redis Insight 的三栏工作台组织，渲染走 GPU，滚动和动画跟随显示器刷新率。

## 功能特性

- **三栏工作台** — 左侧连接资源管理器、中间虚拟化 znode 树、右侧检查器（数据 / ACL / 统计 / 四字命令）
- **节点管理** — 创建、删除、清空子节点、编辑数据、管理 ACL、导入和导出子树，支持持久、临时、持久顺序、临时顺序四种创建模式
- **大量节点** — 子节点名称按需取回，和渲染列表分开。每个展开节点先显示 200 行，可继续加载或一次显示全部；虚拟列表只绘制可见行，不为每个子节点预取 stat
- **连接管理器** — 保存并组织多个 ZooKeeper 连接，支持文件夹分组、重命名、认证和排序
- **搜索** — 先匹配已经加载的名称，再在后台分批遍历集群，结果出来后展开并滚到该节点。搜索不会堵住连接、刷新和编辑
- **高刷新率** — GPUI 在界面变化时按显示器刷新率呈现；节点树只绘制可见行。标题栏可打开帧时间指示。请使用 `cargo run --release` 查看实际流畅度
- **双语与主题** — 中英文实时切换，以及浅色 / 深色主题

## 界面概览

左侧是已保存的连接和文件夹，点一下即可连接。中间是可搜索的 znode 树：展开后按页放入虚拟列表。右侧检查器用来改数据、ACL、查看 stat，或对当前集群执行 `stat` / `srvr` / `mntr` / `conf` / `envi`。

## 环境要求

- [Rust](https://www.rustup.rs/) 1.70+（2021 edition）
- 一个可连接的 ZooKeeper 实例

## 构建与运行

```bash
# 克隆仓库
git clone https://github.com/<your-username>/zk-ui.git
cd zk-ui

# 构建并运行
cargo run --release

# 指定连接地址启动
cargo run --release -- --connect 192.168.1.100:2181 --timeout 10000
```

### 命令行参数

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--connect` | `127.0.0.1:2181` | ZooKeeper 主机地址 |
| `--timeout` | `5000` | 连接超时（毫秒） |

## 项目结构

```
src/
├── main.rs              # GPUI Kit 入口，窗口按显示器刷新
├── config.rs            # 命令行参数解析 (clap)
├── db.rs                # 本地 SQLite 存储（连接配置 & 文件夹）
├── zk/
│   ├── mod.rs           # 模块导出
│   ├── types.rs         # ZK 数据类型 (NodeStat, AclEntry, CreateMode)
│   └── client.rs        # 后台 ZK 线程管理 & 命令协议
└── app/
    ├── mod.rs           # ZkApp 状态与输入框
    ├── view.rs          # 三栏工作台界面
    ├── session.rs       # 连接、树、CRUD 与响应处理
    ├── tree_model.rs    # 节点目录与分页后的可见行（含测试）
    └── i18n.rs          # 中英文
```

## 数据存储

连接配置和文件夹存储在本地 SQLite 数据库中：

- **macOS/Linux**: `~/.local/share/zk-ui/zk-ui.db`（或 `$XDG_DATA_HOME/zk-ui/zk-ui.db`）
- **Windows**: `%APPDATA%/zk-ui/zk-ui.db`

## 主要依赖

| Crate | 用途 |
|---|---|
| `gpui-kit` | GPU 桌面 UI、组件、主题与窗口 |
| `gpui-fps` | 帧时间指示，用来确认刷新是否跟上显示器 |
| `zookeeper` | ZooKeeper 客户端 |
| `rusqlite` | 本地 SQLite 数据库 |
| `clap` | 命令行参数解析 |
| `serde` / `serde_json` | 序列化 |
| `tracing` | 结构化日志 |
| `chrono` | 时间戳格式化 |

## 常见问题

### macOS 提示 "zk-ui.app 已损坏，无法打开"

这是因为应用未经过 Apple 公证，被 macOS Gatekeeper 拦截。在终端执行以下命令即可解决：

```bash
xattr -cr /path/to/zk-ui.app
```

## License

MIT
