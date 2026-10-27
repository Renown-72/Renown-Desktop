export type AppView = "main" | "notepad" | "tile" | "fence" | "fence-manager";

export interface AppRoute {
  view: AppView;
  noteId?: string;
  fenceId?: string;
}

export function getInitialRoute(url: URL = new URL(window.location.href)): AppRoute {
  return routeFromSearch(url.search);
}

export function routeFromSearch(search: string): AppRoute {
  const params = new URLSearchParams(search);
  const view = params.get("view");
  const noteId = params.get("noteId") ?? undefined;
  const fenceId = params.get("fenceId") ?? undefined;

  if (view === "notepad") return noteId ? { view, noteId } : { view };
  if (view === "tile") return noteId ? { view, noteId } : { view };
  if (view === "fence") return fenceId ? { view, fenceId } : { view: "main" };
  if (view === "fence-manager") return { view };
  return { view: "main" };
}

export function buildNotepadUrl(noteId?: string): string {
  return buildUrl("notepad", noteId);
}

export function buildTileUrl(noteId: string): string {
  return buildUrl("tile", noteId);
}

export function buildFenceUrl(fenceId: string): string {
  const params = new URLSearchParams({ view: "fence", fenceId });
  return `index.html?${params.toString()}`;
}

export function buildFenceManagerUrl(): string {
  const params = new URLSearchParams({ view: "fence-manager" });
  return `index.html?${params.toString()}`;
}

function buildUrl(view: AppView, noteId?: string): string {
  const params = new URLSearchParams({ view });
  if (noteId) params.set("noteId", noteId);
  return `index.html?${params.toString()}`;
}
