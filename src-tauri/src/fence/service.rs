//! 栅栏业务逻辑。
//!
//! 把纯数据（[`Config`]）与副作用（文件系统、Win32 窗口）隔开：
//! 本模块只操作内存中的配置，持久化交给 [`crate::fence::store`]，
//! 窗口操作交给 [`crate::fence::window`]。

use crate::fence::desktop;
use crate::fence::model::{
    new_fence, new_inbox, Anchor, Config, Fence, FenceId, FenceKind, FenceView, Item, ItemId,
    ItemRef, Layout, NormGeometry, Snapshot, MAX_FENCES_PER_LAYOUT, MAX_SNAPSHOTS,
};
use crate::fence::monitor;
use crate::fence::rules::{self, Decision, ItemFacts, RuleSet, Target, Template};
use crate::fence::shortcut;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// 同步桌面后的变化摘要，供前端提示。
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// 新出现的条目数。
    pub added: usize,
    /// 从磁盘消失的条目数。
    pub removed: usize,
    /// 被规则重新归类的条目数。
    pub rerouted: usize,
    /// 当前桌面条目总数。
    pub total: usize,
    /// 桌面文件夹路径。
    pub desktop_path: Option<String>,
}

/// 栅栏服务：持有配置并串行化所有修改。
pub struct FenceService {
    inner: Mutex<Inner>,
}

struct Inner {
    config: Config,
    /// 当前生效的布局索引。
    active_layout: usize,
    /// 上次同步时的桌面路径，用于检测桌面搬家。
    last_desktop_path: Option<String>,
}

impl FenceService {
    pub fn new() -> Self {
        let (config, recovered) = crate::fence::store::load();
        if recovered {
            eprintln!("[fence] 配置损坏，已回落到默认配置");
        }
        let service = Self {
            inner: Mutex::new(Inner {
                config,
                active_layout: 0,
                last_desktop_path: None,
            }),
        };
        service.ensure_layout();
        service
    }

    /// 读取配置快照（前端用）。
    pub fn snapshot(&self) -> Config {
        self.inner.lock().unwrap().config.clone()
    }

    /// 当前布局。
    pub fn active_layout(&self) -> Layout {
        let inner = self.inner.lock().unwrap();
        inner.config.layouts[inner.active_layout].clone()
    }

    /// 确保存在与当前显示器匹配的布局；没有则新建。
    pub fn ensure_layout(&self) -> usize {
        let areas = monitor::enumerate();
        let mut inner = self.inner.lock().unwrap();
        if let Some((idx, _)) = monitor::best_layout(&inner.config.layouts, &areas) {
            inner.active_layout = idx;
            inner.config.layouts[idx].shown += 1;
            return idx;
        }
        // 新建布局：沿用最近一次显示的布局里的栅栏结构（保留成员关系），
        // 但把几何重新锚定到主显示器。
        let fingerprint: Vec<_> = areas.iter().map(|a| a.identity()).collect();
        let mut layout = Layout::new(fingerprint);
        let primary = monitor::primary(&areas).cloned();
        let (monitor_id, work_w, work_h) = primary
            .as_ref()
            .map(|a| (a.device_path.clone(), a.width_dip(), a.height_dip()))
            .unwrap_or_else(|| ("DISPLAY1".to_string(), 1920.0, 1080.0));

        let previous = inner
            .config
            .layouts
            .iter()
            .max_by_key(|l| l.shown)
            .map(|l| l.fences.clone())
            .unwrap_or_default();

        if previous.is_empty() {
            layout.fences.push(new_inbox(&monitor_id, work_w, work_h));
        } else {
            let mut y = 24.0f32;
            for old in previous {
                let mut fence = old;
                fence.geometry = NormGeometry::new(
                    &monitor_id,
                    fence.geometry.x,
                    y,
                    fence.geometry.w,
                    fence.geometry.h,
                    work_w,
                    work_h,
                );
                fence.geometry.anchor = Anchor::LeftTop;
                y += 40.0;
                layout.fences.push(fence);
            }
        }
        layout.shown = 1;
        inner.config.layouts.push(layout);
        inner.active_layout = inner.config.layouts.len() - 1;
        inner.active_layout
    }

