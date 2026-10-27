//! 桌面栅栏（Fences）数据模型。
//!
//! 设计移植自 PecoFence（Apache-2.0，https://github.com/DayuanJiang/PecoFence）的
//! `crates/core/src/model.rs`，按本项目的 Tauri + React 架构裁剪：
//! 去掉 Direct2D 渲染相关字段，保留栅栏分组、标签页、多显示器布局、规则引擎所需结构。
//!
//! 序列化统一 camelCase，前端直接消费。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub type FenceId = Uuid;
pub type ItemId = Uuid;
pub type RuleId = Uuid;

/// 配置文件结构版本。结构不兼容变更时递增。
pub const SCHEMA_VERSION: u32 = 1;

/// 一个显示器最多容纳的栅栏数（防御性上限）。
pub const MAX_FENCES_PER_LAYOUT: usize = 200;
/// 保存的布局快照上限。
pub const MAX_SNAPSHOTS: usize = 20;

// ---------------------------------------------------------------------------
// 顶层配置
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub settings: Settings,
    /// 全局条目表，按 id 索引。
    #[serde(default)]
    pub items: HashMap<ItemId, Item>,
    /// 每种显示器组合一份布局。
    #[serde(default)]
    pub layouts: Vec<Layout>,
    #[serde(default)]
    pub rules: crate::fence::rules::RuleSet,
    /// 用户可回滚的布局快照。
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            settings: Settings::default(),
            items: HashMap::new(),
            layouts: Vec::new(),
            rules: crate::fence::rules::RuleSet::default(),
            snapshots: Vec::new(),
        }
    }
}

/// 某一时刻对所有布局（栅栏几何、成员、视图标志）的命名拷贝。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: Uuid,
    pub name: String,
    /// Unix 秒。
    pub ts: i64,
    pub layouts: Vec<Layout>,
}

// ---------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// 是否隐藏 Windows 原生桌面图标（自绘栅栏的前提）。
    #[serde(default = "default_true")]
    pub hide_real_icons: bool,
    /// 默认图标尺寸（DIP）。
    #[serde(default = "default_icon_size")]
    pub icon_size: u32,
    /// 栅栏贴边吸附。
    #[serde(default)]
    pub snapping: SnappingSettings,
    /// 栅栏窗口的 z 序策略。
    #[serde(default)]
    pub zorder: ZOrderSetting,
    /// 双击标题栏卷起/展开。
    #[serde(default = "default_true")]
    pub roll_up_on_double_click: bool,
    /// 空桌面矩形拖拽可新建栅栏。
    #[serde(default = "default_true")]
    pub offer_new_fence: bool,
    /// 桌面文件夹路径（上次见到的），用于桌面搬家后重挂条目。
    #[serde(default)]
    pub desktop_path: Option<String>,
    /// 上次同步桌面条目的 Unix 秒。
    #[serde(default)]
    pub last_sync: Option<i64>,
}

fn default_true() -> bool {
    true
}
fn default_icon_size() -> u32 {
    48
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // 默认不隐藏原生图标：首次启动就清空桌面太激进，由用户在设置里主动开启。
            hide_real_icons: false,
            icon_size: 48,
            snapping: SnappingSettings::default(),
            zorder: ZOrderSetting::default(),
            roll_up_on_double_click: true,
            offer_new_fence: true,
            desktop_path: None,
            last_sync: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnappingSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 吸附阈值（DIP）。
    #[serde(default = "default_snap_threshold")]
    pub threshold: f32,
    /// 栅栏之间互相吸附。
    #[serde(default = "default_true")]
    pub to_fences: bool,
    /// 吸附到工作区边缘。
    #[serde(default = "default_true")]
    pub to_edges: bool,
}

fn default_snap_threshold() -> f32 {
    12.0
}

impl Default for SnappingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 12.0,
            to_fences: true,
            to_edges: true,
        }
    }
}

