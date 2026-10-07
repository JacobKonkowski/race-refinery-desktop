import { useEffect, useMemo, useState } from "react";
import type { Feature } from "../features/registry";
import { getImportStatus, onImportStatus } from "../shared/api";
import { onNavigateToFeature } from "../shared/navigation";
import { ToastHost } from "../shared/ToastHost";
import type { ImportStatus } from "../shared/types";
import { FeatureNav } from "./FeatureNav";

interface Props {
  features: Feature[];
}

/**
 * Feature-agnostic application shell: brand, a header slot for the active
 * feature's actions, a global import-status indicator, optional nav, and the
 * feature outlet. It has no knowledge of laps or any specific feature.
 */
export function AppShell({ features }: Props) {
  const [activeId, setActiveId] = useState(features[0]?.id ?? "");
  const active = useMemo(
    () => features.find((f) => f.id === activeId) ?? features[0],
    [features, activeId],
  );

  useEffect(() => onNavigateToFeature(setActiveId), []);

  if (!active) return null;
  const HeaderActions = active.HeaderActions;

  return (
    <div className="app-shell">
      <header className="app-header" role="banner">
        <div className="app-brand">
          <span className="brand-mark">Race Refinery</span>
          <span className="brand-sub">race telemetry</span>
        </div>
        <div className="app-header-actions" aria-label="Feature actions">
          {HeaderActions ? <HeaderActions /> : null}
        </div>
        <GlobalStatus />
      </header>

      <FeatureNav features={features} activeId={active.id} onSelect={setActiveId} />

      <main className="app-outlet" id="main-content" tabIndex={-1} aria-label={active.label}>
        {active.element}
      </main>
      <ToastHost />
    </div>
  );
}

/** Import progress lives in the shell so future features can share it. */
function GlobalStatus() {
  const [status, setStatus] = useState<ImportStatus | null>(null);

  useEffect(() => {
    getImportStatus().then(setStatus).catch(() => undefined);
    const unlisten = onImportStatus(setStatus);
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  if (!status || (!status.active && !status.message)) return null;

  return (
    <div className="app-status" title={status.currentFile ?? undefined}>
      <span>{status.message}</span>
      {status.active ? (
        <div className="status-bar">
          <span style={{ width: `${Math.min(100, Math.max(0, status.progressPct))}%` }} />
        </div>
      ) : null}
    </div>
  );
}