    /// 同步桌面：枚举真实文件、合并条目、按规则归类。
    pub fn sync_desktop(&self) -> SyncReport {
        let entries = desktop::enumerate_desktop();
        let mut inner = self.inner.lock().unwrap();
        let desktop_path = desktop::user_desktop().map(|p| p.to_string_lossy().to_string());

        // 桌面搬家检测：路径变了就把旧条目标记 orphaned，让新路径重新挂上。
        if let Some(prev) = &inner.last_desktop_path {
            if desktop_path.as_deref() != Some(prev.as_str()) {
                for item in inner.config.items.values_mut() {
                    if item.orphaned_since.is_none() {
                        item.orphaned_since = Some(desktop::now_secs());
                    }
                }
            }
        }
        inner.last_desktop_path = desktop_path.clone();
        inner.config.settings.desktop_path = desktop_path.clone();
        inner.config.settings.last_sync = Some(desktop::now_secs());

        let (merged, created, orphaned) = desktop::merge_into(&inner.config.items, &entries);
        inner.config.items = merged;

        let rerouted = apply_rules_locked(&mut inner);

        SyncReport {
            added: created.len(),
            removed: orphaned.len(),
            rerouted,
            total: inner.config.items.len(),
            desktop_path,
        }
    }

    /// 只重跑规则（不重新枚举桌面）。
    pub fn apply_rules(&self) -> usize {
        let mut inner = self.inner.lock().unwrap();
        apply_rules_locked(&mut inner)
    }

    /// 新建栅栏。
    pub fn create_fence(&self, title: &str, x: f32, y: f32, w: f32, h: f32) -> Fence {
        let areas = monitor::enumerate();
        let primary = monitor::primary(&areas).cloned();
        let (monitor_id, work_w, work_h) = primary
            .as_ref()
            .map(|a| (a.device_path.clone(), a.width_dip(), a.height_dip()))
            .unwrap_or_else(|| ("DISPLAY1".to_string(), 1920.0, 1080.0));

        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let mut fence = new_fence(title, &monitor_id, x, y, work_w, work_h);
        if w > 0.0 {
            fence.geometry.w = w;
        }
        if h > 0.0 {
            fence.geometry.h = h;
            fence.expanded_h = h;
        }
        if inner.config.layouts[idx].fences.len() >= MAX_FENCES_PER_LAYOUT {
            return fence;
        }
        inner.config.layouts[idx].fences.push(fence.clone());
        fence
    }

