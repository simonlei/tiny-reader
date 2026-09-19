# Tiny Reader

一个跨平台的极简 RSS 阅读器，参考 Inoreader 的三栏布局与 `j` / `k` 快捷键。

数据**不在客户端**保存，而是由一个 Rust 服务端统一拉取、解析、入库。多台客户端（家里电脑、笔记本、另一台机器）连同一个服务端即可共享同一份「已读 / 星标 / 订阅列表」状态。

## 架构

```
┌────────────────────────┐        HTTP (Bearer token)        ┌─────────────────────────┐
│  Tauri 2 + Vue 3 客户端 │  ───────────────────────────────▶ │  Rust 服务端 (Axum)      │
│  · 只负责渲染与交互      │  ◀───────────────────────────────  │  · 定时/手动拉取 RSS     │
│  · 设置存 localStorage   │        JSON over REST             │  · SQLite 持久化         │
└────────────────────────┘                                    └─────────────────────────┘
                                                                          │
                                                                          ▼
                                                                   各个 RSS / Atom 源
```

- **客户端** `src-tauri/` + `src/`：Tauri 2 壳 + Vue 3 前端，本身不含任何业务逻辑数据。
- **服务端** `server/`：Axum + SQLite，负责订阅源增删改查、RSS 拉取解析（支持 RSS / Atom / JSON Feed）、OPML 导入导出。
- **鉴权**：单用户模式，服务端配置文件里写死一个 token，客户端在「设置」里填同一个 token 即可。

## 目录结构

```
tiny-reader/
├─ src/                     # Vue 3 前端
│  ├─ api/                  # 服务端 HTTP 客户端 + 类型定义
│  ├─ components/           # Sidebar / ArticleList / ArticleView / 各种对话框
│  ├─ lib/                  # 时间格式化、打开外部链接
│  ├─ stores/               # settings（地址+token）、app（订阅源/文章/选中态）
│  └─ styles/
├─ src-tauri/               # Tauri 2 客户端（Rust）
│  ├─ src/lib.rs            # open_url / app_info 命令
│  ├─ capabilities/
│  └─ tauri.conf.json
├─ server/                  # 数据服务端（Rust）
│  ├─ src/
│  │  ├─ main.rs            # 入口、CLI、优雅退出
│  │  ├─ config.rs          # config.toml 载入（缺失时自动生成随机 token）
│  │  ├─ db.rs              # SQLite 迁移 + CRUD
│  │  ├─ rss.rs             # reqwest + feed-rs 拉取解析，带 ETag/Last-Modified
│  │  ├─ opml.rs            # OPML 导入导出
│  │  ├─ api.rs             # Axum 路由 + token 鉴权中间件
│  │  └─ models.rs
│  └─ config.example.toml
├─ dev.bat / dev.sh         # 启动客户端开发模式（cmd / Git Bash）
├─ start-server.bat         # 启动服务端（cmd）
├─ start-server.sh          # 启动服务端（Git Bash）
└─ tools/                   # 图标生成、MSVC 环境脚本
```

> Windows 下 `.bat` 和 `.sh` 是等价的两套启动脚本，都会自动加载 MSVC 环境。
> Git Bash 里如果 `cmd.exe` 被禁用，`.sh` 会自动回退到 `source tools/msvc-env.sh`
> （该脚本自动探测本机最新的 MSVC 工具集与 Windows SDK 版本）；
> 也可以用 `MSVC_NO_CMD=1 ./start-server.sh` / `MSVC_NO_CMD=1 ./dev.sh` 强制走这条路径。

## 快速开始

### 1. 启动服务端

```bash
cd server
cargo run
```

或者双击项目根目录的 `start-server.bat`（Windows，会自动加载 MSVC 环境）；Git Bash 下用：

```bash
./start-server.sh
```

首次运行会在当前目录生成 `config.toml`，**终端里会打印生成的随机 token**，例如：

```
WARN  tiny_reader_server: 未找到配置文件，已在 ./config.toml 生成默认配置
WARN  tiny_reader_server:   生成的访问 token = aB3dEf...
```

默认监听 `http://127.0.0.1:8787`。

### 2. 安装前端依赖并启动客户端

```bash
npm install
npm run tauri dev
```

或者直接双击 `dev.bat`；Git Bash 下用 `./dev.sh`（依赖缺失时会自动 `npm install`）。

### 3. 配置连接

首次启动会弹出「服务端设置」对话框：

- **服务端地址**：默认 `http://127.0.0.1:8787`；如果服务端跑在另一台机器上，填那台机器的局域网 IP，并把 `config.toml` 里的 `server.host` 改成 `0.0.0.0`。
- **访问 token**：填 `config.toml` 里 `auth.token` 的值。

