import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyTemplate,
  clearIconCache,
  createFence,
  deleteFence,
  deleteSnapshot,
  getDesktopStatus,
  getMonitors,
  listTemplates,
  makeTab,
  openFenceWindow,
  renameFence,
  restoreSnapshot,
  saveSnapshot,
  setActiveTab,
  setHideRealIcons,
  setIconSize,
  setRolledUp,
  setRules,
  splitTab,
  syncDesktop,
  syncWindows,
} from "./api";
import type {
  DesktopStatus,
  Fence,
  MonitorInfo,
  RuleSet,
  SyncReport,
  TemplateInfo,
  ViewLayout,
} from "./types";
import { useFenceConfig } from "./useFenceConfig";

type TabKey = "fences" | "rules" | "snapshots" | "settings";

const TABS: { key: TabKey; label: string }[] = [
  { key: "fences", label: "栅栏" },
  { key: "rules", label: "规则" },
  { key: "snapshots", label: "快照" },
  { key: "settings", label: "设置" },
];

const VIEW_LABEL: Record<ViewLayout, string> = {
  grid: "图标",
  list: "列表",
  details: "详情",
};

/** 桌面整理管理窗口。 */
export function FenceManager() {
  const { config, error, reload } = useFenceConfig();
  const [tab, setTab] = useState<TabKey>("fences");
  const [status, setStatus] = useState<DesktopStatus | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [snapshotName, setSnapshotName] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await getDesktopStatus());
      setMonitors(await getMonitors());
    } catch {
      // 桌面不可用时保持空状态
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
    void listTemplates()
      .then(setTemplates)
      .catch(() => {});
  }, [refreshStatus]);

  const layout = config?.layouts?.[0];
  const fences = useMemo(() => layout?.fences ?? [], [layout]);
  const hosts = useMemo(() => fences.filter((f) => f.tabHost === null), [fences]);

  const run = useCallback(
    async (fn: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await fn();
        await reload();
      } catch (err) {
        setReport({ added: 0, removed: 0, rerouted: 0, total: 0, desktopPath: String(err) });
      } finally {
        setBusy(false);
      }
    },
    [reload],
  );

  const handleSync = useCallback(async () => {
    setBusy(true);
    try {
      const r = await syncDesktop();
      setReport(r);
      await reload();
      await syncWindows();
    } finally {
      setBusy(false);
    }
  }, [reload]);

  const handleCreate = useCallback(async () => {
    const title = newTitle.trim() || "新栅栏";
    await run(async () => {
      await createFence(title, 40, 40, 320, 260);
      await syncWindows();
    });
    setNewTitle("");
  }, [newTitle, run]);

  const handleRename = useCallback(
    async (fence: Fence) => {
      const title = renameValue.trim();
      if (title) await run(() => renameFence(fence.id, title));
      setRenaming(null);
    },
    [renameValue, run],
  );

  const handleToggleRoll = useCallback(
    async (fence: Fence) => {
      await run(async () => {
        await setRolledUp(fence.id, !fence.rolledUp);
        await syncWindows();
      });
    },
    [run],
  );

  const handleDelete = useCallback(
    async (fence: Fence) => {
      await run(async () => {
        await deleteFence(fence.id);
        await syncWindows();
      });
    },
    [run],
  );

  const handleMakeTab = useCallback(
    async (fence: Fence, hostId: string) => {
      await run(async () => {
        await makeTab(fence.id, hostId);
        await syncWindows();
      });
    },
    [run],
  );

  const handleSplitTab = useCallback(
    async (fence: Fence) => {
      await run(async () => {
        await splitTab(fence.id);
        await syncWindows();
      });
    },
    [run],
  );

  const handleToggleRule = useCallback(
    async (rules: RuleSet, ruleId: string) => {
      const next: RuleSet = {
        ...rules,
        list: rules.list.map((r) => (r.id === ruleId ? { ...r, enabled: !r.enabled } : r)),
      };
      await run(() => setRules(next));
    },
    [run],
  );

  const handleDeleteRule = useCallback(
    async (rules: RuleSet, ruleId: string) => {
      const next: RuleSet = { ...rules, list: rules.list.filter((r) => r.id !== ruleId) };
      await run(() => setRules(next));
    },
    [run],
  );

  const handleTemplate = useCallback(
    async (key: string) => {
      await run(async () => {
        await applyTemplate(key);
        await syncWindows();
      });
    },
    [run],
  );

  if (error) {
    return (
      <div className="flex h-screen items-center justify-center bg-paper p-8 text-ink">
        <div className="max-w-md text-center">
          <div className="mb-2 text-lg font-semibold">桌面整理加载失败</div>
          <div className="text-sm text-ink-faint">{error}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col bg-paper text-ink">
      {/* 顶栏 */}
      <header className="flex items-center gap-3 border-b border-ink-ghost/20 px-5 py-3">
        <h1 className="font-display text-lg font-semibold">桌面整理</h1>
        {status && !status.available && (
          <span className="rounded-full bg-danger-bg px-2 py-0.5 text-xs text-red-600">
            桌面不可用
          </span>
        )}
        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            className="rounded-lg bg-bamboo px-3 py-1.5 text-sm text-white disabled:opacity-50"
            onClick={() => void handleSync()}
            disabled={busy}
          >
            {busy ? "同步中…" : "同步桌面"}
          </button>
          <button
            type="button"
            className="rounded-lg border border-ink-ghost/30 px-3 py-1.5 text-sm disabled:opacity-50"
            onClick={() => void run(syncWindows)}
            disabled={busy}
          >
            重建窗口
          </button>
        </div>
      </header>

      {/* 同步结果 */}
      {report && (
        <div className="border-b border-ink-ghost/20 bg-bamboo-mist px-5 py-2 text-xs text-ink-soft">
          新增 {report.added} · 消失 {report.removed} · 重新归类 {report.rerouted} · 总计{" "}
          {report.total}
          {report.desktopPath ? ` · ${report.desktopPath}` : ""}
        </div>
      )}

      {/* 标签页 */}
      <nav className="flex gap-1 border-b border-ink-ghost/20 px-5">
        {TABS.map((t) => (
          <button
            key={t.key}
            type="button"
            className={`-mb-px border-b-2 px-3 py-2 text-sm ${
              tab === t.key
                ? "border-bamboo text-bamboo"
                : "border-transparent text-ink-faint hover:text-ink"
            }`}
            onClick={() => setTab(t.key)}
          >
            {t.label}
          </button>
        ))}
      </nav>

      <main className="flex-1 overflow-y-auto px-5 py-4">
        {tab === "fences" && (
          <section className="space-y-4">
            {/* 新建 */}
            <div className="flex items-center gap-2">
              <input
                className="w-56 rounded-lg border border-ink-ghost/30 bg-paper-warm px-3 py-1.5 text-sm outline-none focus:border-bamboo"
                placeholder="新栅栏名称"
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleCreate();
                }}
              />
              <button
                type="button"
                className="rounded-lg bg-bamboo px-3 py-1.5 text-sm text-white disabled:opacity-50"
                onClick={() => void handleCreate()}
                disabled={busy}
              >
                新建栅栏
              </button>
              <span className="text-xs text-ink-faint">共 {fences.length} 个</span>
            </div>

            {/* 快速模板 */}
            {templates.length > 0 && (
              <div>
                <div className="mb-2 text-xs font-medium text-ink-faint">快速添加</div>
                <div className="flex flex-wrap gap-2">
                  {templates.map((t) => (
                    <button
                      key={t.key}
                      type="button"
                      className="rounded-full border border-ink-ghost/30 px-3 py-1 text-xs hover:border-bamboo hover:text-bamboo disabled:opacity-50"
                      onClick={() => void handleTemplate(t.key)}
                      disabled={busy}
                    >
                      {t.title}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {/* 栅栏列表 */}
            <div className="space-y-2">
              {fences.length === 0 && (
                <div className="rounded-xl border border-dashed border-ink-ghost/40 p-8 text-center text-sm text-ink-faint">
                  还没有栅栏，先点「同步桌面」再新建一个
                </div>
              )}
              {fences.map((fence) => (
                <FenceRow
                  key={fence.id}
                  fence={fence}
                  hosts={hosts}
                  renaming={renaming === fence.id}
                  renameValue={renameValue}
                  onStartRename={() => {
                    setRenaming(fence.id);
                    setRenameValue(fence.title);
                  }}
                  onRenameValue={setRenameValue}
                  onCommitRename={() => void handleRename(fence)}
                  onCancelRename={() => setRenaming(null)}
                  onToggleRoll={() => void handleToggleRoll(fence)}
                  onDelete={() => void handleDelete(fence)}
                  onOpen={() => void openFenceWindow(fence.id)}
                  onMakeTab={(hostId) => void handleMakeTab(fence, hostId)}
                  onSplitTab={() => void handleSplitTab(fence)}
                  onSetActiveTab={(tabId) => void run(() => setActiveTab(fence.id, tabId))}
                  busy={busy}
                />
              ))}
            </div>
          </section>
        )}

        {tab === "rules" && config && (
          <RulesPanel
            rules={config.rules}
            onToggle={(id) => void handleToggleRule(config.rules, id)}
            onDelete={(id) => void handleDeleteRule(config.rules, id)}
            busy={busy}
          />
        )}

        {tab === "snapshots" && config && (
          <section className="space-y-4">
            <div className="flex items-center gap-2">
              <input
                className="w-56 rounded-lg border border-ink-ghost/30 bg-paper-warm px-3 py-1.5 text-sm outline-none focus:border-bamboo"
                placeholder="快照名称"
                value={snapshotName}
                onChange={(e) => setSnapshotName(e.target.value)}
              />
              <button
                type="button"
                className="rounded-lg bg-bamboo px-3 py-1.5 text-sm text-white disabled:opacity-50"
                onClick={() =>
                  void run(async () => {
                    await saveSnapshot(snapshotName.trim() || "未命名快照");
                    setSnapshotName("");
                  })
                }
                disabled={busy}
              >
                保存当前布局
              </button>
            </div>
            <div className="space-y-2">
              {config.snapshots.length === 0 && (
                <div className="rounded-xl border border-dashed border-ink-ghost/40 p-8 text-center text-sm text-ink-faint">
                  还没有快照
                </div>
              )}
              {[...config.snapshots].reverse().map((snap) => (
                <div
                  key={snap.id}
                  className="flex items-center gap-3 rounded-xl border border-ink-ghost/20 bg-paper-warm px-4 py-2.5"
                >
                  <div className="flex-1">
                    <div className="text-sm font-medium">{snap.name}</div>
                    <div className="text-xs text-ink-faint">
                      {new Date(snap.ts * 1000).toLocaleString("zh-CN")} ·{" "}
                      {snap.layouts.reduce((n, l) => n + l.fences.length, 0)} 个栅栏
                    </div>
                  </div>
                  <button
                    type="button"
                    className="rounded-lg border border-ink-ghost/30 px-2.5 py-1 text-xs disabled:opacity-50"
                    onClick={() =>
                      void run(async () => {
                        await restoreSnapshot(snap.id);
                        await syncWindows();
                      })
                    }
                    disabled={busy}
                  >
                    恢复
                  </button>
                  <button
                    type="button"
                    className="rounded-lg border border-ink-ghost/30 px-2.5 py-1 text-xs text-red-600 disabled:opacity-50"
                    onClick={() => void run(() => deleteSnapshot(snap.id))}
                    disabled={busy}
                  >
                    删除
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}

        {tab === "settings" && config && (
          <section className="max-w-xl space-y-5">
            <label className="flex items-center justify-between gap-4 rounded-xl border border-ink-ghost/20 bg-paper-warm px-4 py-3">
              <span>
                <span className="block text-sm font-medium">隐藏原生桌面图标</span>
                <span className="block text-xs text-ink-faint">
                  开启后桌面只显示栅栏，退出应用会自动恢复
                </span>
              </span>
              <input
                type="checkbox"
                className="h-4 w-4 accent-[var(--color-bamboo)]"
                checked={config.settings.hideRealIcons}
                onChange={(e) => void run(() => setHideRealIcons(e.target.checked))}
              />
            </label>

            <div className="rounded-xl border border-ink-ghost/20 bg-paper-warm px-4 py-3">
              <div className="mb-2 flex items-center justify-between">
                <span className="text-sm font-medium">默认图标大小</span>
                <span className="text-xs text-ink-faint">{config.settings.iconSize} px</span>
              </div>
              <input
                type="range"
                min={16}
                max={128}
                step={8}
                className="w-full accent-[var(--color-bamboo)]"
                value={config.settings.iconSize}
                onChange={(e) => void run(() => setIconSize(Number(e.target.value)))}
              />
            </div>

            <div className="rounded-xl border border-ink-ghost/20 bg-paper-warm px-4 py-3 text-xs text-ink-soft">
              <div className="mb-1 font-medium text-ink">显示器</div>
              {monitors.length === 0 && <div className="text-ink-faint">未检测到</div>}
              {monitors.map((m) => (
                <div key={m.devicePath} className="flex justify-between gap-3 py-0.5">
                  <span className="truncate">{m.gdiName || m.devicePath}</span>
                  <span className="shrink-0 text-ink-faint">
                    {Math.round(m.workDip[0])}×{Math.round(m.workDip[1])} @ {m.dpi} DPI
                  </span>
                </div>
              ))}
            </div>

            <button
              type="button"
              className="rounded-lg border border-ink-ghost/30 px-3 py-1.5 text-sm disabled:opacity-50"
              onClick={() => void run(clearIconCache)}
              disabled={busy}
            >
              清空图标缓存
            </button>
          </section>
        )}
      </main>
    </div>
  );
}

// ---------------------------------------------------------------------------
// 栅栏行
// ---------------------------------------------------------------------------

interface FenceRowProps {
  fence: Fence;
  hosts: Fence[];
  renaming: boolean;
  renameValue: string;
  busy: boolean;
  onStartRename: () => void;
  onRenameValue: (v: string) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
  onToggleRoll: () => void;
  onDelete: () => void;
  onOpen: () => void;
  onMakeTab: (hostId: string) => void;
  onSplitTab: () => void;
  onSetActiveTab: (tabId: string | null) => void;
}

function FenceRow({
  fence,
  hosts,
  renaming,
  renameValue,
  busy,
  onStartRename,
  onRenameValue,
  onCommitRename,
  onCancelRename,
  onToggleRoll,
  onDelete,
  onOpen,
  onMakeTab,
  onSplitTab,
  onSetActiveTab,
}: FenceRowProps) {
  const isTab = fence.tabHost !== null;
  const tabCount = fence.tabOrder.length;
  const candidates = hosts.filter((h) => h.id !== fence.id && h.kind !== "inbox");

  return (
    <div className="rounded-xl border border-ink-ghost/20 bg-paper-warm px-4 py-2.5">
      <div className="flex items-center gap-3">
        {renaming ? (
          <input
            autoFocus
            className="w-48 rounded-md border border-bamboo bg-paper px-2 py-1 text-sm outline-none"
            value={renameValue}
            onChange={(e) => onRenameValue(e.target.value)}
            onBlur={onCommitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") onCommitRename();
              if (e.key === "Escape") onCancelRename();
            }}
          />
        ) : (
          <button
            type="button"
            className="text-sm font-medium hover:text-bamboo"
            onClick={onStartRename}
            title="点击重命名"
          >
            {fence.title}
          </button>
        )}

        <span className="rounded-full bg-ink-ghost/15 px-2 py-0.5 text-[11px] text-ink-soft">
          {fence.kind === "inbox" ? "收件箱" : isTab ? "标签页" : "栅栏"}
        </span>
        <span className="text-xs text-ink-faint">{fence.items.length} 项</span>
        {tabCount > 0 && <span className="text-xs text-ink-faint">· {tabCount} 个标签页</span>}
        {fence.rolledUp && <span className="text-xs text-ink-faint">· 已卷起</span>}
        <span className="text-xs text-ink-faint">· {VIEW_LABEL[fence.view.layout]}</span>

        <div className="ml-auto flex items-center gap-1.5">
          {!isTab && (
            <button
              type="button"
              className="rounded-md border border-ink-ghost/30 px-2 py-1 text-xs disabled:opacity-50"
              onClick={onOpen}
              disabled={busy}
            >
              打开窗口
            </button>
          )}
          <button
            type="button"
            className="rounded-md border border-ink-ghost/30 px-2 py-1 text-xs disabled:opacity-50"
            onClick={onToggleRoll}
            disabled={busy}
          >
            {fence.rolledUp ? "展开" : "卷起"}
          </button>
          {isTab ? (
            <button
              type="button"
              className="rounded-md border border-ink-ghost/30 px-2 py-1 text-xs disabled:opacity-50"
              onClick={onSplitTab}
              disabled={busy}
            >
              拆出标签页
            </button>
          ) : (
            candidates.length > 0 && (
              <select
                className="rounded-md border border-ink-ghost/30 bg-paper px-1.5 py-1 text-xs"
                value=""
                onChange={(e) => {
                  if (e.target.value) onMakeTab(e.target.value);
                }}
                disabled={busy}
              >
                <option value="">并入标签页…</option>
                {candidates.map((h) => (
                  <option key={h.id} value={h.id}>
                    {h.title}
                  </option>
                ))}
              </select>
            )
          )}
          {fence.kind !== "inbox" && (
            <button
              type="button"
              className="rounded-md border border-ink-ghost/30 px-2 py-1 text-xs text-red-600 disabled:opacity-50"
              onClick={onDelete}
              disabled={busy}
            >
              删除
            </button>
          )}
        </div>
      </div>

      {/* 标签页切换 */}
      {tabCount > 0 && (
        <div className="mt-2 flex flex-wrap items-center gap-1.5 border-t border-ink-ghost/15 pt-2">
          {fence.tabOrder.map((tabId) => {
            const tab = hosts.find((h) => h.id === tabId);
            const active = fence.activeTab === tabId;
            return (
              <button
                key={tabId}
                type="button"
                className={`rounded-full px-2.5 py-0.5 text-xs ${
                  active ? "bg-bamboo text-white" : "border border-ink-ghost/30 text-ink-soft"
                }`}
                onClick={() => onSetActiveTab(active ? null : tabId)}
              >
                {tab?.title ?? tabId.slice(0, 8)}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 规则面板
// ---------------------------------------------------------------------------

interface RulesPanelProps {
  rules: RuleSet;
  busy: boolean;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
}

function RulesPanel({ rules, busy, onToggle, onDelete }: RulesPanelProps) {
  return (
    <section className="space-y-4">
      <div className="rounded-xl border border-ink-ghost/20 bg-paper-warm px-4 py-3 text-xs text-ink-soft">
        <div>
          未匹配条目：
          <span className="font-medium text-ink">
            {rules.unmatched === "stay" ? "保持原位" : "收进收件箱"}
          </span>
        </div>
        <div className="mt-1">
          规则自动更新：
          <span className="font-medium text-ink">{rules.keepUpdated ? "开" : "关"}</span>
        </div>
      </div>

      {rules.list.length === 0 && (
        <div className="rounded-xl border border-dashed border-ink-ghost/40 p-8 text-center text-sm text-ink-faint">
          还没有规则。回到「栅栏」页用快速模板添加，会自动生成对应规则。
        </div>
      )}

      <div className="space-y-2">
        {rules.list.map((rule) => (
          <div
            key={rule.id}
            className="flex items-center gap-3 rounded-xl border border-ink-ghost/20 bg-paper-warm px-4 py-2.5"
          >
            <input
              type="checkbox"
              className="h-4 w-4 accent-[var(--color-bamboo)]"
              checked={rule.enabled}
              onChange={() => onToggle(rule.id)}
              disabled={busy}
            />
            <div className="flex-1">
              <div className="text-sm font-medium">{rule.name}</div>
              <div className="text-xs text-ink-faint">
                {rule.allOf.length} 个条件 · 优先级 {rule.priorityClass}
                {rule.template ? ` · 模板 ${rule.template}` : ""}
              </div>
            </div>
            <button
              type="button"
              className="rounded-md border border-ink-ghost/30 px-2 py-1 text-xs text-red-600 disabled:opacity-50"
              onClick={() => onDelete(rule.id)}
              disabled={busy}
            >
              删除
            </button>
          </div>
        ))}
      </div>
    </section>
  );
}