    /// 删除栅栏。收件箱不可删。
    pub fn delete_fence(&self, id: FenceId) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let Some(pos) = inner.config.layouts[idx]
            .fences
            .iter()
            .position(|f| f.id == id)
        else {
            return false;
        };
        if inner.config.layouts[idx].fences[pos].kind == FenceKind::Inbox {
            return false;
        }
        // 该栅栏的条目回到收件箱。
        let inbox = inner.config.layouts[idx]
            .inbox()
            .map(|f| f.id)
            .unwrap_or(id);
        let moved: Vec<ItemRef> = inner.config.layouts[idx].fences[pos].items.clone();
        inner.config.layouts[idx].fences.remove(pos);
        if let Some(host_pos) = inner.config.layouts[idx]
            .fences
            .iter()
            .position(|f| f.id == inbox)
        {
            for item in moved {
                if !inner.config.layouts[idx].fences[host_pos].contains_item(item.item_id) {
                    inner.config.layouts[idx].fences[host_pos].items.push(item);
                }
            }
        }
        // 清理标签页引用。
        for fence in &mut inner.config.layouts[idx].fences {
            if fence.tab_host == Some(id) {
                fence.tab_host = None;
            }
            fence.tab_order.retain(|t| *t != id);
            if fence.active_tab == Some(id) {
                fence.active_tab = None;
            }
        }
        true
    }

    /// 重命名栅栏。
    pub fn rename_fence(&self, id: FenceId, title: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        if let Some(f) = inner.config.layouts[idx].fence_mut(id) {
            f.title = title.to_string();
            return true;
        }
        false
    }

    /// 设置栅栏几何。
    pub fn set_geometry(&self, id: FenceId, x: f32, y: f32, w: f32, h: f32) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        if let Some(f) = inner.config.layouts[idx].fence_mut(id) {
            if f.locked {
                return false;
            }
            f.geometry.x = x;
            f.geometry.y = y;
            f.geometry.w = w;
            f.geometry.h = h;
            if !f.rolled_up {
                f.expanded_h = h;
            }
            return true;
        }
        false
    }

    /// 卷起/展开栅栏。
    pub fn set_rolled_up(&self, id: FenceId, rolled: bool) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        if let Some(f) = inner.config.layouts[idx].fence_mut(id) {
            if rolled && !f.rolled_up {
                f.expanded_h = f.geometry.h;
            }
            f.rolled_up = rolled;
            if !rolled {
                f.geometry.h = f.expanded_h;
            }
            return true;
        }
        false
    }

    /// 更新视图设置。
    pub fn set_view(&self, id: FenceId, view: FenceView) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        if let Some(f) = inner.config.layouts[idx].fence_mut(id) {
            f.view = view;
            return true;
        }
        false
    }

    /// 把条目分配到栅栏（用户手动拖放）。
    pub fn assign_item(&self, item_id: ItemId, fence_id: FenceId) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        if !inner.config.items.contains_key(&item_id) {
            return false;
        }
        if inner.config.layouts[idx].fence(fence_id).is_none() {
            return false;
        }
        // 先从所有栅栏移除
        for fence in &mut inner.config.layouts[idx].fences {
            fence.items.retain(|r| r.item_id != item_id);
        }
        if let Some(f) = inner.config.layouts[idx].fence_mut(fence_id) {
            let next_index = f.items.len() as u32;
            f.items.push(ItemRef {
                item_id,
                manual_index: Some(next_index),
                assigned_by: crate::fence::model::AssignedBy::User,
            });
            return true;
        }
        false
    }

    /// 把条目移出栅栏（回到收件箱）。
    pub fn unassign_item(&self, item_id: ItemId) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let inbox = match inner.config.layouts[idx].inbox() {
            Some(f) => f.id,
            None => return false,
        };
        for fence in &mut inner.config.layouts[idx].fences {
            fence.items.retain(|r| r.item_id != item_id);
        }
        if let Some(f) = inner.config.layouts[idx].fence_mut(inbox) {
            f.items.push(ItemRef {
                item_id,
                manual_index: None,
                assigned_by: crate::fence::model::AssignedBy::User,
            });
            return true;
        }
        false
    }

    /// 调整栅栏内条目顺序。
    pub fn reorder_items(&self, fence_id: FenceId, ordered: Vec<ItemId>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let Some(fence) = inner.config.layouts[idx].fence_mut(fence_id) else {
            return false;
        };
        let mut by_id: HashMap<ItemId, ItemRef> =
            fence.items.drain(..).map(|r| (r.item_id, r)).collect();
        for (i, id) in ordered.iter().enumerate() {
            if let Some(mut r) = by_id.remove(id) {
                r.manual_index = Some(i as u32);
                fence.items.push(r);
            }
        }
        // 未出现在 ordered 里的保持在后
        let mut rest: Vec<ItemRef> = by_id.into_values().collect();
        rest.sort_by_key(|r| r.manual_index.unwrap_or(u32::MAX));
        fence.items.extend(rest);
        true
    }

    // -----------------------------------------------------------------
    // 标签页
    // -----------------------------------------------------------------

    /// 把 `fence_id` 变成 `host_id` 的标签页。
    pub fn make_tab(&self, fence_id: FenceId, host_id: FenceId) -> bool {
        if fence_id == host_id {
            return false;
        }
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let fences = &inner.config.layouts[idx].fences;
        let Some(host) = fences.iter().find(|f| f.id == host_id) else {
            return false;
        };
        // 宿主自己不能是标签页
        if host.tab_host.is_some() {
            return false;
        }
        let Some(target) = fences.iter().find(|f| f.id == fence_id) else {
            return false;
        };
        if target.kind == FenceKind::Inbox {
            return false;
        }
        // 不能形成环：目标不能是宿主的宿主
        if target.tab_host == Some(host_id) {
            return false;
        }
        let fences = &mut inner.config.layouts[idx].fences;
        if let Some(t) = fences.iter_mut().find(|f| f.id == fence_id) {
            t.tab_host = Some(host_id);
        }
        if let Some(h) = fences.iter_mut().find(|f| f.id == host_id) {
            if !h.tab_order.contains(&fence_id) {
                h.tab_order.push(fence_id);
            }
            if h.active_tab.is_none() {
                h.active_tab = Some(fence_id);
            }
        }
        true
    }

    /// 把标签页拆出成独立栅栏。
    pub fn split_tab(&self, fence_id: FenceId) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let host_id = match inner.config.layouts[idx].fence(fence_id) {
            Some(f) => f.tab_host,
            None => return false,
        };
        let Some(host_id) = host_id else {
            return false;
        };
        if let Some(t) = inner.config.layouts[idx].fence_mut(fence_id) {
            t.tab_host = None;
        }
        if let Some(h) = inner.config.layouts[idx].fence_mut(host_id) {
            h.tab_order.retain(|t| *t != fence_id);
            if h.active_tab == Some(fence_id) {
                h.active_tab = h.tab_order.first().copied();
            }
        }
        true
    }

    /// 切换宿主栅栏当前显示的标签页。
    pub fn set_active_tab(&self, host_id: FenceId, tab_id: Option<FenceId>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let Some(host) = inner.config.layouts[idx].fence_mut(host_id) else {
            return false;
        };
        if let Some(tab) = tab_id {
            if !host.tab_order.contains(&tab) {
                return false;
            }
        }
        host.active_tab = tab_id;
        true
    }

    /// 设置标签顺序。
    pub fn set_tab_order(&self, host_id: FenceId, order: Vec<FenceId>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        let Some(host) = inner.config.layouts[idx].fence_mut(host_id) else {
            return false;
        };
        // 只接受确实是该宿主标签页的 id
        let valid: Vec<FenceId> = order
            .into_iter()
            .filter(|id| host.tab_order.contains(id))
            .collect();
        host.tab_order = valid;
        true
    }

    // -----------------------------------------------------------------
    // 规则
    // -----------------------------------------------------------------

    pub fn rules(&self) -> RuleSet {
        self.inner.lock().unwrap().config.rules.clone()
    }

    pub fn set_rules(&self, rules: RuleSet) -> usize {
        let mut inner = self.inner.lock().unwrap();
        inner.config.rules = rules;
        apply_rules_locked(&mut inner)
    }

    /// 应用「快速添加」模板：新建栅栏 + 生成规则。
    pub fn apply_template(&self, template: Template, title: Option<&str>) -> Option<Fence> {
        let areas = monitor::enumerate();
        let primary = monitor::primary(&areas).cloned();
        let (monitor_id, work_w, work_h) = primary
            .as_ref()
            .map(|a| (a.device_path.clone(), a.width_dip(), a.height_dip()))
            .unwrap_or_else(|| ("DISPLAY1".to_string(), 1920.0, 1080.0));

        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_layout;
        if inner.config.layouts[idx].fences.len() >= MAX_FENCES_PER_LAYOUT {
            return None;
        }
        // 依次排布，避免叠在一起
        let count = inner.config.layouts[idx].fences.len() as f32;
        let x = 24.0 + (count % 4.0) * 40.0;
        let y = 24.0 + (count % 8.0) * 36.0;
        let fence = new_fence(
            title.unwrap_or(template.title()),
            &monitor_id,
            x,
            y,
            work_w,
            work_h,
        );
        inner.config.layouts[idx].fences.push(fence.clone());
        inner.config.rules.list.push(template.rule(fence.id));
        apply_rules_locked(&mut inner);
        Some(fence)
    }

    // -----------------------------------------------------------------
    // 快照
    // -----------------------------------------------------------------

    pub fn save_snapshot(&self, name: &str) -> Snapshot {
        let mut inner = self.inner.lock().unwrap();
        let snap = Snapshot {
            id: Uuid::new_v4(),
            name: name.to_string(),
            ts: desktop::now_secs(),
            layouts: inner.config.layouts.clone(),
        };
        inner.config.snapshots.push(snap.clone());
        // 超出上限时丢最旧的
        if inner.config.snapshots.len() > MAX_SNAPSHOTS {
            inner.config.snapshots.sort_by_key(|s| s.ts);
            let drop_n = inner.config.snapshots.len() - MAX_SNAPSHOTS;
            inner.config.snapshots.drain(..drop_n);
        }
        snap
    }

    pub fn restore_snapshot(&self, id: Uuid) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let Some(snap) = inner.config.snapshots.iter().find(|s| s.id == id).cloned() else {
            return false;
        };
        inner.config.layouts = snap.layouts;
        inner.active_layout = 0;
        drop(inner);
        self.ensure_layout();
        true
    }

    pub fn delete_snapshot(&self, id: Uuid) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let before = inner.config.snapshots.len();
        inner.config.snapshots.retain(|s| s.id != id);
        inner.config.snapshots.len() != before
    }

    // -----------------------------------------------------------------
    // 设置与持久化
    // -----------------------------------------------------------------

    pub fn set_hide_real_icons(&self, hide: bool) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.config.settings.hide_real_icons = hide;
        true
    }

    pub fn set_icon_size(&self, size: u32) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.config.settings.icon_size = size.clamp(16, 256);
        true
    }

    /// 持久化到磁盘，并广播变更事件。
    pub fn save(&self) -> std::io::Result<()> {
        let cfg = self.snapshot();
        let result = crate::fence::store::save(&cfg);
        if result.is_ok() {
            crate::fence::notify_changed();
        }
        result
    }

    /// 取条目（前端渲染用）。
    pub fn item(&self, id: ItemId) -> Option<Item> {
        self.inner.lock().unwrap().config.items.get(&id).cloned()
    }

    /// 所有条目。
    pub fn items(&self) -> HashMap<ItemId, Item> {
        self.inner.lock().unwrap().config.items.clone()
    }
}

