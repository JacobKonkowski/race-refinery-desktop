import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  checkIracingConfig,
  getSession,
  loadLaps,
  reimportSession,
} from "../../shared/api";
import {
  getAnalyzeOpen,
  setAnalyzeOpen,
  subscribeAnalyzeOpen,
  type AnalyzeOpen,
} from "../../shared/analyzeOpen";
import { formatDate } from "../../shared/format";
import { navigateToFeature } from "../../shared/navigation";
import { showToast } from "../../shared/toast";
import { useTrackMap } from "../../shared/useTrackMap";
import type {
  IracingConfigCheck,
  LapSummary,
  SessionDetail,
  SetLap,
  TrackOutline,
} from "../../shared/types";
import { ComparePanel } from "./ComparePanel";
import { ConfigBanner } from "./ConfigBanner";
import { FuelTirePanel } from "./FuelTirePanel";
import { InsightsStrip } from "./InsightsStrip";
import { LapTable } from "./LapTable";
import { SessionHeader } from "./SessionHeader";
import { TrackMapPanel } from "./TrackMapPanel";
import { computeSessionStats, type SessionStats } from "./sessionStats";

/** Fastest pace-eligible lap in the session (the default compare reference). */
function defaultReferenceLap(laps: LapSummary[]): LapSummary | null {
  return laps
    .filter((l) => l.paceEligible && l.lapTimeMs != null)
    .reduce<LapSummary | null>((best, l) => {
      if (!best || (l.lapTimeMs ?? Infinity) < (best.lapTimeMs ?? Infinity)) return l;
      return best;
    }, null);
}

function setGroupLabel(rows: SetLap[]): (lap: LapSummary) => {
  key: string;
  label: string;
  date: string;
  car: string;
  sessionType: string;
} {
  const byId = new Map(rows.map((row) => [row.lap.id, row]));
  return (lap) => {
    const row = byId.get(lap.id);
    return {
      key: `${row?.sessionId ?? 0}:${lap.sessionNum}`,
      label: [row ? formatDate(row.sessionDate) : "", row?.car ?? "", lap.sessionType]
        .filter(Boolean)
        .join(" · "),
      date: row ? formatDate(row.sessionDate) : "",
      car: row?.car ?? "",
      sessionType: lap.sessionType,
    };
  };
}

