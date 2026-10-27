/**
 * 拖拽排序的纯逻辑。
 *
 * 单独抽出来是因为「插入位置」的边界条件最容易写错（拖到自己前面/后面、
 * 拖到首尾、目标不存在），而这些都能用纯函数覆盖，不需要跑界面。
 */

export type DropEdge = "before" | "after";

/**
 * 计算把 `fromId` 移动到 `toId` 的 `edge` 一侧之后的新顺序。
 *
 * @param order  当前完整顺序（条目 id）
 * @param fromId 被拖动的条目
 * @param toId   拖放目标条目
 * @param edge   放到目标的哪一侧
 * @returns 新顺序；输入非法（id 不存在、自己拖自己）时返回 `null`
 */
export function computeReorder(
  order: readonly string[],
  fromId: string,
  toId: string,
  edge: DropEdge,
): string[] | null {
  if (fromId === toId) return null;
  if (!order.includes(fromId) || !order.includes(toId)) return null;

  // 先移除被拖动项，再按剩余列表计算插入点
  const rest = order.filter((id) => id !== fromId);
  let insertAt = rest.indexOf(toId);
  if (insertAt < 0) return null;
  if (edge === "after") insertAt += 1;
  rest.splice(insertAt, 0, fromId);

  // 顺序没变就没有必要回写
  if (rest.length === order.length && rest.every((id, i) => id === order[i])) {
    return null;
  }
  return rest;
}
