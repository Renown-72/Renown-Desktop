//! 栅栏窗口生命周期管理。
//!
//! 每个栅栏对应一个独立的透明无边框窗口（标签页宿主除外），
//! 窗口创建后会被塞到桌面之上、普通窗口之下，并加上
//! `WS_EX_NOACTIVATE` 使其不抢焦点。

use crate::fence::model::{Fence, FenceId, Layout};
use crate::fence::monitor;
use crate::fence::window;
use std::collections::HashSet;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// 栅栏窗口的 label 前缀。
pub const FENCE_LABEL_PREFIX: &str = "fence-";
/// 管理窗口 label。
pub const MANAGER_LABEL: &str = "fence-manager";

/// 栅栏 id → 窗口 label。
pub fn fence_label(id: FenceId) -> String {
    format!("{FENCE_LABEL_PREFIX}{id}")
}

/// 从窗口 label 反解栅栏 id。
pub fn label_to_id(label: &str) -> Option<FenceId> {
    label
        .strip_prefix(FENCE_LABEL_PREFIX)
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
}

/// 栅栏在屏幕上的逻辑坐标（DIP，虚拟屏幕坐标系）。
///
/// 返回 `(x, y, w, h)`；找不到显示器时回落到主显示器。
pub fn screen_rect(fence: &Fence, areas: &[monitor::WorkArea]) -> (f64, f64, f64, f64) {
    let area = monitor::find(areas, &fence.geometry.monitor)
        .or_else(|| monitor::primary(areas))
        .cloned();
    let (work_w, work_h, origin) = match &area {
        Some(a) => (a.width_dip(), a.height_dip(), a.origin_dip()),
        None => (1920.0, 1080.0, (0.0, 0.0)),
    };
    let (x, y, w, h) = fence.geometry.resolve(work_w, work_h);
    // 卷起时只留标题栏高度。
    let h = if fence.rolled_up { 34.0 } else { h };
    (
        (origin.0 + x) as f64,
        (origin.1 + y) as f64,
        w.max(120.0) as f64,
        h.max(34.0) as f64,
    )
}

/// 创建或更新一个栅栏窗口。
pub fn open_fence(app: &AppHandle, fence: &Fence, areas: &[monitor::WorkArea]) -> Option<String> {
    let label = fence_label(fence.id);
    let (x, y, w, h) = screen_rect(fence, areas);

    if let Some(win) = app.get_webview_window(&label) {
        // 已存在：只更新位置与尺寸。
        let _ = win.set_position(tauri::LogicalPosition::new(x, y));
        let _ = win.set_size(tauri::LogicalSize::new(w, h));
        let _ = win.show();
        apply_desktop_layer(app, &label);
        return Some(label);
    }

    let url = format!("index.html?view=fence&fenceId={}", fence.id);
    let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(&fence.title)
        .inner_size(w, h)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        // 关闭 Tauri 的原生文件拖放拦截，否则它会吞掉页面内的 HTML5 拖放事件，
        // 栅栏内的图标拖拽排序就完全失效。代价是不能再从资源管理器把文件
        // 直接拖进栅栏窗口（跨栅栏移动仍可用右键菜单）。
        .drag_and_drop(false)
        .build();

    match built {
        Ok(win) => {
            let _ = win.show();
            apply_desktop_layer(app, &label);
            Some(label)
        }
        Err(err) => {
            eprintln!("[fence] 创建栅栏窗口失败 {label}: {err}");
            None
        }
    }
}

/// 把窗口放到桌面层并禁止抢焦点。
pub fn apply_desktop_layer(app: &AppHandle, label: &str) {
    let Some(win) = app.get_webview_window(label) else {
        return;
    };
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    let raw = hwnd.0 as isize;
    window::set_no_activate(raw);
    if let Some(host) = window::resolve_icon_host() {
        window::place_above_desktop(raw, &host);
    }
}

/// 关闭一个栅栏窗口。
pub fn close_fence(app: &AppHandle, id: FenceId) {
    let label = fence_label(id);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.close();
    }
}

/// 按布局同步窗口集合：该有的建出来，多余的关掉。
///
/// 标签页里的栅栏没有自己的窗口（由宿主绘制），所以跳过。
pub fn sync_layout(app: &AppHandle, layout: &Layout) {
    let areas = monitor::enumerate();
    let mut wanted: HashSet<FenceId> = HashSet::new();

    for fence in &layout.fences {
        if fence.tab_host.is_some() {
            continue;
        }
        if fence.hide_when_empty && fence.items.is_empty() {
            close_fence(app, fence.id);
            continue;
        }
        wanted.insert(fence.id);
        open_fence(app, fence, &areas);
    }

    // 关掉不再需要的窗口
    for (label, win) in app.webview_windows() {
        if let Some(id) = label_to_id(&label) {
            if !wanted.contains(&id) {
                let _ = win.close();
            }
        }
    }
}

/// 关闭所有栅栏窗口。
pub fn close_all(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if label_to_id(&label).is_some() {
            let _ = win.close();
        }
    }
}

/// 打开（或聚焦）桌面整理管理窗口。
pub fn open_manager(app: &AppHandle) -> Option<String> {
    if let Some(win) = app.get_webview_window(MANAGER_LABEL) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return Some(MANAGER_LABEL.to_string());
    }
    let built = WebviewWindowBuilder::new(
        app,
        MANAGER_LABEL,
        WebviewUrl::App("index.html?view=fence-manager".into()),
    )
    .title("桌面整理")
    .inner_size(980.0, 720.0)
    .min_inner_size(720.0, 520.0)
    .center()
    .build();
    match built {
        Ok(win) => {
            let _ = win.show();
            let _ = win.set_focus();
            Some(MANAGER_LABEL.to_string())
        }
        Err(err) => {
            eprintln!("[fence] 创建管理窗口失败: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fence::model::new_fence;

    #[test]
    fn label_roundtrips() {
        let id = uuid::Uuid::new_v4();
        let label = fence_label(id);
        assert_eq!(label_to_id(&label), Some(id));
    }

    #[test]
    fn label_parser_rejects_foreign_labels() {
        assert_eq!(label_to_id("main"), None);
        assert_eq!(label_to_id("fence-not-a-uuid"), None);
    }

    #[test]
    fn screen_rect_falls_back_without_monitors() {
        let fence = new_fence("测试", "DISPLAY1", 100.0, 50.0, 1920.0, 1080.0);
        let (x, y, w, h) = screen_rect(&fence, &[]);
        assert!(x >= 0.0 && y >= 0.0);
        assert!(w >= 120.0 && h >= 34.0);
    }

    #[test]
    fn rolled_up_fence_is_title_bar_height() {
        let mut fence = new_fence("测试", "DISPLAY1", 100.0, 50.0, 1920.0, 1080.0);
        fence.rolled_up = true;
        let (_, _, _, h) = screen_rect(&fence, &[]);
        assert!((h - 34.0).abs() < 0.01);
    }
}
