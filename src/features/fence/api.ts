import { invoke } from "@tauri-apps/api/core";
import type {
  Config,
  DesktopStatus,
  Fence,
  FenceId,
  FenceView,
  Item,
  ItemId,
  MonitorInfo,
  RuleSet,
  Snapshot,
  SyncReport,
  TemplateInfo,
} from "./types";

// ---------------------------------------------------------------------------
// 配置
// ---------------------------------------------------------------------------

export function getConfig(): Promise<Config> {
  return invoke("fence_get_config");
}

export function getMonitors(): Promise<MonitorInfo[]> {
  return invoke("fence_get_monitors");
}

export function getDesktopStatus(): Promise<DesktopStatus> {
  return invoke("fence_desktop_available");
}

// ---------------------------------------------------------------------------
// 桌面同步
// ---------------------------------------------------------------------------

export function syncDesktop(): Promise<SyncReport> {
  return invoke("fence_sync_desktop");
}

export function applyRules(): Promise<number> {
  return invoke("fence_apply_rules");
}

// ---------------------------------------------------------------------------
// 栅栏 CRUD
// ---------------------------------------------------------------------------

export function createFence(
  title: string,
  x: number,
  y: number,
  w: number,
  h: number,
): Promise<Fence> {
  return invoke("fence_create", { title, x, y, w, h });
}

export function deleteFence(id: FenceId): Promise<boolean> {
  return invoke("fence_delete", { id });
}

export function renameFence(id: FenceId, title: string): Promise<boolean> {
  return invoke("fence_rename", { id, title });
}

export function setGeometry(
  id: FenceId,
  x: number,
  y: number,
  w: number,
  h: number,
): Promise<boolean> {
  return invoke("fence_set_geometry", { id, x, y, w, h });
}

export function setRolledUp(id: FenceId, rolled: boolean): Promise<boolean> {
  return invoke("fence_set_rolled_up", { id, rolled });
}

export function setView(id: FenceId, view: FenceView): Promise<boolean> {
  return invoke("fence_set_view", { id, view });
}

// ---------------------------------------------------------------------------
// 条目
// ---------------------------------------------------------------------------

export function assignItem(itemId: ItemId, fenceId: FenceId): Promise<boolean> {
  return invoke("fence_assign_item", { itemId, fenceId });
}

export function unassignItem(itemId: ItemId): Promise<boolean> {
  return invoke("fence_unassign_item", { itemId });
}

export function reorderItems(fenceId: FenceId, ordered: ItemId[]): Promise<boolean> {
  return invoke("fence_reorder_items", { fenceId, ordered });
}

export function getItem(id: ItemId): Promise<Item | null> {
  return invoke("fence_get_item", { id });
}

// ---------------------------------------------------------------------------
// 标签页
// ---------------------------------------------------------------------------

export function makeTab(fenceId: FenceId, hostId: FenceId): Promise<boolean> {
  return invoke("fence_make_tab", { fenceId, hostId });
}

export function splitTab(fenceId: FenceId): Promise<boolean> {
  return invoke("fence_split_tab", { fenceId });
}

export function setActiveTab(hostId: FenceId, tabId: FenceId | null): Promise<boolean> {
  return invoke("fence_set_active_tab", { hostId, tabId });
}

export function setTabOrder(hostId: FenceId, order: FenceId[]): Promise<boolean> {
  return invoke("fence_set_tab_order", { hostId, order });
}

// ---------------------------------------------------------------------------
// 规则
// ---------------------------------------------------------------------------

export function getRules(): Promise<RuleSet> {
  return invoke("fence_get_rules");
}

export function setRules(rules: RuleSet): Promise<number> {
  return invoke("fence_set_rules", { rules });
}

export function listTemplates(): Promise<TemplateInfo[]> {
  return invoke("fence_list_templates");
}

export function applyTemplate(key: string, title?: string): Promise<Fence | null> {
  return invoke("fence_apply_template", { key, title: title ?? null });
}

// ---------------------------------------------------------------------------
// 快照
// ---------------------------------------------------------------------------

export function saveSnapshot(name: string): Promise<Snapshot> {
  return invoke("fence_save_snapshot", { name });
}

export function restoreSnapshot(id: string): Promise<boolean> {
  return invoke("fence_restore_snapshot", { id });
}

export function deleteSnapshot(id: string): Promise<boolean> {
  return invoke("fence_delete_snapshot", { id });
}

// ---------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------

export function setHideRealIcons(hide: boolean): Promise<boolean> {
  return invoke("fence_set_hide_real_icons", { hide });
}

export function setIconSize(size: number): Promise<boolean> {
  return invoke("fence_set_icon_size", { size });
}

// ---------------------------------------------------------------------------
// 图标与打开
// ---------------------------------------------------------------------------

export function getIcon(itemId: ItemId, sizePx = 48): Promise<string | null> {
  return invoke("fence_get_icon", { itemId, sizePx });
}

export function clearIconCache(): Promise<number> {
  return invoke("fence_clear_icon_cache");
}

export function openItem(itemId: ItemId): Promise<boolean> {
  return invoke("fence_open_item", { itemId });
}

// ---------------------------------------------------------------------------
// 窗口
// ---------------------------------------------------------------------------

export function openManager(): Promise<boolean> {
  return invoke("fence_open_manager");
}

export function syncWindows(): Promise<number> {
  return invoke("fence_sync_windows");
}

export function openFenceWindow(id: FenceId): Promise<boolean> {
  return invoke("fence_open_window", { id });
}

export function closeFenceWindow(id: FenceId): Promise<boolean> {
  return invoke("fence_close_window", { id });
}

export function applyDesktopLayer(label: string): Promise<boolean> {
  return invoke("fence_apply_desktop_layer", { label });
}