点「测试连接」验证，成功后保存即可。

## 服务端配置

完整配置项见 [`server/config.example.toml`](server/config.example.toml)：

| 配置 | 说明 |
| --- | --- |
| `server.host` / `server.port` | 监听地址与端口。`0.0.0.0` 表示允许局域网访问 |
| `auth.token` | 单用户模式的访问 token（也可用 `TINY_READER_TOKEN` 环境变量覆盖） |
| `storage.database_path` | SQLite 文件路径 |
| `storage.max_articles_per_feed` | 每个源最多保留多少篇，超出清理最旧的已读文章；`0` 为不限制 |
| `refresh.concurrency` | 刷新时的并发数 |
| `refresh.timeout_secs` | 单个请求超时 |
| `refresh.auto_interval_secs` | 定时自动刷新间隔（秒），`0` 表示只在点「刷新」时拉取 |

环境变量：`TINY_READER_CONFIG`（配置文件路径）、`TINY_READER_TOKEN`、`TINY_READER_HOST`、`TINY_READER_PORT`、`TINY_READER_DB`。

## 快捷键

| 键 | 功能 |
| --- | --- |
| <kbd>j</kbd> / <kbd>↓</kbd> | 下一篇文章 |
| <kbd>k</kbd> / <kbd>↑</kbd> | 上一篇文章 |
| <kbd>s</kbd> | 收藏 / 取消收藏 |
| <kbd>u</kbd> | 已读 / 未读切换 |
| <kbd>o</kbd> / <kbd>Enter</kbd> | 在系统浏览器里打开原文 |
| <kbd>r</kbd> | 刷新所有订阅源 |
| <kbd>Shift</kbd>+<kbd>A</kbd> | 当前范围全部标为已读 |
| <kbd>Home</kbd> / <kbd>End</kbd> | 跳到第一 / 最后一篇 |
| <kbd>/</kbd> | 聚焦搜索框 |
| <kbd>Esc</kbd> | 关闭对话框 |

选中一篇文章约 0.6 秒后会自动标记为已读（和 Inoreader 一致），此时列表不会把它移除，方便回看；按 <kbd>u</kbd> 可以标回未读。

## HTTP API

所有 `/api/*` 请求都需要携带 token：

```
Authorization: Bearer <token>
# 或
X-Auth-Token: <token>
```

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/health` | 健康检查（无需 token） |
| GET | `/api/feeds` | 订阅源列表（含未读数） |
| POST | `/api/feeds` | 新增订阅源（会先探测拉取一次） |
| GET | `/api/feeds/{id}` | 单个订阅源 |
| PUT | `/api/feeds/{id}` | 修改 url / title / category |
| DELETE | `/api/feeds/{id}` | 删除订阅源及其文章 |
| POST | `/api/feeds/{id}/refresh` | 只刷新这一个源 |
| GET | `/api/feeds/export` | 导出 OPML |
| POST | `/api/feeds/import` | 导入 OPML（JSON `{opml, fetch_now}` 或裸 XML） |
| GET | `/api/articles` | 文章列表，支持 `feed_id` / `unread_only` / `starred_only` / `limit` / `offset` / `q` |
| GET | `/api/articles/{id}` | 单篇文章 |
| POST | `/api/articles/{id}/read` | 设置已读状态 `{value}` |
| POST | `/api/articles/{id}/star` | 设置星标 `{value}` |
| POST | `/api/articles/mark-read` | 批量标记已读 `{feed_id}` 或 `{ids}` |
| GET | `/api/stats` | 总数 / 未读 / 星标 |
| POST | `/api/refresh` | 触发全量刷新（异步） |
| GET | `/api/refresh/status` | 查询刷新进度 |

## 打包

```bash
npm run tauri build          # 客户端安装包
cargo build --release --manifest-path server/Cargo.toml   # 服务端可执行文件
```

## 已知取舍

- **没有多租户**：服务端只有一个 token，拿到 token 就拥有全部权限。跨公网部署时务必挂 HTTPS（例如用反向代理），不要把明文 token 暴露在公网。
- **服务端开启了宽松 CORS**（`Access-Control-Allow-Origin: *`），方便客户端从任意地址连；因为已经有 token 鉴权，这个取舍是可接受的。
- **文章正文直接渲染 RSS 里的 HTML**。客户端的 CSP 目前设为 `null`（关闭），后续若要收紧，需要对正文做白名单清洗。
- **设置存在浏览器 localStorage**，不跨设备同步——这正好符合「阅读状态在服务端、本机偏好在本机」的设计。
