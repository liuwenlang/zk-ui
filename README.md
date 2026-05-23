# zk-ui

[English](README_EN.md) | 中文

一个基于 Rust 和 [egui](https://www.egui.io/) 构建的 [Apache ZooKeeper](https://zookeeper.apache.org/) 原生桌面可视化管理工具。

## 功能特性

- **树形浏览器** — 层级展示 znode 节点，支持懒加载、分批翻页和全局搜索
- **节点管理** — 创建、删除、编辑数据、管理 ACL，支持持久、临时、持久顺序、临时顺序四种创建模式
- **连接管理器** — 保存并组织多个 ZooKeeper 连接，支持文件夹分组和拖拽排序
- **双语界面** — 支持中英文实时切换
- **跨平台** — 支持 macOS、Linux、Windows，自动检测 CJK 字体

## 界面概览

界面由左侧边栏（资源管理器，含已保存的连接/文件夹）、工具栏和中央详情面板（显示节点属性、ACL 或统计信息）组成。

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
├── main.rs              # 程序入口，窗口初始化
├── config.rs            # 命令行参数解析 (clap)
├── db.rs                # 本地 SQLite 存储（连接配置 & 文件夹）
├── zk/
│   ├── mod.rs           # 模块导出
│   ├── types.rs         # ZK 数据类型 (NodeStat, AclEntry, CreateMode)
│   └── client.rs        # 后台 ZK 线程管理 & 命令协议
└── app/
    ├── mod.rs           # ZkApp 结构体、构造函数、主布局
    ├── types.rs         # UI 类型 (Lang, TreeNode, Tab, ConnectState, Pending)
    ├── actions.rs       # 连接、数据加载、CRUD 操作
    ├── respond.rs       # 异步响应通道处理
    ├── tree.rs          # ZK 树节点渲染与交互
    ├── icons.rs         # 自定义图标绘制（文件夹、文档、连接）
    ├── sidebar.rs       # 侧边栏面板（资源管理器、搜索、拖拽）
    ├── dialogs.rs       # 连接和文件夹对话框
    └── detail.rs        # 节点详情面板（属性、ACL、统计标签页）
```

## 数据存储

连接配置和文件夹存储在本地 SQLite 数据库中：

- **macOS/Linux**: `~/.local/share/zk-ui/zk-ui.db`（或 `$XDG_DATA_HOME/zk-ui/zk-ui.db`）
- **Windows**: `%APPDATA%/zk-ui/zk-ui.db`

## 主要依赖

| Crate | 用途 |
|---|---|
| `eframe` / `egui` | 即时模式 GUI 框架 |
| `zookeeper` | ZooKeeper 客户端 |
| `rusqlite` | 本地 SQLite 数据库 |
| `clap` | 命令行参数解析 |
| `serde` / `serde_json` | 序列化 |
| `tracing` | 结构化日志 |
| `chrono` | 时间戳格式化 |

## License

MIT