export function AnalyzePage() {
  const [open, setOpen] = useState<AnalyzeOpen>(() => getAnalyzeOpen());
  const [selectionRows, setSelectionRows] = useState<SetLap[]>([]);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const [detail, setSession] = useState<SessionDetail | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [candidateLapId, setCandidateLapId] = useState<number | null>(null);
  const [referenceLapId, setReferenceLapId] = useState<number | null>(null);
  const [config, setConfig] = useState<IracingConfigCheck | null>(null);
  /** Lap fraction hovered on the compare charts, mirrored on the track map. */
  const [highlightPct, setHighlightPct] = useState<number | null>(null);
  /** Corner-table / chart focus: zoom the map there. `seq` re-triggers the same pct. */
  const [mapFocus, setMapFocus] = useState<{ pct: number; seq: number } | null>(null);
  const focusMapAt = useCallback(
    (pct: number) => setMapFocus((prev) => ({ pct, seq: (prev?.seq ?? 0) + 1 })),
    [],
  );
  const [reimporting, setReimporting] = useState(false);
  const [paceOnly, setPaceOnly] = useState(false);
  const [moreLapColumns, setMoreLapColumns] = useState(false);
  /** Lap-id key already loaded into `selectionRows`, so a save does not fetch twice. */
  const loadedKey = useRef("");
  const [settledKey, setSettledKey] = useState("");
  const pendingCandidate = useRef<number | null>(null);

  useEffect(() => subscribeAnalyzeOpen(() => setOpen(getAnalyzeOpen())), []);

  useEffect(() => {
    checkIracingConfig().then(setConfig).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (open.kind !== "session") {
      setSession(null);
      setLoadingDetail(false);
      return;
    }
    const sessionId = open.sessionId;
    pendingCandidate.current = open.candidateLapId;
    let cancelled = false;
    setLoadingDetail(true);
    getSession(sessionId)
      .then((d) => {
        if (cancelled) return;
        setSession(d);
        const ref = d ? defaultReferenceLap(d.laps) : null;
        setReferenceLapId(ref?.id ?? null);
        const wanted = pendingCandidate.current;
        setCandidateLapId(
          wanted != null && d?.laps.some((lap) => lap.id === wanted) ? wanted : null,
        );
        setMapFocus(null);
      })
      .catch((e) => {
        console.error("getSession failed", e);
        showToast(`Failed to load session: ${String(e)}`, "error");
        setSession(null);
      })
      .finally(() => !cancelled && setLoadingDetail(false));
    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    if (open.kind !== "session" || detail == null || detail.session.id !== open.sessionId) return;
    setReferenceLapId((prev) =>
      prev != null && detail.laps.some((lap) => lap.id === prev)
        ? prev
        : (defaultReferenceLap(detail.laps)?.id ?? null),
    );
    setCandidateLapId((prev) =>
      prev != null && detail.laps.some((lap) => lap.id === prev) ? prev : null,
    );
  }, [open, detail]);

  const selectionKey = open.kind === "selection" ? open.lapIds.join(",") : "";

  useEffect(() => {
    if (open.kind !== "selection") return;
    if (selectionKey === "") {
      loadedKey.current = "";
      setSettledKey("");
      setSelectionRows([]);
      setSelectionError(null);
      return;
    }
    if (loadedKey.current === selectionKey) return;
    const ids = selectionKey.split(",").map(Number);
    let cancelled = false;
    setSelectionError(null);
    loadLaps(ids)
      .then((rows) => {
        if (cancelled) return;
        loadedKey.current = selectionKey;
        setSettledKey(selectionKey);
        setSelectionRows(rows);
        const nextLaps = rows.map((row) => row.lap);
        setReferenceLapId((prev) =>
          prev != null && nextLaps.some((lap) => lap.id === prev)
            ? prev
            : (defaultReferenceLap(nextLaps)?.id ?? null),
        );
        setCandidateLapId((prev) =>
          prev != null && nextLaps.some((lap) => lap.id === prev) ? prev : null,
        );
        setMapFocus(null);
      })
      .catch((e) => {
        if (cancelled) return;
        console.error("loadLaps failed", e);
        loadedKey.current = "";
        setSettledKey(selectionKey);
        setSelectionRows([]);
        setSelectionError(String(e));
        showToast(`Failed to load laps: ${String(e)}`, "error");
      });
    return () => {
      cancelled = true;
    };
  }, [open.kind, selectionKey]);

  const handleReimport = useCallback(
    async (sessionId: number) => {
      setReimporting(true);
      try {
        const newId = await reimportSession(sessionId);
        setAnalyzeOpen({ kind: "session", sessionId: newId, candidateLapId: null });
        showToast("Session re-imported with the latest analysis.", "success");
      } catch (e) {
        showToast(String(e), "error");
      } finally {
        setReimporting(false);
      }
    },
    [],
  );

  const laps = detail?.laps ?? [];
  const stats = useMemo(() => computeSessionStats(laps), [laps]);
  const sessionTypes = useMemo(
    () => [...new Set(laps.map((l) => l.sessionType).filter(Boolean))],
    [laps],
  );
  const selection = open.kind === "selection" ? open : null;
  const selectionLaps = selectionRows.map((row) => row.lap);
  const selectionReady =
    selection != null && settledKey === selection.lapIds.join(",") && selectionError == null;
  const visibleLaps = selection && selectionReady ? selectionLaps : laps;
  const mapTrack = selection && selection.lapIds.length > 0 ? selection.track : detail?.session.track;
  const trackMap = useTrackMap(mapTrack);
  const hasEligible = useMemo(() => laps.some((l) => l.paceEligible), [laps]);
  const okChannelPresent = useMemo(() => laps.some((l) => l.deltaBestOk !== null), [laps]);
  const candidate = visibleLaps.find((l) => l.id === candidateLapId) ?? null;
  const reference = visibleLaps.find((l) => l.id === referenceLapId) ?? null;

  return (
    <div className="analyze analyze-solo">
      <div className="analyze-workspace">
        {open.kind === "empty" ? (
          <EmptyAnalyze config={config} />
        ) : selection ? (
          selection.lapIds.length === 0 ? (
            <EmptyAnalyze config={config} />
          ) : !selectionReady && selectionError ? (
            <div className="loading">{selectionError}</div>
          ) : !selectionReady ? (
            <div className="loading">Loading laps…</div>
          ) : (
            <CompareWorkspace
              header={
                <div className="session-header">
                  <div>
                    <h2>{selection.name.trim() || "Selection"}</h2>
                    <div className="muted">
                      {selection.track} · {selection.lapIds.length} lap
                      {selection.lapIds.length === 1 ? "" : "s"}
                    </div>
                  </div>
                  <ChangeLibraryLink />
                </div>
              }
              laps={selectionLaps}
              track={selection.track}
              trackMap={trackMap}
              candidate={candidate}
              reference={reference}
              candidateLapId={candidateLapId}
              referenceLapId={referenceLapId}
              onSelectCandidate={setCandidateLapId}
              onSelectReference={setReferenceLapId}
              onHoverDistPct={setHighlightPct}
              onFocusDistPct={focusMapAt}
              highlightPct={highlightPct}
              mapFocus={mapFocus}
              paceOnly={paceOnly}
              onPaceOnly={setPaceOnly}
              moreLapColumns={moreLapColumns}
              onMoreLapColumns={() => setMoreLapColumns((v) => !v)}
              showInsights={false}
              showFuel={false}
              sameSubsession={false}
              deltaHeading="Δ Set"
              groupLabel={setGroupLabel(selectionRows)}
            />
          )
        ) : loadingDetail ? (
          <div className="loading">Loading session…</div>
        ) : !detail ? (
          <div className="loading">
            Session not found.{" "}
            <button type="button" className="btn btn-ghost" onClick={() => navigateToFeature("library")}>
              Open Lap Library
            </button>
          </div>
        ) : (
          <CompareWorkspace
            header={
              <SessionHeader
                session={detail.session}
                stats={stats}
                sessionTypes={sessionTypes}
                onReimport={() => handleReimport(detail.session.id)}
                reimporting={reimporting}
                trailing={<ChangeLibraryLink />}
              />
            }
            notice={
              !hasEligible && laps.length > 0 && !okChannelPresent ? <ConfigBanner /> : null
            }
            laps={laps}
            track={detail.session.track}
            trackMap={trackMap}
            candidate={candidate}
            reference={reference}
            candidateLapId={candidateLapId}
            referenceLapId={referenceLapId}
            onSelectCandidate={setCandidateLapId}
            onSelectReference={setReferenceLapId}
            onHoverDistPct={setHighlightPct}
            onFocusDistPct={focusMapAt}
            highlightPct={highlightPct}
            mapFocus={mapFocus}
            paceOnly={paceOnly}
            onPaceOnly={setPaceOnly}
            moreLapColumns={moreLapColumns}
            onMoreLapColumns={() => setMoreLapColumns((v) => !v)}
            showInsights
            showFuel
            stats={stats}
          />
        )}
      </div>
    </div>
  );
}