/// 栅栏窗口相对桌面图标层的 z 序策略。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZOrderSetting {
    /// 贴在桌面图标层之上、普通窗口之下（默认，最像原生）。
    #[default]
    DesktopLayer,
    /// 始终置顶。
    AlwaysTop,
    /// 普通窗口层级。
    Normal,
}

// ---------------------------------------------------------------------------
// 显示器与布局
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorIdentity {
    /// 显示器设备实例路径（如 `GSM7787#5&2C948443&0&UID24832`）。
    pub device_path: String,
    /// 保存时的 GDI 名（`\\.\DISPLAY1`）。
    #[serde(default)]
    pub gdi_name: String,
    /// 保存时工作区尺寸（DIP）。
    pub work_dip: [f32; 2],
    pub dpi: u32,
    /// 保存时物理分辨率（含任务栏）。
    #[serde(default)]
    pub screen_px: [i32; 2],
}

impl MonitorIdentity {
    fn knows_mode(&self) -> bool {
        self.screen_px != [0, 0]
    }

    /// 当前显示器是否与保存时同一分辨率与缩放。
    pub fn same_mode(&self, now: &crate::fence::monitor::WorkArea) -> bool {
        !self.knows_mode() || (self.screen_px == now.screen_px() && self.dpi == now.dpi)
    }

    /// 用于匹配的 GDI 名（缺省回落到设备路径）。
    pub fn gdi_name(&self) -> &str {
        if !self.gdi_name.is_empty() {
            &self.gdi_name
        } else if self.device_path.starts_with('\\') {
            &self.device_path
        } else {
            ""
        }
    }
}

/// 一组显示器上栅栏的摆放。所有布局共享同一批栅栏（标题、条目、标签页、视图），
/// 只是几何与卷起状态按布局各自保存。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub fingerprint: Vec<MonitorIdentity>,
    pub fences: Vec<Fence>,
    /// 每次显示该布局时递增；最大者为最后显示的布局。
    #[serde(default)]
    pub shown: u64,
}

impl Layout {
    pub fn new(fingerprint: Vec<MonitorIdentity>) -> Self {
        Self {
            fingerprint,
            fences: Vec::new(),
            shown: 0,
        }
    }

    pub fn fence(&self, id: FenceId) -> Option<&Fence> {
        self.fences.iter().find(|f| f.id == id)
    }

    pub fn fence_mut(&mut self, id: FenceId) -> Option<&mut Fence> {
        self.fences.iter_mut().find(|f| f.id == id)
    }

    /// 该布局的「桌面」收件箱栅栏。
    pub fn inbox(&self) -> Option<&Fence> {
        self.fences.iter().find(|f| f.kind == FenceKind::Inbox)
    }
}