impl Default for FenceService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 规则应用
// ---------------------------------------------------------------------------

/// 为条目构造规则引擎所需的事实。
pub fn facts_for(item: &Item) -> ItemFacts {
    let file_name = item
        .path()
        .and_then(|p| p.rsplit('\\').next().map(|s| s.to_string()))
        .unwrap_or_else(|| item.display_name.clone());
    let shortcut_info = item
        .path()
        .map(std::path::Path::new)
        .and_then(shortcut::resolve);
    let now = desktop::now_secs();
    let last_touch = item.mtime.max(item.last_opened.unwrap_or(0));
    let idle_days = if last_touch > 0 {
        Some(((now - last_touch).max(0) / 86_400) as u32)
    } else {
        None
    };
    let age_secs = if item.mtime > 0 {
        Some((now - item.mtime).max(0) as u64)
    } else {
        None
    };
    ItemFacts {
        file_name,
        is_folder: item.is_folder,
        size_bytes: item.size,
        shortcut_target: shortcut_info.as_ref().map(|s| s.target.clone()),
        shortcut_target_ext: shortcut_info.as_ref().and_then(|s| s.target_ext.clone()),
        created_minutes_local: local_minutes(item.mtime),
        created_weekday: local_weekday(item.mtime),
        origin: item.origin,
        is_hidden: item.attrs & 0x2 != 0,
        is_system: item.attrs & 0x4 != 0,
        idle_days,
        age_secs,
    }
}