function ChangeLibraryLink() {
  return (
    <button
      type="button"
      className="btn btn-ghost change-library-link"
      onClick={() => navigateToFeature("library")}
    >
      Change in Lap Library
    </button>
  );
}

function CompareWorkspace({
  header,
  notice,
  laps,
  track,
  trackMap,
  candidate,
  reference,
  candidateLapId,
  referenceLapId,
  onSelectCandidate,
  onSelectReference,
  onHoverDistPct,
  onFocusDistPct,
  highlightPct,
  mapFocus,
  paceOnly,
  onPaceOnly,
  moreLapColumns,
  onMoreLapColumns,
  showInsights,
  showFuel,
  stats,
  sameSubsession = true,
  deltaHeading,
  groupLabel,
}: {
  header: ReactNode;
  notice?: ReactNode;
  laps: LapSummary[];
  track: string;
  trackMap: TrackOutline | null;
  candidate: LapSummary | null;
  reference: LapSummary | null;
  candidateLapId: number | null;
  referenceLapId: number | null;
  onSelectCandidate: (id: number) => void;
  onSelectReference: (id: number) => void;
  onHoverDistPct: (pct: number | null) => void;
  onFocusDistPct: (pct: number) => void;
  highlightPct: number | null;
  mapFocus: { pct: number; seq: number } | null;
  paceOnly: boolean;
  onPaceOnly: (value: boolean) => void;
  moreLapColumns: boolean;
  onMoreLapColumns: () => void;
  showInsights: boolean;
  showFuel: boolean;
  stats?: SessionStats;
  sameSubsession?: boolean;
  deltaHeading?: string;
  groupLabel?: (lap: LapSummary) => {
    key: string;
    label: string;
    date: string;
    car: string;
    sessionType: string;
  };
}) {
  return (
    <>
      <div className="analyze-session-bar panel">{header}</div>
      {showInsights && stats ? <InsightsStrip stats={stats} /> : null}
      {notice}
      <div className="analyze-main">
        <ComparePanel
          laps={laps}
          candidate={candidate}
          reference={reference}
          onChangeReference={onSelectReference}
          onHoverDistPct={onHoverDistPct}
          onFocusDistPct={onFocusDistPct}
          sameSubsession={sameSubsession}
        />
        <div className="analyze-map-cell">
          <TrackMapPanel
            outline={trackMap}
            track={track}
            candidate={candidate}
            reference={reference}
            highlightPct={highlightPct}
            focus={mapFocus}
          />
        </div>
      </div>
      <div className="panel analyze-lap-picker">
        <div className="panel-header">
          <h2>Laps</h2>
          <div className="lap-picker-tools">
            <label className="lap-filter">
              <input
                type="checkbox"
                checked={paceOnly}
                onChange={(e) => onPaceOnly(e.target.checked)}
              />
              Pace only
            </label>
            <button
              type="button"
              className={`btn btn-ghost${moreLapColumns ? " on" : ""}`}
              onClick={onMoreLapColumns}
            >
              {moreLapColumns ? "Fewer columns" : "More columns"}
            </button>
          </div>
        </div>
        <div className="lap-select-help muted">
          <div className="lap-legend">
            <span>
              <span className="swatch cand" /> Click a lap to compare
            </span>
            <span>
              <span className="swatch ref" /> Right-click (or Shift-click) to set reference
            </span>
          </div>
          {candidateLapId != null && (referenceLapId == null || referenceLapId === candidateLapId) ? (
            <p className="lap-ref-hint">Right-click another lap as reference.</p>
          ) : null}
        </div>
        <div className="panel-body" style={{ padding: 0 }}>
          <LapTable
            laps={laps}
            candidateLapId={candidateLapId}
            referenceLapId={referenceLapId}
            onSelectCandidate={onSelectCandidate}
            onSelectReference={onSelectReference}
            paceOnly={paceOnly}
            showTireTemps={moreLapColumns}
            deltaHeading={deltaHeading}
            groupLabel={groupLabel}
          />
        </div>
      </div>
      {showFuel ? (
        <details className="analyze-secondary panel">
          <summary>Fuel &amp; tires</summary>
          <div className="panel-body">
            <FuelTirePanel laps={laps} />
          </div>
        </details>
      ) : null}
    </>
  );
}

function EmptyAnalyze({ config }: { config: IracingConfigCheck | null }) {
  return (
    <div className="empty-state">
      <h2>Nothing open yet</h2>
      <p className="muted">
        Open a session or lap from Lap Library, or build a selection / set and choose{" "}
        <strong>Open in Analyze</strong>.
      </p>
      {config && !config.diskEnabled ? (
        <p className="muted">
          Tip: set <code>irsdkEnableDisk=1</code> in{" "}
          <code>Documents\iRacing\app.ini</code> to record IBT files.
        </p>
      ) : null}
      <div className="empty-actions">
        <button
          className="btn btn-primary"
          onClick={() => navigateToFeature("library")}
        >
          Open Lap Library
        </button>
      </div>
    </div>
  );
}
