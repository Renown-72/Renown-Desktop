[简体中文](README.md) | **繁體中文** | [English](README_en-US.md)

<!-- markdownlint-disable -->

<div align="center">

<img src="./src-tauri/icons/icon.png" width="120" alt="Renown 圖示">

# Renown

桌面整理 + 桌面便箋二合一工具（Windows）<br>
以 Tauri 2 + Rust + React 19 打造

[回報問題](https://github.com/Renown-72/Renown-Desktop/issues) · [更新日誌](https://github.com/Renown-72/Renown-Desktop/releases) <br>
[快速開始](#快速開始) · [從原始碼建置](#從原始碼建置)

[![Version](https://img.shields.io/github/v/release/Renown-72/Renown-Desktop)](https://github.com/Renown-72/Renown-Desktop/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![React 19](https://img.shields.io/badge/React-19-blue?logo=react)
![Tauri v2](https://img.shields.io/badge/Tauri-v2-%2324C8D8?logo=tauri)
![Rust Edition 2021](https://img.shields.io/badge/Rust-2021-%23000000?logo=rust)

</div>

<!-- markdownlint-restore -->

---

## Renown 是什麼

Renown 把兩件通常需要兩套工具才能完成的事合在一起：**整理桌面** 與 **隨手記便箋**。

- **桌面便箋** —— 基於 [floral-notepaper](https://github.com/Achilng/floral-notepaper)（MIT）分叉。
  Markdown 編輯與預覽、多視窗、常駐系統匣、三語介面。
- **桌面整理（柵欄）** —— 移植自 [PecoFence](https://github.com/DayuanJiang/PecoFence)（Apache-2.0）。
  桌面玻璃面板、自動歸類的規則引擎、標籤頁、多螢幕佈局、佈局快照。

## 功能

### 桌面便箋

- **Markdown 編輯與預覽** —— 完整 GitHub Flavored Markdown，編輯/預覽無縫切換
- **多視窗** —— 便箋可釘在桌面、可置頂
- **常駐系統匣** —— 全域快速鍵喚出、快速記錄
- **三種語言** —— 簡體中文、繁體中文、English

### 桌面整理（柵欄）

- **柵欄面板** —— 把桌面項目分組放進玻璃面板，位於桌布之上、一般視窗之下
- **規則引擎** —— 依類型、副檔名、名稱、glob、捷徑目標、大小、建立時間、閒置天數自動歸類
- **快速範本** —— 一鍵建立程式、文件、圖片、音樂、影片、壓縮檔、安裝檔、下載、待清理等柵欄
- **標籤頁** —— 把多個柵欄合併成同一視窗的分頁
- **多螢幕** —— 幾何座標相對於各螢幕工作區儲存，並以螢幕指紋匹配，解析度與 DPI 變更後仍能還原
- **佈局快照** —— 儲存與還原整套排列
- **真實系統圖示** —— 透過 `IShellItemImageFactory` 擷取並快取到磁碟
- **可選隱藏原生圖示** —— 隱藏桌面原生圖示只留柵欄；結束程式時自動還原

## 快速開始

```powershell
cd app
npm install
npm run tauri dev
```

開啟 **設定 → 桌面整理** 即可建立第一個柵欄。

## 從原始碼建置

環境需求：

- Windows 10 2004+ / Windows 11
- Rust stable（MSVC 工具鏈），並安裝 Visual Studio Build Tools（C++ 桌面開發 + Windows SDK）
- Node.js 20+
- WebView2 Runtime

```powershell
cd app
npm install
npm run tauri build              # 打包 NSIS 安裝程式
npx tauri build --no-bundle      # 只產出 exe
```

產物位置：`app/src-tauri/target/release/`

## 測試

```powershell
cd app
npm test                                              # 前端單元測試
cargo test --manifest-path src-tauri/Cargo.toml       # 後端測試
npx tsc --noEmit                                      # 型別檢查
```

## 柵欄是怎麼運作的

PecoFence 並不操作 `SysListView32` 的圖示項目，而是直接列舉真實的桌面資料夾、
隱藏原生圖示層、再自己繪製面板。因此後端邏輯可以移植——Renown 沿用這部分，
只把 Direct2D 渲染層換成執行在 Tauri/WebView2 裡的 React 元件。

每個柵欄都是一個獨立的 `WebviewWindow`，依 `NormGeometry`（相對於螢幕工作區的
DIP 座標）定位，並插入 Win32 z 序中「桌面之上、一般視窗之下」的位置。

模組對照與設計取捨見 [app/README.md](app/README.md)。

## 上游歸屬

- 桌面便箋基於 [floral-notepaper](https://github.com/Achilng/floral-notepaper)（MIT License），
  原始版權歸 Achilng 及該專案貢獻者所有。
- 桌面整理子系統移植自 [PecoFence](https://github.com/DayuanJiang/PecoFence)（Apache License 2.0），
  原始版權歸 PecoFence contributors 所有。

已依 MIT 與 Apache-2.0 要求保留授權與版權聲明，完整歸屬與改動說明見 [NOTICE](NOTICE)。

## 授權

MIT
