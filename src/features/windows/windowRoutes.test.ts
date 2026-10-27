import { describe, expect, it } from "vitest";
import {
  buildFenceManagerUrl,
  buildFenceUrl,
  buildNotepadUrl,
  buildTileUrl,
  getInitialRoute,
  routeFromSearch,
} from "./windowRoutes";

describe("window routes", () => {
  it("parses supported routes and note ids", () => {
    expect(routeFromSearch("?view=notepad&noteId=abc-123")).toEqual({
      view: "notepad",
      noteId: "abc-123",
    });
    expect(routeFromSearch("?view=tile&noteId=note-1")).toEqual({
      view: "tile",
      noteId: "note-1",
    });
    expect(routeFromSearch("?view=unknown")).toEqual({ view: "main" });
  });

  it("builds app urls for dynamic windows", () => {
    expect(buildNotepadUrl()).toBe("index.html?view=notepad");
    expect(buildNotepadUrl("abc 123")).toBe("index.html?view=notepad&noteId=abc+123");
    expect(buildTileUrl("note-1")).toBe("index.html?view=tile&noteId=note-1");
  });

  it("reads the browser location by default", () => {
    expect(getInitialRoute(new URL("https://floral-notepaper.test/?view=main"))).toEqual({
      view: "main",
    });
  });

  describe("fence routes", () => {
    it("parses a fence window route with its fence id", () => {
      expect(routeFromSearch("?view=fence&fenceId=f-1")).toEqual({
        view: "fence",
        fenceId: "f-1",
      });
    });

    it("falls back to the main window when the fence id is missing", () => {
      expect(routeFromSearch("?view=fence")).toEqual({ view: "main" });
    });

    it("parses the fence manager route", () => {
      expect(routeFromSearch("?view=fence-manager")).toEqual({ view: "fence-manager" });
    });

    it("builds fence urls", () => {
      expect(buildFenceUrl("f-1")).toBe("index.html?view=fence&fenceId=f-1");
      expect(buildFenceManagerUrl()).toBe("index.html?view=fence-manager");
    });

    it("round-trips a built fence url back through the parser", () => {
      const url = new URL(`https://app.test/${buildFenceUrl("abc 123")}`);
      expect(getInitialRoute(url)).toEqual({ view: "fence", fenceId: "abc 123" });
    });
  });
});
