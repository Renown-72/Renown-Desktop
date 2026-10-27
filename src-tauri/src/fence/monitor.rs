//! 显示器枚举与布局指纹匹配。
//!
//! 栅栏几何按「显示器工作区 + DIP」保存，换分辨率/缩放/换屏后要能还原，
//! 因此需要稳定的显示器标识：优先设备实例路径，其次 EDID 型号+连接器，最后 GDI 名。

use crate::fence::model::{Layout, LayoutMatch, MonitorIdentity};

/// 一台显示器的当前状态。
#[derive(Clone, Debug, PartialEq)]
pub struct WorkArea {
    /// 设备实例路径（`\\?\DISPLAY#GSM7787#5&2c948443&0&UID24832#{e6f07b5f-...}` 的规范化形式），
    /// 取不到时回落到 EDID 型号+连接器，再回落到 GDI 名。
    pub device_path: String,
    /// GDI 名（`\\.\DISPLAY1`）。
    pub gdi_name: String,
    /// 工作区（不含任务栏）物理像素矩形：left, top, right, bottom。
    pub rect_px: [i32; 4],
    /// 整块屏幕（含任务栏）物理像素尺寸。
    pub screen_px: [i32; 2],
    /// 有效 DPI（96 = 100%）。
    pub dpi: u32,
}

impl WorkArea {
    /// 工作区宽度（DIP）。
    pub fn width_dip(&self) -> f32 {
        let w = (self.rect_px[2] - self.rect_px[0]).max(1) as f32;
        w * 96.0 / self.dpi.max(1) as f32
    }

    /// 工作区高度（DIP）。
    pub fn height_dip(&self) -> f32 {
        let h = (self.rect_px[3] - self.rect_px[1]).max(1) as f32;
        h * 96.0 / self.dpi.max(1) as f32
    }

    /// 工作区左上角（DIP，虚拟桌面坐标）。
    pub fn origin_dip(&self) -> (f32, f32) {
        let scale = 96.0 / self.dpi.max(1) as f32;
        (
            self.rect_px[0] as f32 * scale,
            self.rect_px[1] as f32 * scale,
        )
    }

    pub fn screen_px(&self) -> [i32; 2] {
        self.screen_px
    }

    pub fn identity(&self) -> MonitorIdentity {
        MonitorIdentity {
            device_path: self.device_path.clone(),
            gdi_name: self.gdi_name.clone(),
            work_dip: [self.width_dip(), self.height_dip()],
            dpi: self.dpi,
            screen_px: self.screen_px,
        }
    }
}

// ---------------------------------------------------------------------------
// Windows 实现
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod win {
    use super::WorkArea;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW, HDC, HMONITOR,
        MONITORINFOEXW,
    };

    /// `EDD_GET_DEVICE_INTERFACE_NAME`
    const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 0x0000_0001;

    fn wide_to_string(buf: &[u16]) -> String {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        OsString::from_wide(&buf[..len])
            .to_string_lossy()
            .into_owned()
    }

    /// 从 `\\?\DISPLAY#GSM7787#5&2c948443&0&UID24832#{guid}` 提取稳定短标识。
    fn normalize_device_path(raw: &str) -> String {
        let s = raw.trim_start_matches(r"\\?\");
        let s = s.split('#').take(3).collect::<Vec<_>>().join("#");
        if s.is_empty() {
            raw.to_string()
        } else {
            s
        }
    }

    /// 查询某显示器的设备实例路径。
    fn device_path_for(gdi_name: &str) -> Option<String> {
        let mut name: Vec<u16> = gdi_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut dev: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        dev.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = unsafe {
            EnumDisplayDevicesW(
                name.as_mut_ptr(),
                0,
                &mut dev,
                EDD_GET_DEVICE_INTERFACE_NAME,
            )
        };
        if ok == 0 {
            return None;
        }
        let path = wide_to_string(&dev.DeviceID);
        if path.is_empty() {
            None
        } else {
            Some(normalize_device_path(&path))
        }
    }

    struct Collect {
        out: Vec<WorkArea>,
    }

    unsafe extern "system" fn monitor_cb(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let out = unsafe { &mut *(data as *mut Collect) };
        let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) } == 0 {
            return TRUE;
        }
        let gdi_name = wide_to_string(&info.szDevice);
        let r = info.monitorInfo.rcMonitor;
        let w = info.monitorInfo.rcWork;
        let dpi = unsafe { super::dpi_for_monitor(monitor) };
        let device_path = device_path_for(&gdi_name).unwrap_or_else(|| gdi_name.clone());
        out.out.push(WorkArea {
            device_path,
            gdi_name,
            rect_px: [w.left, w.top, w.right, w.bottom],
            screen_px: [r.right - r.left, r.bottom - r.top],
            dpi,
        });
        TRUE
    }

    pub fn enumerate() -> Vec<WorkArea> {
        let mut collect = Collect { out: Vec::new() };
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(monitor_cb),
                &mut collect as *mut Collect as LPARAM,
            );
        }
        // 按虚拟桌面坐标排序，保证顺序稳定（主屏通常在最前）。
        collect.out.sort_by_key(|m| (m.rect_px[0], m.rect_px[1]));
        collect.out
    }
}

