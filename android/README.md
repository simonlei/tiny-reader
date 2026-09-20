# Tiny Reader for Android

Tiny Reader 的安卓客户端，用 **Kotlin + Jetpack Compose** 原生实现。

和桌面端（Tauri + Vue）一样，它本身**不存任何业务数据**，所有订阅源、文章、已读 / 星标状态都来自同一个
[Rust 服务端](../server)。手机和电脑填同一个地址与 token，阅读进度自动共享。

```
┌──────────────────────┐                                  ┌──────────────────────┐
│ Android (Compose)    │ ──┐                          ┌── │ Tauri 2 + Vue 3      │
└──────────────────────┘   │   HTTP (Bearer token)    │    └──────────────────────┘
                           ├──────────────────────────┤
                           │                          │
                    ┌──────▼──────────────────────────▼──────┐
                    │   Rust 服务端 (Axum + SQLite)           │
                    └────────────────────────────────────────┘
```

## 功能对照

与桌面端保持一致的行为：

| 功能 | 实现 |
| --- | --- |
| 读取未读文章 | 默认过滤器就是「未读」，`GET /api/articles?unread_only=true` |
| 点开 0.6 秒后自动标记已读 | `ReaderViewModel.scheduleAutoRead()`，延迟 600ms；列表**不移除**该条，方便回看 |
| 左右滑动切换上 / 下一篇 | 阅读页横向滑动 → `move(+1)` / `move(-1)`，首尾夹住不循环，滑到末尾自动加载下一页 |
| 切换订阅源过滤 | 左上角菜单打开抽屉，按 `category` 分组、可折叠、显示未读数 |
| 全部 / 未读 / 星标 | 列表顶部三个筛选标签 |
| 搜索 | 顶栏放大镜，回车提交（`q` 参数，服务端匹配标题与摘要） |
| 分页 | 每页 50 条，滚动接近底部自动加载 |
| 刷新所有源 | 顶栏刷新按钮或**下拉刷新**，带进度条（轮询 `/api/refresh/status`） |
| 星标 / 已读手动切换 | 阅读页顶部按钮 |
| 当前范围全部已读 | 顶栏 ✓✓ 按钮 |
| 订阅源增删改 | 抽屉里 ＋ 新增；**长按**订阅源编辑 / 删除 / 单独刷新 |
| OPML 导入导出 | 抽屉底部「导入 / 导出」 |
| 在浏览器打开原文 | 阅读页按钮，或点正文里的链接 / 标题 |

桌面端独有、移动端不适用的部分：键盘快捷键（`j`/`k` 等）与 Tauri 自动更新。

## 目录结构

```
android/app/src/main/java/com/simonlei/tinyreader/
├─ MainActivity.kt          # 入口 + 抽屉/列表/阅读/设置的路由
├─ TinyReaderApp.kt         # Application，初始化偏好存储
├─ data/
│  ├─ Models.kt             # DTO，对应 src/api/types.ts
│  ├─ ApiClient.kt          # OkHttp 客户端，对应 src/api/client.ts
│  └─ SettingsStore.kt      # 地址 + token，对应 src/stores/settings.ts
├─ ui/
│  ├─ ReaderViewModel.kt    # 状态机，对应 src/stores/app.ts
│  ├─ ArticleListScreen.kt  # 对应 ArticleList.vue + App.vue 顶栏
│  ├─ ReaderScreen.kt       # 对应 ArticleView.vue（+ 滑动切换）
│  ├─ ArticleWebView.kt     # 正文 WebView，自己识别横向手势
│  ├─ FeedDrawer.kt         # 对应 Sidebar.vue
│  ├─ SettingsScreen.kt     # 对应 SettingsDialog.vue
│  ├─ FeedEditDialog.kt     # 对应 FeedDialog.vue
│  └─ theme/Theme.kt        # 取自 src/styles/main.css 的暗色配色
└─ util/
   ├─ Format.kt             # 对应 src/lib/format.ts
   ├─ Html.kt               # 对应 src/lib/html.ts + 正文页面模板
   └─ External.kt           # 对应 src/lib/tauri.ts 的 openExternal
```

## 构建

需要 JDK 17 与 Android SDK（compileSdk 35）。

```bash
cd android
./gradlew assembleDebug            # 产物：app/build/outputs/apk/debug/app-debug.apk
./gradlew installDebug             # 装到已连接的设备
./gradlew assembleRelease          # 未签名的 release 包
```

`local.properties` 里的 `sdk.dir` 指向本机 SDK；用 Android Studio 直接打开 `android/` 目录也可以。

## 连接服务端

1. 启动服务端，并把 `server/config.toml` 里的 `server.host` 改成 `0.0.0.0`，否则手机连不上：

   ```toml
   [server]
   host = "0.0.0.0"
   port = 8787
   ```

2. 查一下电脑的局域网 IP（Windows：`ipconfig`）。

3. 首次打开 App 会停在「服务端设置」：
   - **服务端地址**：`http://192.168.x.x:8787`（**不能填 `127.0.0.1`**，那是手机自己）
   - **访问 token**：`config.toml` 里 `auth.token` 的值

4. 点「测试连接」，成功后保存。

> 服务端是明文 HTTP，App 已通过 `network_security_config.xml` 允许明文流量。
> 如果要走公网，请在服务端前面挂 HTTPS 反向代理。

## 实现说明：为什么正文 WebView 要自定义

文章正文是 RSS 里的 HTML（含图片、代码块），只能用 WebView 渲染。但 WebView 会吞掉所有触摸事件，
直接套一层 Compose 的 `HorizontalPager` 收不到左右滑动。

所以 `ArticleWebView` 自己判断手势：横向位移超过阈值且明显大于纵向时，给 WebView 补发一个
`ACTION_CANCEL` 并接管事件，回调 `onSwipeNext` / `onSwipePrev`；否则事件照常交给 WebView 做竖向滚动。
文章标题、来源、时间也一起渲染进 HTML，保证整页只有一个垂直滚动容器，不存在嵌套滚动冲突。