/// 布局匹配结果。
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutMatch {
    pub index: usize,
    /// 每个已保存显示器对应的当前显示器索引。
    pub pairs: Vec<usize>,
    /// 已保存 id → 当前 id（显示器换了标识但仍是同一台）。
    pub renamed: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// 栅栏
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FenceKind {
    /// 普通用户栅栏。
    Virtual,
    /// 系统「桌面」收件箱：接收所有未分配条目。每个布局恰好一个。
    Inbox,
    /// 文件夹门户：内容来自某个磁盘文件夹。
    FolderPortal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ItemSourceSpec {
    Desktop,
    Folder {
        path: String,
        #[serde(default)]
        recursive: bool,
        #[serde(default)]
        filter: Option<String>,
    },
}

impl Default for ItemSourceSpec {
    fn default() -> Self {
        Self::Desktop
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    LeftTop,
    LeftBottom,
    LeftVCenter,
    RightTop,
    RightBottom,
    RightVCenter,
    HCenterTop,
    HCenterBottom,
    Center,
}

impl Default for Anchor {
    fn default() -> Self {
        Self::LeftTop
    }
}

/// 栅栏几何：相对显示器工作区，单位 DIP。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormGeometry {
    /// 所属显示器的 `MonitorIdentity::device_path`。
    pub monitor: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 保存时该显示器工作区尺寸，用于换分辨率后按比例还原。
    pub work_w: f32,
    pub work_h: f32,
    pub anchor: Anchor,
}

impl NormGeometry {
    pub fn new(monitor: &str, x: f32, y: f32, w: f32, h: f32, work_w: f32, work_h: f32) -> Self {
        Self {
            monitor: monitor.to_string(),
            x,
            y,
            w,
            h,
            work_w,
            work_h,
            anchor: Anchor::LeftTop,
        }
    }

    /// 按当前工作区尺寸换算成实际 DIP 坐标（处理分辨率变化）。
    pub fn resolve(&self, work_w: f32, work_h: f32) -> (f32, f32, f32, f32) {
        let (mut w, mut h) = (self.w, self.h);
        let (mut x, mut y) = (self.x, self.y);
        // 工作区变大/变小时按比例缩放，避免栅栏跑到屏幕外。
        if self.work_w > 0.0 && self.work_h > 0.0 {
            let sx = work_w / self.work_w;
            let sy = work_h / self.work_h;
            if (sx - 1.0).abs() > 0.001 || (sy - 1.0).abs() > 0.001 {
                w = (w * sx).max(120.0);
                h = (h * sy).max(80.0);
                // 锚点决定缩放时哪条边固定。
                x = match self.anchor {
                    Anchor::RightTop | Anchor::RightBottom | Anchor::RightVCenter => {
                        work_w - (self.work_w - self.x) * sx
                    }
                    Anchor::HCenterTop | Anchor::HCenterBottom | Anchor::Center => {
                        (work_w - w) / 2.0
                    }
                    _ => self.x * sx,
                };
                y = match self.anchor {
                    Anchor::LeftBottom | Anchor::RightBottom | Anchor::HCenterBottom => {
                        work_h - (self.work_h - self.y) * sy
                    }
                    Anchor::LeftVCenter | Anchor::RightVCenter | Anchor::Center => {
                        (work_h - h) / 2.0
                    }
                    _ => self.y * sy,
                };
            }
        }
        // 钳制在屏幕内，至少留出标题栏可抓取。
        let x = x.clamp(-w + 80.0, work_w - 80.0);
        let y = y.clamp(0.0, (work_h - 40.0).max(0.0));
        (x, y, w, h)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortMode {
    #[default]
    Manual,
    Name,
    Type,
    Date,
    Size,
    OpenCount,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewLayout {
    /// 图标网格，名称在下。
    #[default]
    Icons,
    /// 紧凑行：小图标 + 名称。
    List,
    /// 带可排序列的表格：名称 / 修改日期 / 类型 / 大小。
    Details,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Spacing {
    Compact,
    #[default]
    Normal,
    Comfortable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TitleSize {
    Small,
    #[default]
    Normal,
    Large,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FenceView {
    #[serde(default = "default_icon_size")]
    pub icon_size: u32,
    #[serde(default)]
    pub sort: SortMode,
    #[serde(default = "default_label_lines")]
    pub label_lines: u8,
    #[serde(default)]
    pub auto_height: bool,
    /// 反向排序。
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub layout: ViewLayout,
    #[serde(default)]
    pub spacing: Spacing,
    /// 「按时间分组」：今天 / 昨天 / 本周 / 本月 / 更早 分组标题。
    #[serde(default)]
    pub group_by_date: bool,
    /// 详情视图列宽（修改日期、类型、大小），单位 DIP。
    #[serde(default)]
    pub column_widths: Option<[f32; 3]>,
    /// 详情视图显示哪些列。
    #[serde(default)]
    pub columns_visible: Option<[bool; 3]>,
}

fn default_label_lines() -> u8 {
    2
}

impl Default for FenceView {
    fn default() -> Self {
        Self {
            icon_size: 48,
            sort: SortMode::Manual,
            label_lines: 2,
            auto_height: false,
            reverse: false,
            layout: ViewLayout::Icons,
            spacing: Spacing::Normal,
            group_by_date: false,
            column_widths: None,
            columns_visible: None,
        }
    }
}

/// 单个栅栏的外观覆盖（None = 跟随全局主题）。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceOverride {
    /// 玻璃底色着色。
    #[serde(default)]
    pub tint_rgb: Option<[u8; 3]>,
    /// 玻璃不透明度倍数（None = 1）。
    #[serde(default)]
    pub opacity: Option<f32>,
    /// 标题文字颜色。
    #[serde(default)]
    pub title_rgb: Option<[u8; 3]>,
    #[serde(default)]
    pub title_size: Option<TitleSize>,
    /// 标题行在鼠标悬停前折叠。
    #[serde(default)]
    pub title_auto_hide: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fence {
    pub id: FenceId,
    pub title: String,
    pub kind: FenceKind,
    #[serde(default)]
    pub source: ItemSourceSpec,
    pub geometry: NormGeometry,
    #[serde(default)]
    pub rolled_up: bool,
    /// 展开时的高度（DIP），卷起后用于还原。
    pub expanded_h: f32,
    #[serde(default)]
    pub view: FenceView,
    #[serde(default)]
    pub appearance: Option<AppearanceOverride>,
    /// 位置与尺寸不可用鼠标改动（「锁定」）。
    #[serde(default)]
    pub locked: bool,
    /// 作为另一个栅栏的标签页存在。被托管的栅栏没有自己的窗口，
    /// 但仍保留几何，供拆出时还原。
    #[serde(default)]
    pub tab_host: Option<FenceId>,
    /// 宿主栅栏上：当前显示哪个标签页的条目（None = 宿主自己的）。
    #[serde(default)]
    pub active_tab: Option<FenceId>,
    /// 宿主栅栏上：标签顺序（可把宿主自己排在任意位置）。
    #[serde(default)]
    pub tab_order: Vec<FenceId>,
    /// 文件夹门户：双击子文件夹在门户内打开。
    #[serde(default = "default_true")]
    pub portal_navigate: bool,
    /// 文件夹门户：标题前不显示文件夹图标。
    #[serde(default)]
    pub hide_title_icon: bool,
    /// 收件箱：为空时自动隐藏窗口。
    #[serde(default)]
    pub hide_when_empty: bool,
    #[serde(default)]
    pub items: Vec<ItemRef>,
}

impl Fence {
    pub fn new(title: &str, kind: FenceKind, geometry: NormGeometry) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.to_string(),
            kind,
            source: ItemSourceSpec::Desktop,
            expanded_h: geometry.h,
            geometry,
            rolled_up: false,
            view: FenceView::default(),
            appearance: None,
            locked: false,
            tab_host: None,
            active_tab: None,
            tab_order: Vec::new(),
            portal_navigate: true,
            hide_title_icon: false,
            hide_when_empty: false,
            items: Vec::new(),
        }
    }

    pub fn contains_item(&self, id: ItemId) -> bool {
        self.items.iter().any(|r| r.item_id == id)
    }

    /// 该栅栏是否是标签页（被别的栅栏托管）。
    pub fn is_tab(&self) -> bool {
        self.tab_host.is_some()
    }
}

// ---------------------------------------------------------------------------
// 条目
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssignedBy {
    /// 用户手动拖放/指定，规则不得覆盖。
    User,
    /// 由规则引擎归类。
    Rule(RuleId),
    /// 没有规则匹配，落到默认目标（通常是收件箱）。
    /// 与 `User` 区分开，否则规则永远无法接管这些条目。
    Default,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRef {
    pub item_id: ItemId,
    /// 手动排序时的位置。
    #[serde(default)]
    pub manual_index: Option<u32>,
    pub assigned_by: AssignedBy,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKey {
    /// 小写归一化的绝对路径。
    Path(String),
    /// 命名空间条目（回收站、此电脑）的 Base64 PIDL。
    Pidl(String),
}

impl ItemKey {
    /// 把文件系统路径归一化为规范 key 形式。
    pub fn from_path(path: &str) -> Self {
        let mut s: String = path
            .replace('/', "\\")
            .chars()
            .map(|c| {
                let mut lower = c.to_lowercase();
                match (lower.next(), lower.next()) {
                    (Some(l), None) => l,
                    _ => c,
                }
            })
            .collect();
        while s.ends_with('\\') && s.len() > 3 {
            s.pop();
        }
        ItemKey::Path(s)
    }

    pub fn as_path(&self) -> Option<&str> {
        match self {
            ItemKey::Path(p) => Some(p),
            ItemKey::Pidl(_) => None,
        }
    }

    /// 是否为 shell 命名空间条目（`::{CLSID}` 形式）。
    pub fn is_namespace(&self) -> bool {
        matches!(self, ItemKey::Path(p) if p.starts_with("::{"))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    #[default]
    UserDesktop,
    PublicDesktop,
    Namespace,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IconKey {
    /// 按扩展名共享图标（文档、多数文件）。
    ByExt(String),
    /// 文件专属图标（.exe / .lnk / .ico / 自定义图标文件夹 / 图片）。
    ByContent { path: String, mtime: i64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: ItemId,
    pub key: ItemKey,
    #[serde(default)]
    pub origin: Origin,
    pub display_name: String,
    /// 修改时间（Unix 秒）。
    #[serde(default)]
    pub mtime: i64,
    #[serde(default)]
    pub is_folder: bool,
    #[serde(default)]
    pub attrs: u32,
    pub icon_key: IconKey,
    /// 条目在磁盘上消失的起始时间；重新出现则清空。
    #[serde(default)]
    pub orphaned_since: Option<i64>,
    /// 文件大小（字节，文件夹为 0）。
    #[serde(default)]
    pub size: u64,
    /// 从栅栏启动的次数。
    #[serde(default)]
    pub open_count: u32,
    /// 最后一次从栅栏启动的 Unix 秒。
    #[serde(default)]
    pub last_opened: Option<i64>,
    /// 磁盘上的真实拼写（`ABC.zip`）；`key` 是它的大小写折叠形式。
    #[serde(default)]
    pub path: Option<String>,
}

impl Item {
    pub fn is_namespace(&self) -> bool {
        self.key.is_namespace()
    }

    /// 交给 shell 与其他程序使用的路径。
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref().or_else(|| self.key.as_path())
    }

    /// 文件扩展名（小写，含点）。
    pub fn ext(&self) -> Option<String> {
        let name = self
            .path()
            .map(|p| p.rsplit('\\').next().unwrap_or(p))
            .unwrap_or(&self.display_name);
        name.rsplit_once('.')
            .map(|(_, e)| format!(".{}", e.to_lowercase()))
    }
}

// ---------------------------------------------------------------------------
// 便捷构造
// ---------------------------------------------------------------------------

/// 新建一个默认的「桌面」收件箱栅栏。
pub fn new_inbox(monitor: &str, work_w: f32, work_h: f32) -> Fence {
    let w = 420.0_f32.min(work_w * 0.4);
    let h = 320.0_f32.min(work_h * 0.5);
    let mut fence = Fence::new(
        "桌面",
        FenceKind::Inbox,
        NormGeometry::new(monitor, 24.0, 24.0, w, h, work_w, work_h),
    );
    fence.view.auto_height = true;
    fence
}

/// 新建一个普通栅栏。
pub fn new_fence(title: &str, monitor: &str, x: f32, y: f32, work_w: f32, work_h: f32) -> Fence {
    let w = 360.0_f32.min(work_w * 0.35);
    let h = 260.0_f32.min(work_h * 0.4);
    Fence::new(
        title,
        FenceKind::Virtual,
        NormGeometry::new(monitor, x, y, w, h, work_w, work_h),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_key_normalizes_paths() {
        let k = ItemKey::from_path("C:/Users/Alice/Desktop/Report.DOCX");
        assert_eq!(k.as_path(), Some("c:\\users\\alice\\desktop\\report.docx"));
    }

    #[test]
    fn item_key_trims_trailing_separators() {
        let k = ItemKey::from_path("C:\\Temp\\");
        assert_eq!(k.as_path(), Some("c:\\temp"));
        // 盘符根不裁剪
        let root = ItemKey::from_path("C:\\");
        assert_eq!(root.as_path(), Some("c:\\"));
    }

    #[test]
    fn namespace_keys_are_detected() {
        let k = ItemKey::from_path("::{645FF040-5081-101B-9F08-00AA002F954E}");
        assert!(k.is_namespace());
    }

    #[test]
    fn geometry_resolves_identity_when_work_area_unchanged() {
        let g = NormGeometry::new("DISPLAY1", 100.0, 50.0, 300.0, 200.0, 1920.0, 1080.0);
        let (x, y, w, h) = g.resolve(1920.0, 1080.0);
        assert_eq!((x, y, w, h), (100.0, 50.0, 300.0, 200.0));
    }

    #[test]
    fn geometry_scales_with_work_area() {
        let g = NormGeometry::new("DISPLAY1", 100.0, 50.0, 300.0, 200.0, 1920.0, 1080.0);
        let (x, y, w, h) = g.resolve(3840.0, 2160.0);
        assert!((x - 200.0).abs() < 0.01);
        assert!((y - 100.0).abs() < 0.01);
        assert!((w - 600.0).abs() < 0.01);
        assert!((h - 400.0).abs() < 0.01);
    }

    #[test]
    fn geometry_clamps_into_work_area() {
        let g = NormGeometry::new("DISPLAY1", 5000.0, 5000.0, 300.0, 200.0, 1920.0, 1080.0);
        let (x, y, _w, _h) = g.resolve(1920.0, 1080.0);
        assert!(x <= 1920.0 - 80.0);
        assert!(y <= 1080.0 - 40.0);
    }

    #[test]
    fn right_anchored_geometry_keeps_right_edge_on_resize() {
        let mut g = NormGeometry::new("DISPLAY1", 1600.0, 100.0, 300.0, 200.0, 1920.0, 1080.0);
        g.anchor = Anchor::RightTop;
        // 右边缘距工作区右边 20px；放大后仍应保持 20px
        let (x, _y, w, _h) = g.resolve(3840.0, 2160.0);
        assert!((3840.0 - (x + w) - 40.0).abs() < 1.0, "x={x} w={w}");
    }

    #[test]
    fn fence_new_sets_expanded_height() {
        let g = NormGeometry::new("D", 0.0, 0.0, 300.0, 200.0, 1920.0, 1080.0);
        let f = Fence::new("测试", FenceKind::Virtual, g);
        assert_eq!(f.expanded_h, 200.0);
        assert!(!f.rolled_up);
        assert!(!f.is_tab());
    }

    #[test]
    fn item_ext_is_lowercase_with_dot() {
        let mut item = Item {
            id: Uuid::new_v4(),
            key: ItemKey::from_path("C:\\a\\B.ZIP"),
            origin: Origin::UserDesktop,
            display_name: "B.ZIP".into(),
            mtime: 0,
            is_folder: false,
            attrs: 0,
            icon_key: IconKey::ByExt(".zip".into()),
            orphaned_since: None,
            size: 0,
            open_count: 0,
            last_opened: None,
            path: Some("C:\\a\\B.ZIP".into()),
        };
        assert_eq!(item.ext().as_deref(), Some(".zip"));
        item.path = None;
        assert_eq!(item.ext().as_deref(), Some(".zip"));
    }

    #[test]
    fn config_roundtrips_through_json() {
        let mut cfg = Config::default();
        let mut layout = Layout::new(vec![]);
        layout.fences.push(new_inbox("D1", 1920.0, 1080.0));
        cfg.layouts.push(layout);
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.layouts.len(), 1);
        assert_eq!(back.layouts[0].fences.len(), 1);
        assert_eq!(back.layouts[0].fences[0].kind, FenceKind::Inbox);
    }
}