/// Unix 秒 → 本地时间「距零点分钟数」。
fn local_minutes(ts: i64) -> Option<u16> {
    if ts <= 0 {
        return None;
    }
    use chrono::{Local, TimeZone};
    let dt = Local.timestamp_opt(ts, 0).single()?;
    use chrono::Timelike;
    Some((dt.hour() * 60 + dt.minute()) as u16)
}

/// Unix 秒 → 本地星期（0 = 周一）。
fn local_weekday(ts: i64) -> Option<u8> {
    if ts <= 0 {
        return None;
    }
    use chrono::{Datelike, Local, TimeZone};
    let dt = Local.timestamp_opt(ts, 0).single()?;
    Some(dt.weekday().num_days_from_monday() as u8)
}

/// 在已持有锁的情况下应用规则。返回被重新归类的条目数。
fn apply_rules_locked(inner: &mut Inner) -> usize {
    let idx = inner.active_layout;
    let rules = inner.config.rules.clone();
    let items: Vec<Item> = inner.config.items.values().cloned().collect();
    let fence_ids: Vec<FenceId> = inner.config.layouts[idx]
        .fences
        .iter()
        .map(|f| f.id)
        .collect();
    let inbox_id = inner.config.layouts[idx].inbox().map(|f| f.id);

    // 条目当前所在栅栏
    let mut current: HashMap<ItemId, FenceId> = HashMap::new();
    for fence in &inner.config.layouts[idx].fences {
        for r in &fence.items {
            current.insert(r.item_id, fence.id);
        }
    }

    let mut rerouted = 0;
    let mut desired: HashMap<ItemId, (FenceId, Option<Uuid>)> = HashMap::new();

    // 用户手动分配的条目不被规则抢走。
    let manual: std::collections::HashSet<ItemId> = inner.config.layouts[idx]
        .fences
        .iter()
        .flat_map(|f| f.items.iter())
        .filter(|r| matches!(r.assigned_by, crate::fence::model::AssignedBy::User))
        .map(|r| r.item_id)
        .collect();

    for item in &items {
        if manual.contains(&item.id) {
            continue;
        }
        let facts = facts_for(item);
        let decision = rules.evaluate(&facts);
        let target_fence = match decision {
            Decision::Skip => continue,
            Decision::Route { target, rule } => {
                let fid = resolve_target(target, &fence_ids, inbox_id);
                match fid {
                    Some(f) => (f, Some(rule)),
                    None => continue,
                }
            }
            Decision::Default(target) => {
                // 已在某个栅栏且设置为「保持不动」时，不迁移
                let already = current.get(&item.id).copied();
                if rules.unmatched == rules::Unmatched::Stay {
                    if let Some(f) = already {
                        (f, None)
                    } else {
                        match resolve_target(target, &fence_ids, inbox_id) {
                            Some(f) => (f, None),
                            None => continue,
                        }
                    }
                } else {
                    match resolve_target(target, &fence_ids, inbox_id) {
                        Some(f) => (f, None),
                        None => continue,
                    }
                }
            }
        };
        // 用户手动分配的条目不被规则抢走
        if current.get(&item.id) != Some(&target_fence.0) {
            rerouted += 1;
        }
        desired.insert(item.id, target_fence);
    }

    // 重建成员关系
    for fence in &mut inner.config.layouts[idx].fences {
        fence.items.retain(|r| !desired.contains_key(&r.item_id));
    }
    for (item_id, (fence_id, rule_id)) in desired {
        if let Some(f) = inner.config.layouts[idx].fence_mut(fence_id) {
            let assigned_by = match rule_id {
                Some(r) => crate::fence::model::AssignedBy::Rule(r),
                // 默认归类不能标成 User，否则下次规则就抢不回来了。
                None => crate::fence::model::AssignedBy::Default,
            };
            f.items.push(ItemRef {
                item_id,
                manual_index: None,
                assigned_by,
            });
        }
    }
    rerouted
}

