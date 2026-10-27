import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  applyDesktopLayer,
  assignItem,
  openItem,
  setActiveTab,
  setRolledUp,
  setView,
  syncWindows,
  unassignItem,
} from "./api";
import type { Fence, FenceView, Item, ViewLayout } from "./types";
import { useFenceConfig, useIcons } from "./useFenceConfig";
import "./fence.css";

/** 视图模式循环顺序。 */
const VIEW_CYCLE: ViewLayout[] = ["grid", "list", "details"];

const VIEW_LABEL: Record<ViewLayout, string> = {
  grid: "图标",
  list: "列表",
  details: "详情",
};

interface Props {
  fenceId: string;
}

/**
 * 单个栅栏窗口的内容。
 *
 * 每个栅栏是一个独立的透明窗口，只渲染自己这一块；
 * 标签页宿主会把当前激活标签页的内容画在自己的区域里。
 */
export function FenceOverlay({ fenceId }: Props) {
  const { config, error } = useFenceConfig();
  const [menu, setMenu] = useState<{ x: number; y: number; itemId: string } | null>(null);

  // 窗口落到桌面层（不抢焦点、压在普通窗口之下）
  useEffect(() => {
    void applyDesktopLayer(getCurrentWebviewWindow().label).catch(() => {
      // 非 Windows 或桌面不可用时忽略
    });
  }, []);

  const layout = config?.layouts?.[0];
  const fence = useMemo(
    () => layout?.fences.find((f) => f.id === fenceId) ?? null,
    [layout, fenceId],
  );

  // 当前实际显示的栅栏：宿主显示激活标签页，否则显示自己
  const shown: Fence | null = useMemo(() => {
    if (!fence) return null;
    if (fence.activeTab) {
      return layout?.fences.find((f) => f.id === fence.activeTab) ?? fence;
    }
    return fence;
  }, [fence, layout]);

  const items = useMemo(() => {
    if (!shown || !config) return [];
    const refs = [...shown.items];
    if (shown.view.sort === "manual" || shown.view.sort === "name") {
      // 手动顺序优先，其次按名称
      refs.sort((a, b) => {
        const ai = a.manualIndex ?? Number.MAX_SAFE_INTEGER;
        const bi = b.manualIndex ?? Number.MAX_SAFE_INTEGER;
        if (ai !== bi) return ai - bi;
        const an = config.items[a.itemId]?.displayName ?? "";
        const bn = config.items[b.itemId]?.displayName ?? "";
        return an.localeCompare(bn, "zh-CN");
      });
    } else if (shown.view.sort === "date") {
      refs.sort(
        (a, b) => (config.items[b.itemId]?.mtime ?? 0) - (config.items[a.itemId]?.mtime ?? 0),
      );
    } else if (shown.view.sort === "size") {
      refs.sort(
        (a, b) => (config.items[b.itemId]?.size ?? 0) - (config.items[a.itemId]?.size ?? 0),
      );
    }
    const list = refs.map((r) => config.items[r.itemId]).filter((it): it is Item => Boolean(it));
    return shown.view.reverse ? list.reverse() : list;
  }, [shown, config]);

  const itemIds = useMemo(() => items.map((i) => i.id), [items]);
  const icons = useIcons(itemIds, shown?.view.iconSize ?? 48);

  const tabs = useMemo(() => {
    if (!fence || fence.tabOrder.length === 0) return [];
    return fence.tabOrder
      .map((id) => layout?.fences.find((f) => f.id === id))
      .filter((f): f is Fence => Boolean(f));
  }, [fence, layout]);

  const toggleRoll = useCallback(async () => {
    if (!fence) return;
    await setRolledUp(fence.id, !fence.rolledUp);
    await syncWindows();
  }, [fence]);

  const cycleView = useCallback(async () => {
    if (!shown) return;
    const idx = VIEW_CYCLE.indexOf(shown.view.layout);
    const next: FenceView = {
      ...shown.view,
      layout: VIEW_CYCLE[(idx + 1) % VIEW_CYCLE.length],
    };
    await setView(shown.id, next);
  }, [shown]);

  const handleOpen = useCallback(async (item: Item) => {
    try {
      await openItem(item.id);
    } catch {
      // 打开失败时保持界面不变
    }
  }, []);

  const handleMoveTo = useCallback(async (itemId: string, targetFenceId: string) => {
    await assignItem(itemId, targetFenceId);
    setMenu(null);
  }, []);

  const handleRemove = useCallback(async (itemId: string) => {
    await unassignItem(itemId);
    setMenu(null);
  }, []);

  // 点击空白关闭右键菜单
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("click", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("blur", close);
    };
  }, [menu]);

  if (error) {
    return (
      <div className="fence-shell fence-error">
        <div className="fence-error-title">桌面整理不可用</div>
        <div className="fence-error-body">{error}</div>
      </div>
    );
  }

  if (!fence) {
    return <div className="fence-shell fence-loading">加载中…</div>;
  }

  // 标签页宿主显示自己的标题，当前标签由标签条高亮表示。
  const title = fence.title;
  const count = items.length;
  const otherFences = (layout?.fences ?? []).filter(
    (f) => f.id !== shown?.id && f.tabHost === null,
  );

  return (
    <div className={`fence-shell fence-${shown?.view.layout ?? "grid"}`}>
      {/* 标题栏：可拖动整个窗口 */}
      <div className="fence-titlebar" data-tauri-drag-region>
        <span className="fence-title" data-tauri-drag-region>
          {title}
        </span>
        <span className="fence-count">{count}</span>
        <div className="fence-actions">
          <button
            type="button"
            className="fence-action"
            title={`切换视图（当前：${VIEW_LABEL[shown?.view.layout ?? "grid"]}）`}
            onClick={cycleView}
          >
            {VIEW_LABEL[shown?.view.layout ?? "grid"]}
          </button>
          <button
            type="button"
            className="fence-action"
            title={fence.rolledUp ? "展开" : "卷起"}
            onClick={toggleRoll}
          >
            {fence.rolledUp ? "▾" : "▴"}
          </button>
        </div>
      </div>

      {/* 标签页条 */}
      {tabs.length > 0 && (
        <div className="fence-tabs">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              className={`fence-tab ${tab.id === shown?.id ? "is-active" : ""}`}
              onClick={() => void setActiveTab(fence.id, tab.id)}
            >
              {tab.title}
              <span className="fence-tab-count">{tab.items.length}</span>
            </button>
          ))}
        </div>
      )}

      {/* 内容区 */}
      {!fence.rolledUp && (
        <div className="fence-body">
          {items.length === 0 ? (
            <div className="fence-empty">空栅栏 · 把桌面文件拖进来</div>
          ) : (
            <div className={`fence-items fence-items-${shown?.view.layout ?? "grid"}`}>
              {items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className="fence-item"
                  title={item.path ?? item.displayName}
                  onDoubleClick={() => void handleOpen(item)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ x: e.clientX, y: e.clientY, itemId: item.id });
                  }}
                >
                  <span className="fence-item-icon">
                    {icons[item.id] ? (
                      <img src={icons[item.id]} alt="" draggable={false} />
                    ) : (
                      <span className="fence-item-icon-fallback">
                        {item.isFolder ? "📁" : "📄"}
                      </span>
                    )}
                  </span>
                  <span className="fence-item-name">{item.displayName}</span>
                  {shown?.view.layout === "details" && (
                    <>
                      <span className="fence-item-meta">{formatSize(item.size)}</span>
                      <span className="fence-item-meta">{formatDate(item.mtime)}</span>
                    </>
                  )}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {/* 右键菜单 */}
      {menu && (
        <div
          className="fence-menu"
          style={{ left: menu.x, top: menu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="fence-menu-item"
            onClick={() => {
              const item = items.find((i) => i.id === menu.itemId);
              if (item) void handleOpen(item);
              setMenu(null);
            }}
          >
            打开
          </button>
          <div className="fence-menu-sep" />
          {otherFences.map((f) => (
            <button
              key={f.id}
              type="button"
              className="fence-menu-item"
              onClick={() => void handleMoveTo(menu.itemId, f.id)}
            >
              移动到「{f.title}」
            </button>
          ))}
          <div className="fence-menu-sep" />
          <button
            type="button"
            className="fence-menu-item"
            onClick={() => void handleRemove(menu.itemId)}
          >
            移出栅栏
          </button>
        </div>
      )}
    </div>
  );
}

function formatSize(bytes: number): string {
  if (!bytes) return "—";
  const units = ["B", "KB", "MB", "GB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function formatDate(secs: number): string {
  if (!secs) return "—";
  const d = new Date(secs * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate(),
  ).padStart(2, "0")}`;
}
