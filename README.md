# Renown

桌面整理 + 桌面便签二合一工具（Windows）。

> 本项目基于 [floral-notepaper](https://github.com/Achilng/floral-notepaper)（MIT License）分叉改造，
> 桌面整理（栅栏）子系统移植自 [PecoFence](https://github.com/DayuanJiang/PecoFence)（Apache-2.0）。
> 当前阶段：**可运行骨架** —— 品牌改造、构建链路、桌面整理核心功能均已完成。

---

## 当前状态

| 项目         | 状态                                           |
| ------------ | ---------------------------------------------- |
| 技术栈       | Tauri 2 + Rust + React 19 + TypeScript         |
| 前端构建     | ✅ `npm run build` 通过                        |
| 后端编译     | ✅ `cargo check` / `cargo test` 通过（229 项） |
| 前端测试     | ✅ `npm test` 通过（115 项）                   |
| 可执行文件   | ✅ `npx tauri build` 产出                      |
| 桌面整理功能 | ✅ 栅栏 / 规则引擎 / 标签页 / 多屏布局 / 快照  |

## 环境要求

- **Windows 10 2004+ / Windows 11**
- **Rust** stable（MSVC 工具链）；安装 Visual Studio Build Tools（含 C++ 桌面开发 + Windows SDK）
- **Node.js** 20+
- **WebView2 Runtime**（Win11 通常预装；Win10 需单独装）
- Tauri CLI（已通过 `@tauri-apps/cli` 作为 devDependency 提供，无需全局安装）

## 开发

```powershell
npm install           # 安装前端依赖
npm run tauri dev     # 启动开发模式（前端热更新 + Rust 热重载）
```

## 构建

```powershell
npm run tauri build              # 打包 NSIS 安装程序
npx tauri build --no-bundle      # 只产出 exe，不打包安装器
```

产物位置：`src-tauri/target/release/`

## 测试

```powershell
npm test                                              # 前端单元测试
cargo test --manifest-path src-tauri/Cargo.toml       # 后端测试
npx tsc --noEmit                                      # 类型检查
```

---

## 相比上游做了什么改动

### 1. 品牌与标识

| 位置                               | 改动                                                                        |
| ---------------------------------- | --------------------------------------------------------------------------- |
| `tauri.conf.json`                  | `productName` → `Renown`，`identifier` → `com.renown.desktop`，版本 `0.1.0` |
| `Cargo.toml`                       | 包名 `floral-notepaper` → `renown`，lib `renown_lib`                        |
| `package.json`                     | `name` → `renown`，`version` → `0.1.0`                                      |
| `index.html` / 窗口标题 / 托盘文案 | 三语统一为 `Renown`                                                         |
| `src/locales/*/translation.json`   | `about.productName`、`about.summary` 更新                                   |

### 2. ⚠️ 安全相关：切除上游的更新与分发链路

这部分**必须改**，否则你的用户会收到来自原作者服务器的更新，或把 bug 报给上游：

| 隐患                                          | 处理                                                                                                 |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 自更新器默认指向 `Achilng/floral-notepaper`   | `DEFAULT_GITHUB_REPO` 置空 + 空值直接返回错误；改用 `RENOWN_UPDATE_GITHUB_REPO` 环境变量指定自有仓库 |
| 默认自动检查更新                              | `auto_check` 默认值改为 `false`                                                                      |
| 默认更新源为 MirrorChyan（上游第三方分发）    | `CheckSourcePreference` / `DownloadSourcePreference` 默认改为 `GithubFirst`                          |
| 构建期拉取上游贡献者头像展示为"作者"          | `scripts/fetch-contributors.mjs` 不再自动请求，需显式设 `RENOWN_UPSTREAM_REPO`                       |
| "关于"页的 GitHub / 反馈 / 许可证链接指向上游 | 收敛到 `src/config/app.ts`，改一处即可                                                               |

### 3. 数据隔离

配置与数据目录从 `floral-notepaper` / `花笺` 改为 `Renown`，环境变量前缀 `FLORAL_NOTEPAPER_*` → `RENOWN_*`，避免与本机安装的原版应用互相覆盖数据。

### 4. 清理的商业化入口

移除内置提示中的 MirrorChyan 推广文案，保留纯功能提示。

### 5. 新增：桌面整理（栅栏）子系统

移植自 [PecoFence](https://github.com/DayuanJiang/PecoFence)（Apache-2.0），**只移植后端逻辑，UI 用 React 重写**。

#### 为什么可以移植

PecoFence 并不操作 `SysListView32` 的图标项，而是：

1. 直接枚举真实的桌面文件夹（用户桌面 + 公共桌面）；
2. 把原生图标层隐藏掉；
3. 自己画栅栏面板。

所以数据模型、规则引擎、图标提取、窗口层级这些逻辑与渲染层无关，可以整体搬到 Tauri 上。
只有 Direct2D 渲染层（`crates/render`）不可移植，由 React 组件替代。

#### 代码结构

| 文件                                  | 职责                                                           |
| ------------------------------------- | -------------------------------------------------------------- |
| `src-tauri/src/fence/model.rs`        | 栅栏 / 条目 / 布局 / 显示器指纹数据模型，`NormGeometry` 归一化 |
| `src-tauri/src/fence/rules.rs`        | 规则引擎：条件求值、优先级排序、模板（程序/文档/图片…）        |
| `src-tauri/src/fence/desktop.rs`      | 桌面文件夹枚举、条目合并、临时文件/系统文件过滤                |
| `src-tauri/src/fence/icons.rs`        | `IShellItemImageFactory` 提取图标 → PNG → 磁盘缓存             |
| `src-tauri/src/fence/monitor.rs`      | 显示器枚举、DPI、布局指纹匹配（多屏）                          |
| `src-tauri/src/fence/window.rs`       | Progman/WorkerW 发现、隐藏原生图标、z 序插入                   |
| `src-tauri/src/fence/shortcut.rs`     | `.lnk` / `.url` 目标解析（手写 MS-SHLLINK 二进制解析）         |
| `src-tauri/src/fence/store.rs`        | 配置读写（原子写入 + 损坏备份）                                |
| `src-tauri/src/fence/service.rs`      | 业务门面：同步、规则应用、栅栏 CRUD、标签页、快照              |
| `src-tauri/src/fence/commands.rs`     | 38 个 Tauri 命令                                               |
| `src-tauri/src/fence/windows.rs`      | 栅栏窗口的创建 / 定位 / 关闭                                   |
| `src/features/fence/FenceOverlay.tsx` | 单个栅栏窗口的 UI（图标网格 / 列表 / 详情、标签条、卷起）      |
| `src/features/fence/FenceManager.tsx` | 管理窗口（栅栏 / 规则 / 快照 / 设置四个页签）                  |

#### 关键设计决策

- **栅栏是独立窗口**，不是嵌在主窗口里。每个栅栏一个 `WebviewWindow`（`?view=fence&fenceId=…`），
  管理界面单独一个窗口（`?view=fence-manager`）。这样避免改动 3000+ 行的 `MainWindow.tsx`。
- **`hide_real_icons` 默认 `false`**。首次启动绝不能把用户桌面清空；用户显式开启后才隐藏，
  且 `fence::shutdown()` 无论配置如何都会恢复原生图标。
- **窗口类命令必须是 `async`**。Tauri 的同步命令在主线程执行，而 `WebviewWindowBuilder::build()`
  会把工作派发回主线程并等待——同步命令里调用它会直接死锁。
- **`AssignedBy` 区分 `User` / `Rule` / `Default`**。默认兜底归类不能标成 `User`，
  否则规则引擎永远无法接管这些条目。
- **配置变更用全局事件广播**：`FenceService::save()` 发出 `fence-changed`，
  所有窗口监听后自行刷新，不需要每个命令单独通知。

#### 已实现的交互

- **拖拽排序**：在「手动」排序下，图标可直接拖动改变顺序（网格按列插入，列表/详情按行插入，
  青色指示线标出落点）。顺序写入 `manualIndex` 持久化。非手动排序时给出提示，避免拖完被排序规则覆盖。
  纯计算逻辑抽在 `src/features/fence/reorder.ts`，有 10 个单测覆盖首尾/跨格/自身等边界。
- **双击重命名**：双击标题就地编辑，回车提交、Esc 取消。
- **条目右键菜单**：打开 / 移动到其它栅栏 / 移出栅栏。
- **栅栏右键菜单**：重命名 / 切换视图 / 卷起展开 / 删除（收件箱不可删）。

#### 已知限制

- 强制结束进程（任务管理器 / `Stop-Process -Force`）时 `shutdown()` 不会执行，
  若此前开启了「隐藏原生桌面图标」，桌面图标会保持隐藏，重启应用即恢复。
- 栅栏窗口关闭了 Tauri 的原生文件拖放拦截（`drag_and_drop(false)`），否则它会吞掉
  HTML5 拖放事件导致无法排序。**代价**：不能从资源管理器把文件直接拖进栅栏窗口；
  跨栅栏移动请用右键菜单的「移动到…」。
- 未实现 PecoFence 的 CLI、AI Agent 集成、portal 栅栏与 watchdog 进程。

---

## 仓库与上游同步

本项目托管在 **https://github.com/Renown-72/Renown-Desktop**，
是 [floral-notepaper](https://github.com/Achilng/floral-notepaper) 的 GitHub fork。

remote 约定：

| remote     | 地址                                              | 用途                 |
| ---------- | ------------------------------------------------- | -------------------- |
| `origin`   | `https://github.com/Renown-72/Renown-Desktop.git` | 本项目，日常提交推送 |
| `upstream` | `https://github.com/Achilng/floral-notepaper.git` | 上游，只读同步       |

同步上游更新：

```powershell
git fetch upstream
git merge upstream/main    # 或 git rebase upstream/main
```

> ⚠️ **同步上游时务必保留**上述第 2、3 节的改动。特别是
> `DEFAULT_GITHUB_REPO`、`auto_check` 默认值、`APP_ID`、数据目录名，
> 这些若被上游覆盖，会导致应用去拉上游的更新包、或与原版应用共用数据目录。
> 建议每次 merge 后运行 `npm test` 与 `cargo test`——相关断言已覆盖这些约束。

---

## 路线图

参照调研结论（见 `../research/00-SUMMARY.md`）：

1. **已完成**：fork + 品牌改造 + 构建链路验证
2. **已完成**：桌面整理（栅栏）子系统 —— 数据模型、规则引擎、图标提取、多屏布局、标签页、快照
3. 便签能力补强：SQLite 主存储、备份、多屏位置钳制
4. 栅栏交互打磨：拖拽排序、拖入/拖出、双击重命名、右键菜单完善
5. 布局快照自动轮换 + 显示器热插拔响应

## 许可证

MIT。原始版权归 Achilng 及 floral-notepaper 贡献者所有。

桌面整理子系统移植自 PecoFence（Apache-2.0），归属声明与改动说明见 [NOTICE](./NOTICE)。
其余第三方组件见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。
