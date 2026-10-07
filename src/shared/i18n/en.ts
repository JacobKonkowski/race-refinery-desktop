/** English UI strings — see docs/I18N.md */
export const en = {
  "app.name": "Race Refinery",
  "nav.analyze": "Analyze",
  "nav.live": "Live",
  "live.startMonitor": "Start live monitor",
  "live.stopMonitor": "Stop live monitor",
  "live.startAudio": "Start audio coach",
  "live.testCoach": "Test Coach",
  "live.startMonitorWidgets": "Start monitor widgets",
  "live.stopMonitorWidgets": "Stop monitor widgets",
  "live.startVrHud": "Start VR HUD",
  "analyze.import": "Import IBT",
  "analyze.insights": "Insights",
} as const;

export type MessageKey = keyof typeof en;
