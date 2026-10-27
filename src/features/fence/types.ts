/**
 * 桌面栅栏前端类型定义。
 *
 * 与 Rust 侧 `src-tauri/src/fence/model.rs` 一一对应；
 * 所有结构体在 Rust 端都是 `#[serde(rename_all = "camelCase")]`。
 */

export type FenceId = string;
export type ItemId = string;

export type FenceKind = "inbox" | "manual" | "rule" | "portal";

export type Anchor =
  | "leftTop"
  | "rightTop"
  | "leftBottom"
  | "rightBottom"
  | "hCenterTop"
  | "hCenterBottom"
  | "leftVCenter"
  | "rightVCenter"
  | "center";

export type SortMode = "name" | "date" | "type" | "size" | "manual" | "openCount";

export type ViewLayout = "grid" | "list" | "details";

export type Spacing = "compact" | "normal" | "loose";

export type TitleSize = "small" | "normal" | "large";

export type ZOrderSetting = "desktop" | "normal" | "topmost";

export type Origin = "userDesktop" | "publicDesktop" | "other";

/** 条目归属来源：用户手动 / 规则归类 / 默认兜底。 */
export type AssignedBy = "user" | "default" | { rule: string };

export type IconKey =
  | { byExt: string }
  | { byPath: string }
  | { byFolder: boolean }
  | { byShell: string };

export type ItemKey = { path: string } | { name: string } | { shellId: string };

export interface NormGeometry {
  monitor: string;
  x: number;
  y: number;
  w: number;
  h: number;
  workW: number;
  workH: number;
  anchor: Anchor;
}

export interface FenceView {
  iconSize: number;
  sort: SortMode;
  labelLines: number;
  autoHeight: boolean;
  reverse: boolean;
  layout: ViewLayout;
  spacing: Spacing;
  groupByDate: boolean;
  columnWidths?: [number, number, number] | null;
  columnsVisible?: [boolean, boolean, boolean] | null;
}

export interface AppearanceOverride {
  tintRgb?: [number, number, number] | null;
  opacity?: number | null;
  titleRgb?: [number, number, number] | null;
  titleSize?: TitleSize | null;
  titleAutoHide?: boolean | null;
}

export interface ItemRef {
  itemId: ItemId;
  manualIndex?: number | null;
  assignedBy: AssignedBy;
}

export interface Fence {
  id: FenceId;
  title: string;
  kind: FenceKind;
  source: unknown;
  geometry: NormGeometry;
  rolledUp: boolean;
  expandedH: number;
  view: FenceView;
  appearance?: AppearanceOverride | null;
  locked: boolean;
  tabHost?: FenceId | null;
  activeTab?: FenceId | null;
  tabOrder: FenceId[];
  portalNavigate: boolean;
  hideTitleIcon: boolean;
  hideWhenEmpty: boolean;
  items: ItemRef[];
}

export interface Item {
  id: ItemId;
  key: ItemKey;
  origin: Origin;
  displayName: string;
  mtime: number;
  isFolder: boolean;
  attrs: number;
  iconKey: IconKey;
  orphanedSince?: number | null;
  size: number;
  openCount: number;
  lastOpened?: number | null;
  path?: string | null;
}

export interface MonitorIdentity {
  devicePath: string;
  gdiName: string;
  workDip: [number, number];
  dpi: number;
  screenPx: [number, number];
}

export interface Layout {
  fingerprint: MonitorIdentity[];
  fences: Fence[];
  shown: number;
}

export interface SnappingSettings {
  enabled: boolean;
  threshold: number;
  toFences: boolean;
  toEdges: boolean;
}

export interface Settings {
  hideRealIcons: boolean;
  iconSize: number;
  snapping: SnappingSettings;
  zorder: ZOrderSetting;
  rollUpOnDoubleClick: boolean;
  offerNewFence: boolean;
  desktopPath?: string | null;
  lastSync?: number | null;
}

// ---------------------------------------------------------------------------
// 规则
// ---------------------------------------------------------------------------

export type Target = "inbox" | { fence: FenceId };

export type Class = "target" | "content" | "time" | "state";

export type Cond =
  | { type: string }
  | { ext: string }
  | { exactName: string }
  | { name: string }
  | { glob: string }
  | { shortcutTarget: string }
  | { foldersOnly: null }
  | { filesOnly: null }
  | { plainFiles: null }
  | { sizeMb: [number | null, number | null] }
  | { createdTime: [number, number] }
  | { createdWeekday: number[] }
  | { origin: Origin }
  | { idleDays: number };

export interface Rule {
  id: string;
  name: string;
  enabled: boolean;
  target: Target;
  allOf: Cond[];
  priorityClass: Class;
  template?: string | null;
}

export type Unmatched = "inbox" | "stay";

export interface RuleSet {
  defaultTarget: Target;
  keepUpdated: boolean;
  unmatched: Unmatched;
  list: Rule[];
}

export interface Config {
  schemaVersion: number;
  settings: Settings;
  items: Record<ItemId, Item>;
  layouts: Layout[];
  rules: RuleSet;
  snapshots: Snapshot[];
}

export interface Snapshot {
  id: string;
  name: string;
  ts: number;
  layouts: Layout[];
}

// ---------------------------------------------------------------------------
// 命令返回
// ---------------------------------------------------------------------------

export interface SyncReport {
  added: number;
  removed: number;
  rerouted: number;
  total: number;
  desktopPath?: string | null;
}

export interface MonitorInfo {
  devicePath: string;
  gdiName: string;
  workDip: [number, number];
  originDip: [number, number];
  dpi: number;
  screenPx: [number, number];
}

export interface DesktopStatus {
  available: boolean;
  iconsVisible: boolean;
  desktopPath?: string | null;
}

export interface TemplateInfo {
  key: string;
  title: string;
}
