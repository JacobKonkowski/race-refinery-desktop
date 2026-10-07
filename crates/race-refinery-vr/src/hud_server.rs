//! Local HTTP HUD for in-headset use via OpenKneeboard (or any Web Dashboard tab).
//! Works with iRacing OpenXR ΓÇö no SteamVR required.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::VrOverlayService;
use race_refinery_live::LiveService;

pub const HUD_PORT: u16 = 17342;

pub fn hud_url() -> String {
    format!("http://127.0.0.1:{HUD_PORT}/vr")
}

/// Returns true if the HUD HTTP server is accepting connections.
pub fn check_hud_health() -> bool {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr: SocketAddr = format!("127.0.0.1:{HUD_PORT}")
        .parse()
        .unwrap_or_else(|_| "127.0.0.1:17342".parse().unwrap());
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let req = "GET /api/health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 128];
    let Ok(n) = stream.read(&mut buf) else {
        return false;
    };
    String::from_utf8_lossy(&buf[..n]).contains("200 OK")
}

pub fn open_hud_preview() -> Result<(), String> {
    let url = hud_url();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = url;
        Err("HUD preview is supported on Windows only".into())
    }
}

pub fn run_hud_server(
    service: Arc<VrOverlayService>,
    live: Arc<LiveService>,
    cancel: CancellationToken,
    // When true, primary web-mode HUD owns status text; when false, companion preview only.
    own_status: bool,
) -> anyhow::Result<()> {
    let addr = format!("127.0.0.1:{HUD_PORT}");
    let listener = TcpListener::bind(&addr)?;
    if own_status {
        service.status.lock().message = format!("HUD ready at {}", hud_url());
        service.status.lock().runtime = "OpenXR (Web HUD)".into();
        service.status.lock().active = true;
    } else {
        tracing::info!("VR browser preview listening at {}", hud_url());
    }

    listener.set_nonblocking(true)?;

    while !cancel.is_cancelled() {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let live = Arc::clone(&live);
                thread::spawn(move || {
                    let _ = handle_connection(&mut stream, &live);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e.into()),
        }
    }

    if own_status {
        service.status.lock().active = false;
        service.status.lock().message = "In-headset HUD stopped".into();
    }
    Ok(())
}

