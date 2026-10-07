import type { Feature } from "../registry";
import { SettingsPage } from "./SettingsPage";

export const settingsFeature: Feature = {
  id: "settings",
  label: "Settings",
  path: "/settings",
  element: <SettingsPage />,
};