fn resolve_target(
    target: Target,
    fence_ids: &[FenceId],
    inbox_id: Option<FenceId>,
) -> Option<FenceId> {
    match target {
        Target::Inbox => inbox_id,
        Target::Fence(id) => {
            if fence_ids.contains(&id) {
                Some(id)
            } else {
                inbox_id
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fence::model::{IconKey, ItemKey, Origin};

    fn item(name: &str, is_folder: bool) -> Item {
        Item {
            id: Uuid::new_v4(),
            key: ItemKey::from_path(&format!("C:\\Users\\A\\Desktop\\{name}")),
            origin: Origin::UserDesktop,
            display_name: name.to_string(),
            mtime: desktop::now_secs(),
            is_folder,
            attrs: 0,
            icon_key: IconKey::ByExt(".txt".into()),
            orphaned_since: None,
            size: 10,
            open_count: 0,
            last_opened: None,
            path: Some(format!("C:\\Users\\A\\Desktop\\{name}")),
        }
    }

    #[test]
    fn facts_extract_file_name_and_flags() {
        let mut it = item("Report.docx", false);
        it.attrs = 0x2; // hidden
        let f = facts_for(&it);
        assert_eq!(f.file_name, "Report.docx");
        assert!(f.is_hidden);
        assert!(!f.is_system);
        assert!(!f.is_folder);
    }

    #[test]
    fn facts_compute_idle_days() {
        let mut it = item("old.zip", false);
        it.mtime = desktop::now_secs() - 10 * 86_400;
        let f = facts_for(&it);
        assert_eq!(f.idle_days, Some(10));
    }

    #[test]
    fn local_time_helpers_handle_epoch() {
        assert_eq!(local_minutes(0), None);
        assert_eq!(local_weekday(0), None);
        // 一个已知时间点应能取到值
        assert!(local_minutes(1_700_000_000).is_some());
        assert!(local_weekday(1_700_000_000).is_some());
    }

    #[test]
    fn resolve_target_falls_back_to_inbox_for_missing_fence() {
        let inbox = Uuid::new_v4();
        let missing = Uuid::new_v4();
        assert_eq!(
            resolve_target(Target::Fence(missing), &[inbox], Some(inbox)),
            Some(inbox)
        );
        assert_eq!(
            resolve_target(Target::Inbox, &[inbox], Some(inbox)),
            Some(inbox)
        );
    }

    #[test]
    fn resolve_target_returns_none_without_inbox() {
        assert_eq!(resolve_target(Target::Inbox, &[], None), None);
    }

    /// 构造一个只含收件箱的空配置。
    fn inner_with_inbox() -> (Inner, FenceId) {
        let mut inner = Inner {
            config: Config::default(),
            active_layout: 0,
            last_desktop_path: None,
        };
        let inbox = new_inbox("DISPLAY1", 1920.0, 1080.0);
        let inbox_id = inbox.id;
        let mut layout = Layout::new(vec![]);
        layout.fences.push(inbox);
        inner.config.layouts.push(layout);
        (inner, inbox_id)
    }

    fn items_in(inner: &Inner, fence_id: FenceId) -> Vec<ItemId> {
        inner.config.layouts[0]
            .fence(fence_id)
            .map(|f| f.items.iter().map(|r| r.item_id).collect())
            .unwrap_or_default()
    }

    #[test]
    fn default_routing_marks_items_as_default_not_user() {
        let (mut inner, inbox_id) = inner_with_inbox();
        let a = item("a.txt", false);
        let b = item("b.txt", false);
        inner.config.items.insert(a.id, a.clone());
        inner.config.items.insert(b.id, b.clone());

        apply_rules_locked(&mut inner);

        let refs = &inner.config.layouts[0].fence(inbox_id).unwrap().items;
        assert_eq!(refs.len(), 2);
        assert!(
            refs.iter()
                .all(|r| matches!(r.assigned_by, crate::fence::model::AssignedBy::Default)),
            "默认归类不能标成 User，否则规则永远抢不回来"
        );
    }

    #[test]
    fn rules_can_take_over_previously_default_routed_items() {
        let (mut inner, inbox_id) = inner_with_inbox();
        let exe = item("app.exe", false);
        let txt = item("note.txt", false);
        inner.config.items.insert(exe.id, exe.clone());
        inner.config.items.insert(txt.id, txt.clone());

        // 第一轮：没有规则，全部进收件箱
        apply_rules_locked(&mut inner);
        assert_eq!(items_in(&inner, inbox_id).len(), 2);

        // 加一个「程序」栅栏 + 规则
        let programs = new_fence("程序", "DISPLAY1", 100.0, 100.0, 1920.0, 1080.0);
        let programs_id = programs.id;
        inner.config.layouts[0].fences.push(programs);
        inner
            .config
            .rules
            .list
            .push(Template::Programs.rule(programs_id));

        // 第二轮：exe 应被规则抢走
        let rerouted = apply_rules_locked(&mut inner);
        assert_eq!(rerouted, 1, "应只有 exe 被重新归类");
        assert_eq!(items_in(&inner, programs_id), vec![exe.id]);
        assert_eq!(items_in(&inner, inbox_id), vec![txt.id]);
    }

    #[test]
    fn user_assigned_items_are_never_stolen_by_rules() {
        let (mut inner, inbox_id) = inner_with_inbox();
        let exe = item("app.exe", false);
        inner.config.items.insert(exe.id, exe.clone());

        // 用户手动把 exe 放进收件箱
        inner.config.layouts[0]
            .fence_mut(inbox_id)
            .unwrap()
            .items
            .push(ItemRef {
                item_id: exe.id,
                manual_index: None,
                assigned_by: crate::fence::model::AssignedBy::User,
            });

        let programs = new_fence("程序", "DISPLAY1", 100.0, 100.0, 1920.0, 1080.0);
        let programs_id = programs.id;
        inner.config.layouts[0].fences.push(programs);
        inner
            .config
            .rules
            .list
            .push(Template::Programs.rule(programs_id));

        apply_rules_locked(&mut inner);

        assert_eq!(items_in(&inner, programs_id), Vec::<ItemId>::new());
        assert_eq!(items_in(&inner, inbox_id), vec![exe.id]);
    }
}