/// 查询显示器 DPI（失败时回落到 96）。
#[cfg(target_os = "windows")]
unsafe fn dpi_for_monitor(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR) -> u32 {
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    let mut x = 0u32;
    let mut y = 0u32;
    let hr = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) };
    if hr == 0 && x > 0 {
        x
    } else {
        96
    }
}

#[cfg(not(target_os = "windows"))]
mod win {
    use super::WorkArea;
    pub fn enumerate() -> Vec<WorkArea> {
        vec![WorkArea {
            device_path: "DISPLAY1".into(),
            gdi_name: r"\\.\DISPLAY1".into(),
            rect_px: [0, 0, 1920, 1040],
            screen_px: [1920, 1080],
            dpi: 96,
        }]
    }
}

/// 枚举当前所有显示器。
pub fn enumerate() -> Vec<WorkArea> {
    win::enumerate()
}

/// 主显示器（虚拟桌面原点所在的那块），取不到时返回第一块。
pub fn primary(areas: &[WorkArea]) -> Option<&WorkArea> {
    areas
        .iter()
        .find(|a| a.rect_px[0] == 0 && a.rect_px[1] == 0)
        .or_else(|| areas.first())
}

/// 按 `device_path` 找显示器。
pub fn find<'a>(areas: &'a [WorkArea], device_path: &str) -> Option<&'a WorkArea> {
    areas.iter().find(|a| a.device_path == device_path)
}

// ---------------------------------------------------------------------------
// 布局指纹匹配
// ---------------------------------------------------------------------------

/// 已保存布局与当前显示器集合的匹配度。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchScore {
    /// 精确匹配（设备路径相同）的数量。
    pub exact: usize,
    /// 通过 GDI 名或型号回退匹配的数量。
    pub fallback: usize,
    /// 已保存但当前不存在的显示器数量。
    pub missing: usize,
    /// 当前存在但布局里没有的显示器数量。
    pub extra: usize,
}

impl MatchScore {
    /// 是否可用：所有已保存显示器都找到了对应，且没有多余显示器。
    pub fn is_complete(&self) -> bool {
        self.missing == 0 && self.extra == 0
    }

    /// 排序用：精确匹配优先，其次回退匹配，最后惩罚缺失/多余。
    fn rank(&self) -> (usize, usize, usize, usize) {
        (
            self.missing + self.extra,
            usize::MAX - self.exact,
            usize::MAX - self.fallback,
            0,
        )
    }
}

/// 计算一份布局指纹与当前显示器的匹配度。
pub fn score(layout: &Layout, areas: &[WorkArea]) -> MatchScore {
    let mut used = vec![false; areas.len()];
    let mut exact = 0;
    let mut fallback = 0;

    // 第一轮：设备路径精确匹配
    for saved in &layout.fingerprint {
        if let Some(i) = areas
            .iter()
            .enumerate()
            .position(|(i, a)| !used[i] && a.device_path == saved.device_path)
        {
            used[i] = true;
            exact += 1;
        }
    }
    // 第二轮：GDI 名回退（显示器换了接口但仍是同一块屏）
    for saved in &layout.fingerprint {
        let already = areas
            .iter()
            .enumerate()
            .any(|(i, a)| used[i] && a.device_path == saved.device_path);
        if already {
            continue;
        }
        let gdi = saved.gdi_name();
        if gdi.is_empty() {
            continue;
        }
        if let Some(i) = areas
            .iter()
            .enumerate()
            .position(|(i, a)| !used[i] && a.gdi_name == gdi)
        {
            used[i] = true;
            fallback += 1;
        }
    }

    let missing = layout.fingerprint.len() - exact - fallback;
    let extra = areas.len() - exact - fallback;
    MatchScore {
        exact,
        fallback,
        missing,
        extra,
    }
}

/// 在已有布局中找出与当前显示器最匹配的一份。
///
/// 返回 `None` 表示没有任何布局可用（需要新建）。
pub fn best_layout(layouts: &[Layout], areas: &[WorkArea]) -> Option<(usize, MatchScore)> {
    let mut best: Option<(usize, MatchScore)> = None;
    for (i, layout) in layouts.iter().enumerate() {
        let s = score(layout, areas);
        if !s.is_complete() {
            continue;
        }
        match &best {
            Some((_, cur)) if cur.rank() <= s.rank() => {}
            _ => best = Some((i, s)),
        }
    }
    best
}

