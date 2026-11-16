/**
 * Startup handoff: reveal the main window on the HTML splash, apply the
 * persisted theme before React mounts, then fade the splash out.
 *
 * No artificial minimum hold — the splash stays only while boot work runs.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getSettings } from "./shared/api";
import { applyTheme, DEFAULT_THEME_ID } from "./shared/themes";

export async function revealMainWindow(): Promise<void> {
  try {
    await getCurrentWindow().show();
  } catch {
    // Browser / non-Tauri preview — ignore.
  }
}

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
