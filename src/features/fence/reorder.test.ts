import { describe, expect, it } from "vitest";
import { computeReorder } from "./reorder";

const base = ["a", "b", "c", "d", "e"];

describe("computeReorder", () => {
  it("把后面的项拖到前面", () => {
    expect(computeReorder(base, "e", "b", "before")).toEqual(["a", "e", "b", "c", "d"]);
  });

  it("把前面的项拖到后面", () => {
    expect(computeReorder(base, "a", "c", "after")).toEqual(["b", "c", "a", "d", "e"]);
  });

  it("拖到第一项之前", () => {
    expect(computeReorder(base, "d", "a", "before")).toEqual(["d", "a", "b", "c", "e"]);
  });

  it("拖到最后一项之后", () => {
    expect(computeReorder(base, "a", "e", "after")).toEqual(["b", "c", "d", "e", "a"]);
  });

  it("向后拖动时 after 只跨一格", () => {
    // a 放到 b 后面，应得到 b a c d e，而不是多跨一格
    expect(computeReorder(base, "a", "b", "after")).toEqual(["b", "a", "c", "d", "e"]);
  });

  it("向前拖动时 before 只跨一格", () => {
    expect(computeReorder(base, "c", "b", "before")).toEqual(["a", "c", "b", "d", "e"]);
  });

  it("自己拖自己返回 null", () => {
    expect(computeReorder(base, "c", "c", "before")).toBeNull();
    expect(computeReorder(base, "c", "c", "after")).toBeNull();
  });

  it("不存在的 id 返回 null", () => {
    expect(computeReorder(base, "x", "b", "before")).toBeNull();
    expect(computeReorder(base, "a", "x", "after")).toBeNull();
  });

  it("结果顺序未变化时返回 null（避免无意义回写）", () => {
    expect(computeReorder(["a", "b"], "a", "b", "before")).toBeNull();
  });

  it("单元素与空列表不会崩", () => {
    expect(computeReorder(["a"], "a", "a", "after")).toBeNull();
    expect(computeReorder([], "a", "b", "before")).toBeNull();
  });
});
