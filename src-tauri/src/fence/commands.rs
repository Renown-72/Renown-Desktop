//! 栅栏相关的 Tauri 命令。
//!
//! 前端通过 `invoke("fence_*")` 调用；所有命令返回 `Result<T, String>`，
//! 错误信息直接可展示给用户。

use crate::fence::model::{Config, Fence, FenceView, Item, Snapshot};
use crate::fence::rules::{RuleSet, Template};
use crate::fence::service::{FenceService, SyncReport};
use crate::fence::{desktop, icons, monitor, window};
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

/// 命令统一错误类型。
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn parse_id(s: &str) -> Result<Uuid, String> {
    Uuid::parse_str(s).map_err(|_| format!("无效的 id: {s}"))
}

// ---------------------------------------------------------------------------
// 配置读取
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_get_config(service: State<'_, Arc<FenceService>>) -> Config {
    service.snapshot()
}

#[tauri::command]
pub fn fence_get_monitors() -> Vec<MonitorInfo> {
    monitor::enumerate()
        .into_iter()
        .map(|a| MonitorInfo {
            device_path: a.device_path.clone(),
            gdi_name: a.gdi_name.clone(),
            work_dip: [a.width_dip(), a.height_dip()],
            origin_dip: {
                let (x, y) = a.origin_dip();
                [x, y]
            },
            dpi: a.dpi,
            screen_px: a.screen_px,
        })
        .collect()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub device_path: String,
    pub gdi_name: String,
    pub work_dip: [f32; 2],
    pub origin_dip: [f32; 2],
    pub dpi: u32,
    pub screen_px: [i32; 2],
}

// ---------------------------------------------------------------------------
// 桌面同步
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_sync_desktop(service: State<'_, Arc<FenceService>>) -> Result<SyncReport, String> {
    let report = service.sync_desktop();
    service.save().map_err(err)?;
    Ok(report)
}

#[tauri::command]
pub fn fence_apply_rules(service: State<'_, Arc<FenceService>>) -> Result<usize, String> {
    let n = service.apply_rules();
    service.save().map_err(err)?;
    Ok(n)
}