fn handle_connection(stream: &mut std::net::TcpStream, live: &LiveService) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let raw_path = req
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    // The layout is selected client-side from the query string; route on path only.
    let path = raw_path.split('?').next().unwrap_or("/");

    let (status, content_type, body) = match path {
        "/api/live" => {
            let snap = live.snapshot.lock().clone();
            let json = serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into());
            ("200 OK", "application/json", json)
        }
        // Cached circuit outline for the current track; `null` when none exists.
        "/api/track-map" => {
            let track = live.snapshot.lock().track.clone();
            let outline = race_refinery_storage::load_track_map(&track);
            let json = serde_json::to_string(&outline).unwrap_or_else(|_| "null".into());
            ("200 OK", "application/json", json)
        }
        "/api/health" => ("200 OK", "application/json", r#"{"ok":true}"#.to_string()),
        "/vr" | "/" => (
            "200 OK",
            "text/html; charset=utf-8",
            VR_HUD_HTML.to_string(),
        ),
        _ => ("404 Not Found", "text/plain", "Not found".to_string()),
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

// Single page that renders any overlay layout client-side, selected by the
// `?layout=` query (ironman | standings | relative | radar | trackmap) and the optional
// `?pace=` query (best | optimal | both). The native OpenXR layer
// mirrors the `ironman` coach layout in Direct2D; this page is the browser
// preview and the visual reference. `?layout=ironman` is the default.
const VR_HUD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Race Refinery VR HUD</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    html, body { width: 100%; min-height: 100%; background: transparent; overflow: hidden; }
    body {
      font-family: "Consolas", "Cascadia Mono", "Segoe UI Mono", monospace;
      color: #5dffa8;
      display: flex; align-items: center; justify-content: center;
      padding: 8px;
      text-shadow: 0 0 6px rgba(0,0,0,0.95), 0 0 2px #000;
    }
    .wait {
      color: rgba(93,255,168,0.7); font-size: 14px; letter-spacing: 0.12em;
      text-transform: uppercase;
    }
    .slow { color: #ff6b6b; } .fast { color: #5dffa8; } .neutral { color: rgba(93,255,168,0.75); }
    .warn { color: #ffb347; }

    /* Iron Man coach HUD: distributed across the upper windshield, transparent. */
    .ironman { position: relative; width: min(94vw, 880px); height: 260px; pointer-events: none; user-select: none; }
    .ironman .corner { position: absolute; font-size: 18px; font-weight: 700; letter-spacing: 0.12em; }
    .ironman .pos { top: 0; left: 0; }
    .ironman .flag { top: 0; right: 0; }
    .ironman .hero { position: absolute; top: 44px; left: 50%; transform: translateX(-50%); text-align: center; }
    .ironman .lapnum { font-size: 12px; letter-spacing: 0.24em; color: rgba(93,255,168,0.7); }
    .ironman .laptime { font-size: 64px; font-weight: 700; line-height: 1.05; color: #e8fff3; }
    .ironman .gap { position: absolute; top: 96px; text-align: center; }
    .ironman .gap.ahead { left: 0; } .ironman .gap.behind { right: 0; }
    .ironman .gap .lbl { display: block; font-size: 11px; letter-spacing: 0.18em; color: rgba(93,255,168,0.55); }
    .ironman .gap .val { font-size: 26px; font-weight: 700; }
    .ironman .deltas { position: absolute; top: 150px; left: 50%; transform: translateX(-50%); display: flex; gap: 24px; font-size: 22px; font-weight: 700; white-space: nowrap; }
    .ironman .pack { position: absolute; top: 188px; left: 50%; transform: translateX(-50%); font-size: 24px; font-weight: 700; letter-spacing: 0.12em; }
    .ironman .sectors { position: absolute; bottom: 26px; left: 0; right: 0; display: grid; grid-template-columns: repeat(3, 1fr); gap: 14px; }
    .ironman .sector { height: 4px; background: rgba(93,255,168,0.15); position: relative; }
    .ironman .sector .fill { position: absolute; left: 0; top: 0; bottom: 0; background: #5dffa8; }
    .ironman .footer { position: absolute; bottom: 0; left: 50%; transform: translateX(-50%); font-size: 14px; letter-spacing: 0.1em; color: rgba(93,255,168,0.7); }

    /* Phase C list/board layouts. */
    .board { width: min(94vw, 520px); font-size: 16px; }
    .board h1 { font-size: 12px; letter-spacing: 0.22em; color: rgba(93,255,168,0.6); margin-bottom: 8px; text-transform: uppercase; }
    .board .row { display: grid; grid-template-columns: 36px 48px 1fr auto; gap: 10px; padding: 3px 0; }
    .board .row.you { color: #e8fff3; font-weight: 700; }
    .board .num { color: #d8ffe9; }
    .relative .row { grid-template-columns: 48px 1fr auto; }
    .radar { position: relative; width: 320px; height: 320px; }
    .radar .me, .radar .car { position: absolute; border-radius: 50%; transform: translate(-50%, -50%); }
    .radar .me { width: 16px; height: 16px; background: #e8fff3; left: 50%; top: 50%; }
    .radar .car { width: 12px; height: 12px; background: #ffb347; left: 50%; }
    .trackmap { width: 360px; height: 360px; }
    .trackmap .circuit { fill: none; stroke: rgba(93,255,168,0.55); stroke-width: 0.02; stroke-linejoin: round; }
    .trackmap .start { fill: none; stroke: #5dffa8; stroke-width: 0.012; }
    .trackmap .car { fill: #ffb347; }
    .trackmap .car.pit { fill: rgba(93,255,168,0.5); }
    .trackmap .me { fill: #e8fff3; }
  </style>
</head>
<body>
  <div id="root"><p class="wait">Awaiting telemetry</p></div>
  <script>
    const params = new URLSearchParams(location.search);
    const LAYOUT = params.get("layout") || "ironman";
    const PACE = params.get("pace") || "both";

    function fmt(ms) {
      if (ms == null || ms <= 0) return "\u2014";
      const s = ms / 1000, m = Math.floor(s / 60), sec = s - m * 60;
      return m > 0 ? m + ":" + sec.toFixed(3).padStart(6, "0") : sec.toFixed(3);
    }
    function fmtDelta(ms) { return ms == null ? "\u2014" : (ms >= 0 ? "+" : "") + (ms / 1000).toFixed(3); }
    function fmtGap(sec) { return sec == null ? "\u2014" : sec.toFixed(1) + "s"; }
    function deltaClass(ms) { return ms == null ? "neutral" : ms > 0 ? "slow" : ms < 0 ? "fast" : "neutral"; }
    function position(s) {
      const c = s.playerClassPosition, a = s.playerPosition;
      if (c != null && a != null && c !== a) return "P" + c + " \u00b7 P" + a;
      if (c != null) return "P" + c;
      if (a != null) return "P" + a;
      return "";
    }
    const PACK = { clear: "CLEAR", carLeft: "\u25C0 CAR", carRight: "CAR \u25B6", threeWide: "3-WIDE", twoCarsLeft: "2 LEFT", twoCarsRight: "2 RIGHT", off: "" };
    function hasLiveData(s) { return s && (s.track || s.lap > 0 || (s.fuelLevel != null && s.fuelLevel > 0)); }

    function fieldPace(s) {
      const b = s.deltaToSessionBestMs, o = s.deltaToSessionOptimalMs;
      if (PACE === "optimal" && o != null) return '<span class="' + deltaClass(o) + '">OPT ' + fmtDelta(o) + '</span>';
      if (PACE === "both") {
        let out = "";
        if (b != null) out += '<span class="' + deltaClass(b) + '">FLD ' + fmtDelta(b) + '</span>';
        if (o != null) out += '<span class="' + deltaClass(o) + '">OPT ' + fmtDelta(o) + '</span>';
        return out;
      }
      return b != null ? '<span class="' + deltaClass(b) + '">FLD ' + fmtDelta(b) + '</span>' : "";
    }

    function sectorPct(s, sectorNum) {
      const sec = (s.sectors || []).find(x => x.sectorNum === sectorNum);
      if (sec && sec.completed) return 100;
      if (s.currentSector !== sectorNum) return 0;
      const bounds = (s.sectorBoundaries && s.sectorBoundaries.length >= 2)
        ? s.sectorBoundaries : [0, 0.33, 0.66, 1];
      const start = bounds[sectorNum - 1] ?? 0;
      const end = bounds[sectorNum] ?? 1;
      const span = end - start;
      if (span <= 0) return 0;
      return Math.min(100, Math.max(0, ((s.lapDistPct || 0) - start) / span * 100));
    }

    function renderIronman(s) {
      const packLabel = PACK[s.packState] || "";
      const packClass = s.packState === "clear" ? "fast" : "warn";
      const sectorList = (s.sectors && s.sectors.length)
        ? s.sectors
        : [{ sectorNum: 1 }, { sectorNum: 2 }, { sectorNum: 3 }];
      const sectors = sectorList.map(sec => {
        const pct = sectorPct(s, sec.sectorNum);
        return '<div class="sector"><div class="fill" style="width:' + pct + '%"></div></div>';
      }).join("");
      const sectorCols = 'repeat(' + sectorList.length + ', 1fr)';
      return '<div class="ironman">' +
        '<div class="corner pos">' + position(s) + '</div>' +
        (s.sessionFlags ? '<div class="corner flag warn">FLAG</div>' : '') +
        '<div class="hero"><div class="lapnum">LAP ' + (s.lap || 0) + '</div>' +
          '<div class="laptime">' + fmt(s.lapTimeMs) + '</div></div>' +
        '<div class="gap ahead"><span class="lbl">AHEAD</span><span class="val">' + fmtGap(s.gapToCarAheadS) + '</span></div>' +
        '<div class="gap behind"><span class="lbl">BEHIND</span><span class="val">' + fmtGap(s.gapToCarBehindS) + '</span></div>' +
        '<div class="deltas">' +
          '<span class="' + deltaClass(s.deltaToBestMs) + '">\u0394B ' + fmtDelta(s.deltaToBestMs) + '</span>' +
          '<span class="' + deltaClass(s.deltaToLastMs) + '">\u0394L ' + fmtDelta(s.deltaToLastMs) + '</span>' +
          fieldPace(s) +
        '</div>' +
        (packLabel ? '<div class="pack ' + packClass + '">' + packLabel + '</div>' : '') +
        '<div class="sectors" style="grid-template-columns:' + sectorCols + '">' + sectors + '</div>' +
        '<div class="footer">' + (s.fuelLevel || 0).toFixed(1) + ' L \u00b7 ' + Math.round(s.speed || 0) + '</div>' +
      '</div>';
    }

    function sortByPos(list) {
      return (list || []).slice().sort((a, b) => (a.position > 0 ? a.position : 1e9) - (b.position > 0 ? b.position : 1e9));
    }
    function renderStandings(s) {
      const rows = sortByPos(s.competitors).slice(0, 12).map(c =>
        '<div class="row' + (c.isPlayer ? ' you' : '') + '">' +
          '<span>' + (c.position > 0 ? c.position : "\u2014") + '</span>' +
          '<span class="num">#' + (c.carNumber || c.carIdx) + '</span>' +
          '<span>' + c.driverName + (c.onPitRoad ? ' <span class="warn">PIT</span>' : '') + '</span>' +
          '<span>' + fmt(c.bestLapMs) + '</span>' +
        '</div>').join("");
      return '<div class="board"><h1>Standings</h1>' + rows + '</div>';
    }
    function renderRelative(s) {
      const near = (s.competitors || [])
        .filter(c => !c.isPlayer && c.gapToPlayerS != null && Math.abs(c.gapToPlayerS) <= 8)
        .sort((a, b) => b.gapToPlayerS - a.gapToPlayerS)
        .map(c =>
          '<div class="row">' +
            '<span class="num">#' + (c.carNumber || c.carIdx) + '</span>' +
            '<span>' + c.driverName + '</span>' +
            '<span class="' + (c.gapToPlayerS >= 0 ? 'fast' : 'slow') + '">' + (c.gapToPlayerS >= 0 ? '+' : '') + c.gapToPlayerS.toFixed(1) + 's</span>' +
          '</div>').join("");
      return '<div class="board relative"><h1>Relative</h1>' + near +
        '<div class="row you"><span class="num">YOU</span><span>' + position(s) + '</span><span></span></div></div>';
    }
    function renderRadar(s) {
      const cars = (s.competitors || [])
        .filter(c => !c.isPlayer && c.gapToPlayerS != null && Math.abs(c.gapToPlayerS) <= 3)
        .map(c => '<div class="car" style="top:' + (50 - c.gapToPlayerS * 14) + '%"></div>').join("");
      return '<div class="radar"><div class="me"></div>' + cars + '</div>';
    }

    // Outline for the current track, fetched on track change (see /api/track-map).
    let TRACK_MAP = null, TRACK_MAP_FOR = null;
    function pointAt(points, pct) {
      const t = ((pct % 1) + 1) % 1;
      const first = points[0], last = points[points.length - 1];
      const mix = (a, b, u) => ({ x: a.x + (b.x - a.x) * u, y: a.y + (b.y - a.y) * u });
      if (t <= first.pct || t >= last.pct) {
        const span = 1 - last.pct + first.pct;
        if (span <= 0) return first;
        const travelled = t >= last.pct ? t - last.pct : 1 - last.pct + t;
        return mix(last, first, travelled / span);
      }
      let hi = points.findIndex(p => p.pct > t);
      if (hi < 1) hi = points.length - 1;
      const a = points[hi - 1], b = points[hi], span = b.pct - a.pct;
      return mix(a, b, span > 0 ? (t - a.pct) / span : 0);
    }
    function renderTrackMap(s) {
      const points = (TRACK_MAP && TRACK_MAP.points) || [];
      if (points.length < 2) return '<p class="wait">No track map yet</p>';
      const dot = (pct, cls, r) => {
        const p = pointAt(points, pct || 0);
        return '<circle class="' + cls + '" cx="' + p.x + '" cy="' + p.y + '" r="' + r + '"/>';
      };
      const cars = (s.competitors || [])
        .filter(c => !c.isPlayer)
        .map(c => dot(c.lapDistPct, c.onPitRoad ? 'car pit' : 'car', 0.02)).join("");
      return '<svg class="trackmap" viewBox="0 0 1 1" preserveAspectRatio="xMidYMid meet">' +
        '<path class="circuit" d="' + TRACK_MAP.svgPath + '"/>' +
        dot(0, 'start', 0.018) + cars + dot(s.lapDistPct, 'me', 0.026) +
      '</svg>';
    }

    const RENDERERS = { ironman: renderIronman, standings: renderStandings, relative: renderRelative, radar: renderRadar, trackmap: renderTrackMap };

    function render(s) {
      const root = document.getElementById("root");
      if (!hasLiveData(s)) { root.innerHTML = '<p class="wait">Awaiting telemetry</p>'; return; }
      const fn = RENDERERS[LAYOUT] || renderIronman;
      root.innerHTML = fn(s);
    }
    async function refreshTrackMap(track) {
      if (LAYOUT !== "trackmap" || track === TRACK_MAP_FOR) return;
      TRACK_MAP_FOR = track;
      try { TRACK_MAP = await (await fetch("/api/track-map")).json(); } catch (_) { TRACK_MAP = null; }
    }
    async function poll() {
      try {
        const s = await (await fetch("/api/live")).json();
        await refreshTrackMap(s.track);
        render(s);
      } catch (_) {}
    }
    poll();
    setInterval(poll, 100);
  </script>
</body>
</html>
"#;
