/**
 * Feature registry — the seam that makes the app expandable.
 *
 * The shell renders whatever features are listed here; it knows nothing about
 * laps or any specific feature. Adding a future surface (Settings, …) is a
 * single append here plus a folder under `features/`. When more than one feature
 * is registered the nav bar appears automatically.
 */
import type { ComponentType, ReactNode } from "react";
import { analyzeFeature } from "./analyze";
import { liveFeature } from "./live";
import { settingsFeature } from "./settings";
import { voiceStudioFeature } from "./voice-studio";

export interface Feature {
  id: string;
  label: string;
  path: string;
  /** The feature's page, rendered into the shell outlet. */
  element: ReactNode;
  /** Optional actions rendered into the shell header's action slot. */
  HeaderActions?: ComponentType;
}

export const features: Feature[] = [
  analyzeFeature,
  liveFeature,
  voiceStudioFeature,
  settingsFeature,
];
