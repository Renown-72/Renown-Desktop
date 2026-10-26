# Renown

桌面整理 + 桌面便签二合一工具（Windows）。

> 本项目基于 [floral-notepaper](https://github.com/Achilng/floral-notepaper)（MIT License）分叉改造。
> 当前阶段：**可运行骨架** —— 已完成品牌改造、构建链路验证，桌面整理功能待叠加。

---

## 当前状态

| 项目         | 状态                                           |
| ------------ | ---------------------------------------------- |
| 技术栈       | Tauri 2 + Rust + React 19 + TypeScript         |
| 前端构建     | ✅ `npm run build` 通过                        |
| 后端编译     | ✅ `cargo check` / `cargo test` 通过（148 项） |
| 前端测试     | ✅ `npm test` 通过（110 项）                   |
| 可执行文件   | ✅ `npx tauri build` 产出                      |
| 桌面整理功能 | ⬜ 待开发（见路线图）                          |

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

---

## 仓库与上游同步

本项目托管在 **https://github.com/Renown-72/Renown-Desktop**，
是 [floral-notepaper](https://github.com/Achilng/floral-notepaper) 的 GitHub fork。

remote 约定：

| remote | 地址 | 用途 |
|---|---|---|
| `origin` | `https://github.com/Renown-72/Renown-Desktop.git` | 本项目，日常提交推送 |
| `upstream` | `https://github.com/Achilng/floral-notepaper.git` | 上游，只读同步 |

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

1. **本阶段（已完成）**：fork + 品牌改造 + 构建链路验证
2. 便签能力补强：SQLite 主存储、备份、多屏位置钳制
3. 桌面图标层：先做**只读 + 保存/恢复布局**（风险最低）
4. 布局快照 + 显示器指纹多套布局
5. 收纳盒 → 栅栏分区 → 覆盖层自绘

## 许可证

MIT。原始版权归 Achilng 及 floral-notepaper 贡献者所有，详见 [LICENSE](./LICENSE) 与 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。