/// 构造 [`LayoutMatch`]：每个已保存显示器对应的当前显示器索引。
pub fn layout_match(layout: &Layout, areas: &[WorkArea], index: usize) -> LayoutMatch {
    let mut pairs = Vec::new();
    let mut renamed = Vec::new();
    let mut used = vec![false; areas.len()];

    for saved in &layout.fingerprint {
        if let Some(i) = areas
            .iter()
            .enumerate()
            .position(|(i, a)| !used[i] && a.device_path == saved.device_path)
        {
            used[i] = true;
            pairs.push(i);
            continue;
        }
        let gdi = saved.gdi_name();
        if let Some(i) = areas
            .iter()
            .enumerate()
            .position(|(i, a)| !used[i] && !gdi.is_empty() && a.gdi_name == gdi)
        {
            used[i] = true;
            pairs.push(i);
            renamed.push((saved.device_path.clone(), areas[i].device_path.clone()));
        }
    }

    LayoutMatch {
        index,
        pairs,
        renamed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(device: &str, gdi: &str) -> WorkArea {
        WorkArea {
            device_path: device.into(),
            gdi_name: gdi.into(),
            rect_px: [0, 0, 1920, 1040],
            screen_px: [1920, 1080],
            dpi: 96,
        }
    }

    fn layout_with(devices: &[&str]) -> Layout {
        Layout::new(
            devices
                .iter()
                .map(|d| MonitorIdentity {
                    device_path: (*d).into(),
                    gdi_name: String::new(),
                    work_dip: [1920.0, 1040.0],
                    dpi: 96,
                    screen_px: [1920, 1080],
                })
                .collect(),
        )
    }

    #[test]
    fn work_area_dip_conversion() {
        let mut a = area("D1", r"\\.\DISPLAY1");
        a.rect_px = [0, 0, 3840, 2080];
        a.dpi = 192;
        assert!((a.width_dip() - 1920.0).abs() < 0.01);
        assert!((a.height_dip() - 1040.0).abs() < 0.01);
    }

    #[test]
    fn identity_captures_mode() {
        let a = area("D1", r"\\.\DISPLAY1");
        let id = a.identity();
        assert_eq!(id.screen_px, [1920, 1080]);
        assert_eq!(id.dpi, 96);
        assert!(id.same_mode(&a));
    }

    #[test]
    fn same_mode_detects_resolution_change() {
        let a = area("D1", r"\\.\DISPLAY1");
        let id = a.identity();
        let mut changed = a.clone();
        changed.screen_px = [2560, 1440];
        assert!(!id.same_mode(&changed));
    }

    #[test]
    fn exact_match_scores_complete() {
        let layout = layout_with(&["A", "B"]);
        let areas = vec![area("A", r"\\.\DISPLAY1"), area("B", r"\\.\DISPLAY2")];
        let s = score(&layout, &areas);
        assert_eq!(s.exact, 2);
        assert!(s.is_complete());
    }

    #[test]
    fn missing_monitor_breaks_completeness() {
        let layout = layout_with(&["A", "B"]);
        let areas = vec![area("A", r"\\.\DISPLAY1")];
        let s = score(&layout, &areas);
        assert_eq!(s.missing, 1);
        assert!(!s.is_complete());
    }

    #[test]
    fn extra_monitor_breaks_completeness() {
        let layout = layout_with(&["A"]);
        let areas = vec![area("A", r"\\.\DISPLAY1"), area("B", r"\\.\DISPLAY2")];
        let s = score(&layout, &areas);
        assert_eq!(s.extra, 1);
        assert!(!s.is_complete());
    }

    #[test]
    fn gdi_name_fallback_matches_renamed_monitor() {
        let mut layout = layout_with(&["OLD"]);
        layout.fingerprint[0].gdi_name = r"\\.\DISPLAY1".into();
        let areas = vec![area("NEW", r"\\.\DISPLAY1")];
        let s = score(&layout, &areas);
        assert_eq!(s.fallback, 1);
        assert!(s.is_complete());
    }

    #[test]
    fn best_layout_picks_complete_match() {
        let layouts = vec![
            layout_with(&["A", "B"]),
            layout_with(&["A"]),
            layout_with(&["C"]),
        ];
        let areas = vec![area("A", r"\\.\DISPLAY1")];
        let (idx, _) = best_layout(&layouts, &areas).unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn best_layout_returns_none_when_nothing_matches() {
        let layouts = vec![layout_with(&["A", "B"])];
        let areas = vec![area("Z", r"\\.\DISPLAY9")];
        assert!(best_layout(&layouts, &areas).is_none());
    }

    #[test]
    fn layout_match_reports_renames() {
        let mut layout = layout_with(&["OLD"]);
        layout.fingerprint[0].gdi_name = r"\\.\DISPLAY1".into();
        let areas = vec![area("NEW", r"\\.\DISPLAY1")];
        let m = layout_match(&layout, &areas, 0);
        assert_eq!(m.pairs, vec![0]);
        assert_eq!(m.renamed, vec![("OLD".to_string(), "NEW".to_string())]);
    }

    #[test]
    fn primary_prefers_origin_monitor() {
        let mut a = area("A", r"\\.\DISPLAY1");
        a.rect_px = [-1920, 0, 0, 1040];
        let b = area("B", r"\\.\DISPLAY2");
        let areas = vec![a, b];
        assert_eq!(primary(&areas).unwrap().device_path, "B");
    }
}
