/** Zoom / pan camera over the track map's `0 0 1 1` unit box. */
import type { MapPoint } from "./trackMap";

export interface MapCamera {
  /** 1 = whole track fits; larger zooms in. */
  scale: number;
  /** View centre in unit-box coordinates. */
  cx: number;
  cy: number;
}

export const MIN_SCALE = 1;
export const MAX_SCALE = 6;
/** Zoom level used when jumping to a corner. */
export const FOCUS_SCALE = 3;
export const FIT_CAMERA: MapCamera = { scale: 1, cx: 0.5, cy: 0.5 };

/** Screen rectangle of the `<svg>` element (a `DOMRect` fits). */
export interface ScreenRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function isFit(c: MapCamera): boolean {
  return c.scale === FIT_CAMERA.scale && c.cx === FIT_CAMERA.cx && c.cy === FIT_CAMERA.cy;
}

/**
 * Clamp zoom to [`MIN_SCALE`, `MAX_SCALE`] and keep the view centre within
 * half a view of the unit box, so the track can't be dragged off-screen.
 */
export function clampCamera(c: MapCamera): MapCamera {
  const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, c.scale));
  const quarter = 0.25 / scale;
  const clamp = (v: number) => Math.min(1 - quarter, Math.max(quarter, v));
  return { scale, cx: clamp(c.cx), cy: clamp(c.cy) };
}

export function viewBoxFromCamera(c: MapCamera): string {
  const size = 1 / c.scale;
  return `${c.cx - size / 2} ${c.cy - size / 2} ${size} ${size}`;
}

/**
 * Size (px) of the square the viewBox is drawn into. The SVG uses
 * `preserveAspectRatio="xMidYMid meet"`, so a square viewBox fits the
 * element's shorter side and is centred along the longer one.
 */
function drawnSide(rect: ScreenRect): number {
  return Math.min(rect.width, rect.height);
}

/** Map a screen point to unit-box coordinates under the current camera. */
export function clientToMap(
  c: MapCamera,
  rect: ScreenRect,
  clientX: number,
  clientY: number,
): MapPoint {
  const side = drawnSide(rect);
  const size = 1 / c.scale;
  const u = (clientX - rect.left - (rect.width - side) / 2) / side;
  const v = (clientY - rect.top - (rect.height - side) / 2) / side;
  return { x: c.cx - size / 2 + u * size, y: c.cy - size / 2 + v * size };
}

/** Zoom by `factor` while keeping `anchor` (unit-box point) under the cursor. */
export function zoomAt(c: MapCamera, anchor: MapPoint, factor: number): MapCamera {
  const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, c.scale * factor));
  const keep = c.scale / scale;
  return clampCamera({
    scale,
    cx: anchor.x + (c.cx - anchor.x) * keep,
    cy: anchor.y + (c.cy - anchor.y) * keep,
  });
}

/** Move the view by a screen-pixel drag delta. */
export function panByPixels(
  c: MapCamera,
  rect: ScreenRect,
  dxPx: number,
  dyPx: number,
): MapCamera {
  const side = drawnSide(rect);
  if (side <= 0) return c;
  const unitsPerPx = 1 / c.scale / side;
  return clampCamera({ ...c, cx: c.cx - dxPx * unitsPerPx, cy: c.cy - dyPx * unitsPerPx });
}

/** Centre on `point`, zooming in to at least `FOCUS_SCALE`. */
export function focusOn(c: MapCamera, point: MapPoint): MapCamera {
  return clampCamera({ scale: Math.max(c.scale, FOCUS_SCALE), cx: point.x, cy: point.y });
}
