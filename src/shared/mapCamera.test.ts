import { describe, expect, it } from "vitest";
import {
  FIT_CAMERA,
  FIT_SCALE,
  FOCUS_SCALE,
  MAX_SCALE,
  MIN_SCALE,
  clampCamera,
  clientToMap,
  focusOn,
  panByPixels,
  viewBoxFromCamera,
  zoomAt,
} from "./mapCamera";

/** 400x200 element: the square viewBox is drawn 200 px wide, centred. */
const rect = { left: 0, top: 0, width: 400, height: 200 };

describe("mapCamera", () => {
  it("fit camera pads around the unit box", () => {
    const size = 1 / FIT_SCALE;
    expect(viewBoxFromCamera(FIT_CAMERA)).toBe(
      `${0.5 - size / 2} ${0.5 - size / 2} ${size} ${size}`,
    );
  });

  it("maps screen points through the letterboxed square", () => {
    const edge = { scale: 1, cx: 0.5, cy: 0.5 };
    expect(clientToMap(edge, rect, 100, 0)).toEqual({ x: 0, y: 0 });
    expect(clientToMap(edge, rect, 200, 100)).toEqual({ x: 0.5, y: 0.5 });
    expect(clientToMap(edge, rect, 300, 200)).toEqual({ x: 1, y: 1 });
  });

  it("zooming keeps the anchor under the cursor", () => {
    const anchor = { x: 0.3, y: 0.6 };
    const start = { scale: 1, cx: 0.5, cy: 0.5 };
    const zoomed = zoomAt(start, anchor, 2);
    expect(zoomed.scale).toBe(2);
    const screen = { x: 100 + 0.3 * 200, y: 0.6 * 200 };
    const after = clientToMap(zoomed, rect, screen.x, screen.y);
    expect(after.x).toBeCloseTo(anchor.x);
    expect(after.y).toBeCloseTo(anchor.y);
  });

  it("clamps zoom range", () => {
    expect(zoomAt(FIT_CAMERA, { x: 0.5, y: 0.5 }, 0.25).scale).toBe(MIN_SCALE);
    expect(zoomAt({ ...FIT_CAMERA, scale: 5 }, { x: 0.5, y: 0.5 }, 4).scale).toBe(MAX_SCALE);
  });

  it("allows zooming out past the unit box", () => {
    expect(MIN_SCALE).toBeLessThan(FIT_SCALE);
    expect(zoomAt(FIT_CAMERA, { x: 0.5, y: 0.5 }, 0.5).scale).toBeLessThan(FIT_SCALE);
  });

  it("pans opposite to the drag and stays near the track", () => {
    const cam = { scale: 2, cx: 0.5, cy: 0.5 };
    // Dragging right by 100 px moves a 200 px / 0.5-unit view left by 0.25.
    expect(panByPixels(cam, rect, 100, 0).cx).toBeCloseTo(0.25);
    const far = panByPixels(cam, rect, 10_000, -10_000);
    expect(far.cx).toBeCloseTo(0.125);
    expect(far.cy).toBeCloseTo(0.875);
  });

  it("clamp keeps the centre within a quarter view of the box", () => {
    expect(clampCamera({ scale: 1, cx: -1, cy: 2 })).toEqual({ scale: 1, cx: 0.25, cy: 0.75 });
  });

  it("clamp pins the centre when zoomed out past the unit box", () => {
    expect(clampCamera({ scale: MIN_SCALE, cx: 0, cy: 1 })).toEqual({
      scale: MIN_SCALE,
      cx: 0.5,
      cy: 0.5,
    });
  });

  it("focus zooms in but never out", () => {
    expect(focusOn(FIT_CAMERA, { x: 0.4, y: 0.6 })).toEqual({
      scale: FOCUS_SCALE,
      cx: 0.4,
      cy: 0.6,
    });
    expect(focusOn({ scale: 5, cx: 0.5, cy: 0.5 }, { x: 0.4, y: 0.6 }).scale).toBe(5);
  });
});