// ---------------------------------------------------------------------------
// 栅栏 CRUD
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_create(
    service: State<'_, Arc<FenceService>>,
    title: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Result<Fence, String> {
    let fence = service.create_fence(&title, x, y, w, h);
    service.save().map_err(err)?;
    Ok(fence)
}

#[tauri::command]
pub fn fence_delete(service: State<'_, Arc<FenceService>>, id: String) -> Result<bool, String> {
    let ok = service.delete_fence(parse_id(&id)?);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_rename(
    service: State<'_, Arc<FenceService>>,
    id: String,
    title: String,
) -> Result<bool, String> {
    let ok = service.rename_fence(parse_id(&id)?, &title);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_set_geometry(
    service: State<'_, Arc<FenceService>>,
    id: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Result<bool, String> {
    let ok = service.set_geometry(parse_id(&id)?, x, y, w, h);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_set_rolled_up(
    service: State<'_, Arc<FenceService>>,
    id: String,
    rolled: bool,
) -> Result<bool, String> {
    let ok = service.set_rolled_up(parse_id(&id)?, rolled);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_set_view(
    service: State<'_, Arc<FenceService>>,
    id: String,
    view: FenceView,
) -> Result<bool, String> {
    let ok = service.set_view(parse_id(&id)?, view);
    service.save().map_err(err)?;
    Ok(ok)
}

// ---------------------------------------------------------------------------
// 条目归属
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_assign_item(
    service: State<'_, Arc<FenceService>>,
    item_id: String,
    fence_id: String,
) -> Result<bool, String> {
    let ok = service.assign_item(parse_id(&item_id)?, parse_id(&fence_id)?);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_unassign_item(
    service: State<'_, Arc<FenceService>>,
    item_id: String,
) -> Result<bool, String> {
    let ok = service.unassign_item(parse_id(&item_id)?);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_reorder_items(
    service: State<'_, Arc<FenceService>>,
    fence_id: String,
    ordered: Vec<String>,
) -> Result<bool, String> {
    let ids = ordered
        .iter()
        .map(|s| parse_id(s))
        .collect::<Result<Vec<_>, _>>()?;
    let ok = service.reorder_items(parse_id(&fence_id)?, ids);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_get_item(
    service: State<'_, Arc<FenceService>>,
    id: String,
) -> Result<Option<Item>, String> {
    Ok(service.item(parse_id(&id)?))
}

// ---------------------------------------------------------------------------
// 标签页
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_make_tab(
    service: State<'_, Arc<FenceService>>,
    fence_id: String,
    host_id: String,
) -> Result<bool, String> {
    let ok = service.make_tab(parse_id(&fence_id)?, parse_id(&host_id)?);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_split_tab(
    service: State<'_, Arc<FenceService>>,
    fence_id: String,
) -> Result<bool, String> {
    let ok = service.split_tab(parse_id(&fence_id)?);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_set_active_tab(
    service: State<'_, Arc<FenceService>>,
    host_id: String,
    tab_id: Option<String>,
) -> Result<bool, String> {
    let tab = match tab_id {
        Some(s) => Some(parse_id(&s)?),
        None => None,
    };
    let ok = service.set_active_tab(parse_id(&host_id)?, tab);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_set_tab_order(
    service: State<'_, Arc<FenceService>>,
    host_id: String,
    order: Vec<String>,
) -> Result<bool, String> {
    let ids = order
        .iter()
        .map(|s| parse_id(s))
        .collect::<Result<Vec<_>, _>>()?;
    let ok = service.set_tab_order(parse_id(&host_id)?, ids);
    service.save().map_err(err)?;
    Ok(ok)
}

// ---------------------------------------------------------------------------
// 规则
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_get_rules(service: State<'_, Arc<FenceService>>) -> RuleSet {
    service.rules()
}

#[tauri::command]
pub fn fence_set_rules(
    service: State<'_, Arc<FenceService>>,
    rules: RuleSet,
) -> Result<usize, String> {
    let n = service.set_rules(rules);
    service.save().map_err(err)?;
    Ok(n)
}

#[tauri::command]
pub fn fence_list_templates() -> Vec<TemplateInfo> {
    Template::all()
        .iter()
        .map(|t| TemplateInfo {
            key: t.key().to_string(),
            title: t.title().to_string(),
        })
        .collect()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub key: String,
    pub title: String,
}

#[tauri::command]
pub fn fence_apply_template(
    service: State<'_, Arc<FenceService>>,
    key: String,
    title: Option<String>,
) -> Result<Option<Fence>, String> {
    let template = Template::parse(&key).ok_or_else(|| format!("未知模板: {key}"))?;
    let fence = service.apply_template(template, title.as_deref());
    service.save().map_err(err)?;
    Ok(fence)
}

// ---------------------------------------------------------------------------
// 快照
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_save_snapshot(
    service: State<'_, Arc<FenceService>>,
    name: String,
) -> Result<Snapshot, String> {
    let snap = service.save_snapshot(&name);
    service.save().map_err(err)?;
    Ok(snap)
}

#[tauri::command]
pub fn fence_restore_snapshot(
    service: State<'_, Arc<FenceService>>,
    id: String,
) -> Result<bool, String> {
    let ok = service.restore_snapshot(parse_id(&id)?);
    service.save().map_err(err)?;
    Ok(ok)
}

#[tauri::command]
pub fn fence_delete_snapshot(
    service: State<'_, Arc<FenceService>>,
    id: String,
) -> Result<bool, String> {
    let ok = service.delete_snapshot(parse_id(&id)?);
    service.save().map_err(err)?;
    Ok(ok)
}

// ---------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_set_hide_real_icons(
    service: State<'_, Arc<FenceService>>,
    hide: bool,
) -> Result<bool, String> {
    service.set_hide_real_icons(hide);
    service.save().map_err(err)?;
    apply_icon_visibility(hide);
    Ok(true)
}

#[tauri::command]
pub fn fence_set_icon_size(
    service: State<'_, Arc<FenceService>>,
    size: u32,
) -> Result<bool, String> {
    service.set_icon_size(size);
    service.save().map_err(err)?;
    Ok(true)
}

/// 按配置隐藏/显示原生桌面图标。
pub fn apply_icon_visibility(hide: bool) {
    if let Some(host) = window::resolve_icon_host() {
        window::set_icons_visible(&host, !hide);
    }
}

// ---------------------------------------------------------------------------
// 图标
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_get_icon(
    service: State<'_, Arc<FenceService>>,
    item_id: String,
    size_px: Option<u32>,
) -> Result<Option<String>, String> {
    let id = parse_id(&item_id)?;
    let Some(item) = service.item(id) else {
        return Ok(None);
    };
    let Some(path) = item.path() else {
        return Ok(None);
    };
    let size = size_px.unwrap_or(48).clamp(16, 256);
    let Some(png) = icons::png_for(Path::new(path), &item.icon_key, size) else {
        return Ok(None);
    };
    Ok(Some(format!(
        "data:image/png;base64,{}",
        base64_encode(&png)
    )))
}

#[tauri::command]
pub fn fence_clear_icon_cache() -> Result<usize, String> {
    icons::clear_cache().map_err(err)
}

/// 极简 base64 编码（避免为一个小功能引入依赖）。
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

// ---------------------------------------------------------------------------
// 打开条目
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fence_open_item(
    service: State<'_, Arc<FenceService>>,
    item_id: String,
) -> Result<bool, String> {
    let id = parse_id(&item_id)?;
    let Some(item) = service.item(id) else {
        return Ok(false);
    };
    let Some(path) = item.path() else {
        return Ok(false);
    };
    open_path(path).map_err(err)?;
    Ok(true)
}

/// 用 shell 打开路径。
pub fn open_path(path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let verb = HSTRING::from("open");
        let file = HSTRING::from(path);
        let ret = unsafe {
            ShellExecuteW(
                None,
                &verb,
                &file,
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecuteW 返回值 > 32 表示成功
        if ret.0 as isize > 32 {
            Ok(())
        } else {
            Err(format!("打开失败: {}", path))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// 窗口层
// ---------------------------------------------------------------------------

/// 把某个栅栏窗口放到桌面之上、普通窗口之下。
///
/// 注意：所有涉及窗口创建/关闭的命令都必须是 `async`。
/// Tauri 的同步命令在主线程执行，而 `WebviewWindowBuilder::build()` 会
/// 把工作派发回主线程并等待，同步命令里调用它会直接死锁。
#[tauri::command]
pub async fn fence_place_window(app: AppHandle, label: String) -> Result<bool, String> {
    let Some(win) = app.get_webview_window(&label) else {
        return Ok(false);
    };
    let hwnd = win.hwnd().map_err(err)?;
    let Some(host) = window::resolve_icon_host() else {
        return Ok(false);
    };
    Ok(window::place_above_desktop(hwnd.0 as isize, &host))
}

/// 设置栅栏窗口置顶。
#[tauri::command]
pub async fn fence_set_window_topmost(
    app: AppHandle,
    label: String,
    on: bool,
) -> Result<bool, String> {
    let Some(win) = app.get_webview_window(&label) else {
        return Ok(false);
    };
    let hwnd = win.hwnd().map_err(err)?;
    Ok(window::set_topmost(hwnd.0 as isize, on))
}

/// 桌面图标宿主是否可用（前端据此提示「桌面整理不可用」）。
#[tauri::command]
pub fn fence_desktop_available() -> DesktopStatus {
    let host = window::resolve_icon_host();
    DesktopStatus {
        available: host.is_some(),
        icons_visible: host.as_ref().map(window::icons_visible).unwrap_or(false),
        desktop_path: desktop::user_desktop().map(|p| p.to_string_lossy().to_string()),
    }
}

// ---------------------------------------------------------------------------
// 窗口生命周期
// ---------------------------------------------------------------------------

/// 打开桌面整理管理窗口。
#[tauri::command]
pub async fn fence_open_manager(app: AppHandle) -> Result<bool, String> {
    Ok(crate::fence::windows::open_manager(&app).is_some())
}

/// 按当前布局重建/更新所有栅栏窗口。
#[tauri::command]
pub async fn fence_sync_windows(
    app: AppHandle,
    service: State<'_, Arc<FenceService>>,
) -> Result<usize, String> {
    let layout = service.active_layout();
    let count = layout
        .fences
        .iter()
        .filter(|f| f.tab_host.is_none())
        .count();
    crate::fence::windows::sync_layout(&app, &layout);
    Ok(count)
}

/// 单独打开某个栅栏窗口。
#[tauri::command]
pub async fn fence_open_window(
    app: AppHandle,
    service: State<'_, Arc<FenceService>>,
    id: String,
) -> Result<bool, String> {
    let id = parse_id(&id)?;
    let layout = service.active_layout();
    let Some(fence) = layout.fences.iter().find(|f| f.id == id) else {
        return Ok(false);
    };
    let areas = monitor::enumerate();
    Ok(crate::fence::windows::open_fence(&app, fence, &areas).is_some())
}

/// 关闭某个栅栏窗口。
#[tauri::command]
pub async fn fence_close_window(app: AppHandle, id: String) -> Result<bool, String> {
    let id = parse_id(&id)?;
    crate::fence::windows::close_fence(&app, id);
    Ok(true)
}

/// 把指定窗口重新压回桌面层（窗口失焦或被顶上来后调用）。
#[tauri::command]
pub async fn fence_apply_desktop_layer(app: AppHandle, label: String) -> Result<bool, String> {
    crate::fence::windows::apply_desktop_layer(&app, &label);
    Ok(true)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStatus {
    pub available: bool,
    pub icons_visible: bool,
    pub desktop_path: Option<String>,
}

// ---------------------------------------------------------------------------
// 命令注册
// ---------------------------------------------------------------------------

/// 生成 `tauri::generate_handler!` 需要的命令列表。
#[macro_export]
macro_rules! fence_commands {
    () => {
        $crate::fence::commands::fence_get_config,
        $crate::fence::commands::fence_get_monitors,
        $crate::fence::commands::fence_sync_desktop,
        $crate::fence::commands::fence_apply_rules,
        $crate::fence::commands::fence_create,
        $crate::fence::commands::fence_delete,
        $crate::fence::commands::fence_rename,
        $crate::fence::commands::fence_set_geometry,
        $crate::fence::commands::fence_set_rolled_up,
        $crate::fence::commands::fence_set_view,
        $crate::fence::commands::fence_assign_item,
        $crate::fence::commands::fence_unassign_item,
        $crate::fence::commands::fence_reorder_items,
        $crate::fence::commands::fence_get_item,
        $crate::fence::commands::fence_make_tab,
        $crate::fence::commands::fence_split_tab,
        $crate::fence::commands::fence_set_active_tab,
        $crate::fence::commands::fence_set_tab_order,
        $crate::fence::commands::fence_get_rules,
        $crate::fence::commands::fence_set_rules,
        $crate::fence::commands::fence_list_templates,
        $crate::fence::commands::fence_apply_template,
        $crate::fence::commands::fence_save_snapshot,
        $crate::fence::commands::fence_restore_snapshot,
        $crate::fence::commands::fence_delete_snapshot,
        $crate::fence::commands::fence_set_hide_real_icons,
        $crate::fence::commands::fence_set_icon_size,
        $crate::fence::commands::fence_get_icon,
        $crate::fence::commands::fence_clear_icon_cache,
        $crate::fence::commands::fence_open_item,
        $crate::fence::commands::fence_place_window,
        $crate::fence::commands::fence_set_window_topmost,
        $crate::fence::commands::fence_desktop_available,
        $crate::fence::commands::fence_open_manager,
        $crate::fence::commands::fence_sync_windows,
        $crate::fence::commands::fence_open_window,
        $crate::fence::commands::fence_close_window,
        $crate::fence::commands::fence_apply_desktop_layer,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_handles_binary() {
        let data: Vec<u8> = (0..=255).collect();
        let encoded = base64_encode(&data);
        assert_eq!(encoded.len(), 344);
        assert!(encoded
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='));
    }

    #[test]
    fn parse_id_rejects_garbage() {
        assert!(parse_id("not-a-uuid").is_err());
        assert!(parse_id(&Uuid::new_v4().to_string()).is_ok());
    }
}
