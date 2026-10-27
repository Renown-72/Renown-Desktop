//! 桌面窗口层操作：定位图标宿主、隐藏原生图标、把栅栏窗口插到桌面之上。
//!
//! 移植自 PecoFence（Apache-2.0）的 `crates/platform/src/desktop.rs`。
//!
//! 核心思路：栅栏窗口是**顶层窗口**，通过 `SetWindowPos` 插到
//! 「桌面图标宿主窗口」的正上方，于是它压在壁纸与图标之上、普通应用窗口之下，
//! 表现得就像桌面的一部分。

use std::ffi::c_void;

/// 桌面图标宿主。
#[derive(Clone, Copy, Debug)]
pub struct IconHost {
    /// 承载桌面内容的顶层窗口（`Progman` 或某个 `WorkerW`）。
    pub host: *mut c_void,
    /// `SHELLDLL_DefView` 子窗口；隐藏它就等于隐藏桌面图标。
    pub def_view: *mut c_void,
}

impl IconHost {
    pub fn is_valid(&self) -> bool {
        !self.host.is_null()
    }
}

// ---------------------------------------------------------------------------
// Windows 实现
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod win {
    use super::IconHost;
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowExW, FindWindowW, GetAncestor, GetClassNameW, GetWindow,
        GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        SetWindowLongW, SetWindowPos, ShowWindow, GA_ROOT, GWL_EXSTYLE, GW_HWNDNEXT, GW_HWNDPREV,
        HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOOWNERZORDER, SWP_NOSIZE, SW_HIDE, SW_SHOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST,
    };

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 窗口类名。
    pub fn class_name(h: HWND) -> String {
        if h.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 256];
        let n = unsafe { GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }

    /// 窗口所属进程 id。
    pub fn window_pid(h: HWND) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, &mut pid) };
        pid
    }

    /// 查找子窗口。
    pub fn find_child(parent: HWND, class: &str) -> HWND {
        let class = to_wide(class);
        unsafe {
            FindWindowExW(
                parent,
                std::ptr::null_mut(),
                class.as_ptr(),
                std::ptr::null(),
            )
        }
    }

    /// 桌面（Progman）窗口。
    pub fn shell_window() -> HWND {
        let class = to_wide("Progman");
        unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) }
    }

    pub fn is_visible(h: HWND) -> bool {
        unsafe { IsWindowVisible(h) != 0 }
    }

    pub fn is_iconic(h: HWND) -> bool {
        unsafe { IsIconic(h) != 0 }
    }

    /// 窗口是否被 DWM 隐藏（虚拟桌面切换、UWP 挂起等）。
    pub fn is_cloaked(h: HWND) -> bool {
        let mut cloaked = 0u32;
        let hr = unsafe {
            DwmGetWindowAttribute(
                h,
                DWMWA_CLOAKED as u32,
                &mut cloaked as *mut u32 as *mut c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        hr == 0 && cloaked != 0
    }

    pub fn root_ancestor(h: HWND) -> HWND {
        unsafe { GetAncestor(h, GA_ROOT) }
    }

    pub fn is_topmost(h: HWND) -> bool {
        let ex = unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32;
        ex & WS_EX_TOPMOST != 0
    }

    /// 一个外部窗口是否真的能遮挡桌面。最小化的终端、零尺寸伪控制台宿主
    /// 可能仍保留 WS_VISIBLE。
    pub fn has_visible_surface(h: HWND) -> bool {
        if !is_visible(h) || is_iconic(h) || is_cloaked(h) {
            return false;
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let ok = unsafe { GetWindowRect(h, &mut rect) } != 0;
        ok && rect.right > rect.left && rect.bottom > rect.top
    }

    struct EnumCtx {
        pid: u32,
        out: Vec<HWND>,
    }

    unsafe extern "system" fn enum_cb(h: HWND, lparam: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(lparam as *mut EnumCtx) };
        if window_pid(h) == ctx.pid {
            ctx.out.push(h);
        }
        TRUE
    }

    /// 某进程的所有顶层窗口。
    pub fn top_level_windows_of(pid: u32) -> Vec<HWND> {
        let mut ctx = EnumCtx {
            pid,
            out: Vec::new(),
        };
        unsafe {
            EnumWindows(Some(enum_cb), &mut ctx as *mut EnumCtx as LPARAM);
        }
        ctx.out
    }

    /// 定位桌面图标宿主。
    ///
    /// 现代 Windows（24H2+）图标在 `Progman` 下；旧版在某个 `WorkerW` 下。
    pub fn resolve_icon_host() -> Option<IconHost> {
        let progman = shell_window();
        if progman.is_null() || class_name(progman) != "Progman" {
            return None;
        }
        let def_view_under_progman = find_child(progman, "SHELLDLL_DefView");
        if !def_view_under_progman.is_null() {
            return Some(IconHost {
                host: progman,
                def_view: def_view_under_progman,
            });
        }

        // 旧版：Progman 下没有 DefView，去同进程的 WorkerW 里找。
        let explorer_pid = window_pid(progman);
        for h in top_level_windows_of(explorer_pid) {
            if class_name(h) != "WorkerW" {
                continue;
            }
            let def_view = find_child(h, "SHELLDLL_DefView");
            if !def_view.is_null() {
                return Some(IconHost { host: h, def_view });
            }
        }

        // 兜底：直接找 FolderView 的列表视图，取其根窗口。
        let list_class = to_wide("SysListView32");
        let list_title = to_wide("FolderView");
        let list_view = unsafe {
            FindWindowExW(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                list_class.as_ptr(),
                list_title.as_ptr(),
            )
        };
        if !list_view.is_null() {
            let root = root_ancestor(list_view);
            let def_view = find_child(root, "SHELLDLL_DefView");
            return Some(IconHost {
                host: root,
                def_view,
            });
        }
        None
    }

    const ANCHOR_FLAGS: u32 = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER;

    /// `host` 上方最低的、真正绘制表面的窗口。
    fn surface_window_above(host: HWND, self_hwnd: HWND) -> HWND {
        let mut above = unsafe { GetWindow(host, GW_HWNDPREV) };
        while !above.is_null() && above != self_hwnd && !has_visible_surface(above) {
            above = unsafe { GetWindow(above, GW_HWNDPREV) };
        }
        above
    }

    /// 把 `h` 插到 `host` 正上方：压在桌面之上、普通窗口之下。
    pub fn insert_above(h: HWND, host: HWND) -> bool {
        let above = surface_window_above(host, h);
        if above == h {
            return true;
        }
        let insert_after = if above.is_null() || (is_topmost(above) && !is_topmost(host)) {
            HWND_TOP
        } else {
            above
        };
        let ok = unsafe { SetWindowPos(h, insert_after, 0, 0, 0, 0, ANCHOR_FLAGS) } != 0;
        if ok {
            return true;
        }
        // UIPI 拒绝（上方是管理员进程窗口）时，从底部重新锚定。
        let next = unsafe { GetWindow(host, GW_HWNDNEXT) };
        if next.is_null() {
            unsafe {
                SetWindowPos(h, HWND_BOTTOM, 0, 0, 0, 0, ANCHOR_FLAGS);
                SetWindowPos(host, HWND_BOTTOM, 0, 0, 0, 0, ANCHOR_FLAGS);
            }
            return true;
        }
        false
    }

    /// 显示/隐藏原生桌面图标。
    pub fn set_icons_visible(host: &IconHost, visible: bool) -> bool {
        if host.def_view.is_null() {
            return false;
        }
        let cmd = if visible { SW_SHOW } else { SW_HIDE };
        unsafe { ShowWindow(host.def_view, cmd) != 0 }
    }

    /// 原生桌面图标当前是否可见。
    pub fn icons_visible(host: &IconHost) -> bool {
        if host.def_view.is_null() {
            return false;
        }
        is_visible(host.def_view)
    }

    /// 置顶/取消置顶。
    pub fn set_topmost(h: HWND, on: bool) -> bool {
        let insert = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
        (unsafe { SetWindowPos(h, insert, 0, 0, 0, 0, ANCHOR_FLAGS) }) != 0
    }

    /// 送到 z 序底部。
    pub fn send_to_bottom(h: HWND) -> bool {
        (unsafe { SetWindowPos(h, HWND_BOTTOM, 0, 0, 0, 0, ANCHOR_FLAGS) }) != 0
    }

    /// 加上 `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`：点击不抢焦点、不出现在 Alt+Tab。
    pub fn set_no_activate(h: HWND) -> bool {
        let ex = unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32;
        let want = ex | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
        if want == ex {
            return true;
        }
        (unsafe { SetWindowLongW(h, GWL_EXSTYLE, want as i32) }) != 0
    }
}

// ---------------------------------------------------------------------------
// 跨平台门面
// ---------------------------------------------------------------------------

/// 定位桌面图标宿主。
pub fn resolve_icon_host() -> Option<IconHost> {
    #[cfg(target_os = "windows")]
    {
        win::resolve_icon_host()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// 显示/隐藏原生桌面图标。返回是否成功。
pub fn set_icons_visible(host: &IconHost, visible: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        win::set_icons_visible(host, visible)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (host, visible);
        false
    }
}

/// 原生桌面图标当前是否可见。
pub fn icons_visible(host: &IconHost) -> bool {
    #[cfg(target_os = "windows")]
    {
        win::icons_visible(host)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = host;
        false
    }
}

/// 把栅栏窗口插到桌面之上、普通窗口之下。
///
/// `raw_hwnd` 是 Tauri 窗口的原生句柄（`isize`）。
pub fn place_above_desktop(raw_hwnd: isize, host: &IconHost) -> bool {
    #[cfg(target_os = "windows")]
    {
        if raw_hwnd == 0 || host.host.is_null() {
            return false;
        }
        win::insert_above(raw_hwnd as *mut c_void, host.host)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (raw_hwnd, host);
        false
    }
}

/// 设置窗口置顶状态。
pub fn set_topmost(raw_hwnd: isize, on: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        if raw_hwnd == 0 {
            return false;
        }
        win::set_topmost(raw_hwnd as *mut c_void, on)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (raw_hwnd, on);
        false
    }
}

/// 把窗口送到 z 序底部。
pub fn send_to_bottom(raw_hwnd: isize) -> bool {
    #[cfg(target_os = "windows")]
    {
        if raw_hwnd == 0 {
            return false;
        }
        win::send_to_bottom(raw_hwnd as *mut c_void)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = raw_hwnd;
        false
    }
}

/// 让窗口不抢焦点、不出现在 Alt+Tab（`WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`）。
pub fn set_no_activate(raw_hwnd: isize) -> bool {
    #[cfg(target_os = "windows")]
    {
        if raw_hwnd == 0 {
            return false;
        }
        win::set_no_activate(raw_hwnd as *mut c_void)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = raw_hwnd;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_host_validity() {
        let host = IconHost {
            host: std::ptr::null_mut(),
            def_view: std::ptr::null_mut(),
        };
        assert!(!host.is_valid());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn resolves_a_plausible_icon_host() {
        // 桌面宿主在正常桌面上一定存在；无桌面会话下允许失败。
        if let Some(host) = resolve_icon_host() {
            assert!(host.is_valid());
        }
    }
}
