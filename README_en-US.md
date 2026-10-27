[简体中文](README.md) | [繁體中文](README_zh-HK.md) | **English**

<!-- markdownlint-disable -->

<div align="center">

<img src="./src-tauri/icons/icon.png" width="120" alt="Renown Icon">

# Renown

A desktop organizer + sticky note app for Windows<br>
Built with Tauri 2 + Rust + React 19

[Report an Issue](https://github.com/Renown-72/Renown-Desktop/issues) · [Changelog](https://github.com/Renown-72/Renown-Desktop/releases) <br>
[Quick Start](#quick-start) · [Building from Source](#building-from-source)

[![Version](https://img.shields.io/github/v/release/Renown-72/Renown-Desktop)](https://github.com/Renown-72/Renown-Desktop/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![React 19](https://img.shields.io/badge/React-19-blue?logo=react)
![Tauri v2](https://img.shields.io/badge/Tauri-v2-%2324C8D8?logo=tauri)
![Rust Edition 2021](https://img.shields.io/badge/Rust-2021-%23000000?logo=rust)

</div>

<!-- markdownlint-restore -->

---

## What is Renown

Renown combines two things people usually need separate tools for: **keeping the
desktop tidy** and **taking quick notes**.

- **Sticky notes** — forked from [floral-notepaper](https://github.com/Achilng/floral-notepaper) (MIT).
  Markdown editing and preview, multiple windows, tray residency, three UI languages.
- **Desktop fences** — ported from [PecoFence](https://github.com/DayuanJiang/PecoFence) (Apache-2.0).
  Glass panels on the desktop, a rule engine that files items automatically, tabs,
  multi-monitor layouts, layout snapshots.

## Features

### Sticky notes

- **Markdown editing & preview** — GitHub Flavored Markdown with seamless toggling
- **Multiple windows** — pin notes to the desktop, keep them always on top
- **Tray residency** — global hotkey to summon, quick capture
- **Three languages** — Simplified Chinese, Traditional Chinese, English

### Desktop fences

- **Fence panels** — group desktop items into glass panels that sit above the
  wallpaper and below normal windows
- **Rule engine** — file items automatically by type, extension, name, glob,
  shortcut target, size, creation time, or idle days
- **Templates** — one click to create fences for programs, documents, images,
  music, video, archives, installers, downloads, cleanup
- **Tabs** — merge several fences into one window as tabs
- **Multi-monitor** — geometry stored relative to each monitor's work area, with
  a monitor fingerprint so layouts survive resolution and DPI changes
- **Layout snapshots** — save and restore the whole arrangement
- **Real shell icons** — extracted through `IShellItemImageFactory` and cached on disk
- **Optional icon hiding** — hide the native desktop icons so only fences show;
  restored automatically on exit

## Quick Start

```powershell
cd app
npm install
npm run tauri dev
```

Open **Settings → Desktop Organizer** to create your first fence.

## Building from Source

Requirements:

- Windows 10 2004+ / Windows 11
- Rust stable (MSVC toolchain) with Visual Studio Build Tools (C++ desktop + Windows SDK)
- Node.js 20+
- WebView2 Runtime

```powershell
cd app
npm install
npm run tauri build              # NSIS installer
npx tauri build --no-bundle      # exe only
```

Output: `app/src-tauri/target/release/`

## Testing

```powershell
cd app
npm test                                              # frontend unit tests
cargo test --manifest-path src-tauri/Cargo.toml       # backend tests
npx tsc --noEmit                                      # type check
```

## How the fences work

PecoFence does not manipulate `SysListView32` icon items. Instead it enumerates
the real desktop folders, hides the native icon layer, and draws its own panels.
That makes the backend logic portable, so Renown reuses it and replaces only the
Direct2D renderer with React components running in Tauri/WebView2.

Each fence is its own `WebviewWindow`, positioned from a `NormGeometry`
(DIP coordinates relative to the monitor work area) and inserted into the Win32
z-order directly above the desktop host and below normal windows.

See [app/README.md](app/README.md) for the module map and design decisions.

## Upstream attribution

- Sticky notes are based on [floral-notepaper](https://github.com/Achilng/floral-notepaper)
  (MIT License). Original copyright belongs to Achilng and that project's contributors.
- The desktop fence subsystem is ported from [PecoFence](https://github.com/DayuanJiang/PecoFence)
  (Apache License 2.0). Original copyright belongs to PecoFence contributors.

License and copyright notices are retained as required by MIT and Apache-2.0.
See [NOTICE](NOTICE) for the full attribution and a list of changes.

## License

MIT
