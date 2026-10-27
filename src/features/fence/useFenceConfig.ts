import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getConfig, getIcon } from "./api";
import type { Config } from "./types";

/**
 * 加载栅栏配置，并在 `fence-changed` 事件后自动刷新。
 *
 * 每个栅栏窗口都独立持有这份状态；主进程任何一次 `save()` 都会广播事件，
 * 因此窗口之间能保持一致。
 */
export function useFenceConfig() {
  const [config, setConfig] = useState<Config | null>(null);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);

  const reload = useCallback(async () => {
    try {
      const next = await getConfig();
      if (mounted.current) {
        setConfig(next);
        setError(null);
      }
    } catch (err) {
      if (mounted.current) setError(String(err));
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void reload();
    const unlisten = listen("fence-changed", () => {
      void reload();
    });
    return () => {
      mounted.current = false;
      void unlisten.then((fn) => fn());
    };
  }, [reload]);

  return { config, error, reload };
}

/**
 * 批量加载条目图标。
 *
 * 返回 `itemId -> dataUrl` 映射；图标由后端提取并缓存到磁盘，
 * 这里只做一次内存缓存，避免重复 IPC。
 */
export function useIcons(itemIds: string[], sizePx: number) {
  const [icons, setIcons] = useState<Record<string, string>>({});
  const cache = useRef<Map<string, string>>(new Map());
  const pending = useRef<Set<string>>(new Set());

  useEffect(() => {
    let cancelled = false;
    const missing = itemIds.filter((id) => !cache.current.has(id) && !pending.current.has(id));
    if (missing.length === 0) {
      // 把已有缓存同步到 state
      const next: Record<string, string> = {};
      for (const id of itemIds) {
        const v = cache.current.get(id);
        if (v) next[id] = v;
      }
      setIcons(next);
      return;
    }

    void (async () => {
      for (const id of missing) {
        if (cancelled) return;
        pending.current.add(id);
        try {
          const url = await getIcon(id, sizePx);
          if (url) cache.current.set(id, url);
        } catch {
          // 图标缺失不影响主流程
        } finally {
          pending.current.delete(id);
        }
      }
      if (cancelled) return;
      const next: Record<string, string> = {};
      for (const id of itemIds) {
        const v = cache.current.get(id);
        if (v) next[id] = v;
      }
      setIcons(next);
    })();

    return () => {
      cancelled = true;
    };
  }, [itemIds.join(","), sizePx]);

  return icons;
}
