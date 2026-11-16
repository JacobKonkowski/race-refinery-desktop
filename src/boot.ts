/**
 * Startup handoff: apply the persisted theme before React mounts, then fade
 * the HTML splash out after the first paint.
 *
 * The native window stays visible from process start (dark backgroundColor +
 * static splash). No artificial hold and no delayed show().
 */
import { getSettings } from "./shared/api";
import { applyTheme, DEFAULT_THEME_ID } from "./shared/themes";

/** Load settings and paint CSS vars before the shell mounts (avoids theme hop). */
export async function prepareInitialTheme(): Promise<void> {
  try {
    const settings = await getSettings();
    applyTheme(settings.themeId || DEFAULT_THEME_ID);
  } catch {
    applyTheme(DEFAULT_THEME_ID);
  }
}

/** Fade out and remove `#boot-splash` after the first React paint. */
export function dismissBootSplash(): void {
  const splash = document.getElementById("boot-splash");
  if (!splash || splash.classList.contains("boot-splash--exit")) return;

  splash.classList.add("boot-splash--exit");

  let removed = false;
  const remove = () => {
    if (removed) return;
    removed = true;
    splash.remove();
  };

  splash.addEventListener("transitionend", remove, { once: true });
  window.setTimeout(remove, 500);
}
