import type { MvLayout } from "../../store/multiViewStore";

/** A tile rectangle in container (CSS px, top-left origin) coordinates. */
export interface LayoutRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Gap between tiles (CSS px); the compositor backdrop shows through it. */
export const GAP = 6;

/**
 * Compute each tile's rectangle for `count` tiles in the given layout.
 * Returned by index (tiles[i] → rects[i]). Even grid: 1=full, 2=side-by-side,
 * 3/4=2×2. Focus: the `focusIndex` tile fills the top, the rest sit in a strip
 * along the bottom.
 */
export function computeLayout(
  count: number,
  layout: MvLayout,
  focusIndex: number,
  W: number,
  H: number,
): LayoutRect[] {
  if (count <= 0 || W <= 0 || H <= 0) return [];
  if (count === 1) return [{ x: 0, y: 0, w: W, h: H }];

  if (layout === "focus") {
    const stripH = Math.max(96, Math.round(H * 0.22));
    const mainH = H - stripH - GAP;
    const main: LayoutRect = { x: 0, y: 0, w: W, h: mainH };
    const others = count - 1;
    const cw = (W - GAP * (others - 1)) / others;
    const rects: LayoutRect[] = [];
    let oi = 0;
    for (let i = 0; i < count; i++) {
      if (i === focusIndex) {
        rects.push(main);
      } else {
        rects.push({
          x: Math.round(oi * (cw + GAP)),
          y: H - stripH,
          w: Math.round(cw),
          h: stripH,
        });
        oi++;
      }
    }
    return rects;
  }

  // Even grid.
  if (count === 2) {
    const cw = (W - GAP) / 2;
    return [
      { x: 0, y: 0, w: Math.round(cw), h: H },
      { x: Math.round(cw + GAP), y: 0, w: Math.round(cw), h: H },
    ];
  }
  // 3 or 4 → 2×2.
  const cw = (W - GAP) / 2;
  const ch = (H - GAP) / 2;
  const cellAt = (col: number, row: number): LayoutRect => ({
    x: Math.round(col * (cw + GAP)),
    y: Math.round(row * (ch + GAP)),
    w: Math.round(cw),
    h: Math.round(ch),
  });
  const slots = [cellAt(0, 0), cellAt(1, 0), cellAt(0, 1), cellAt(1, 1)];
  return slots.slice(0, count);
}

/**
 * The empty 2×2 slot used for the inline "+ Add" cell when there are exactly
 * three tiles in the even grid (the 4th cell). Returns null otherwise.
 */
export function addSlot(
  count: number,
  layout: MvLayout,
  W: number,
  H: number,
): LayoutRect | null {
  if (layout !== "grid" || count !== 3 || W <= 0 || H <= 0) return null;
  return computeLayout(4, "grid", 0, W, H)[3];
}
