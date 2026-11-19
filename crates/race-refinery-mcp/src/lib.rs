//! Read-only MCP server hosted inside the Race Refinery app.
//!
//! Listens on `127.0.0.1` only when Settings enables it. Tools wrap the same
//! SQLite / analysis paths the Analyze UI uses — no LLM, no writes, no live SHM.

mod server;
mod summary;

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use race_refinery_storage::Database;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

pub use server::DEFAULT_MCP_PORT;

/// Snapshot for Settings / IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    /// User preference from settings.
    pub enabled: bool,
    /// True when a localhost listener is actually bound.
    pub listening: bool,
    pub port: u16,
    /// Full Cursor-facing URL when listening, e.g. `http://127.0.0.1:7847/mcp`.
    pub url: Option<String>,
    pub message: String,
}

impl Default for McpStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            listening: false,
            port: DEFAULT_MCP_PORT,
            url: None,
            message: "MCP server is off".into(),
        }
    }
}

/// Owns the optional localhost MCP listener for the app lifetime.
pub struct McpService {
    status: Arc<Mutex<McpStatus>>,
    /// Cancels the running serve task when present.
    cancel: Mutex<Option<CancellationToken>>,
}

impl McpService {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(McpStatus::default())),
            cancel: Mutex::new(None),
        }
    }

    pub fn status(&self) -> McpStatus {
        self.status.lock().clone()
    }

    /// Apply Settings: start when enabled, stop when disabled.
    pub fn sync(&self, enabled: bool, port: u16, db: Arc<Mutex<Database>>) {
        if !enabled {
            self.stop("MCP server is off");
            return;
        }
        let current = self.status.lock().clone();
        if current.listening && current.port == port && current.enabled {
            return;
        }
        self.stop("Restarting MCP server…");
        if let Err(err) = self.start(port, db) {
            let mut status = self.status.lock();
            status.enabled = true;
            status.listening = false;
            status.port = port;
            status.url = None;
            status.message = format!("MCP failed to start: {err}");
            tracing::error!(%err, port, "MCP server failed to start");
        }
    }

    fn start(&self, port: u16, db: Arc<Mutex<Database>>) -> anyhow::Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let cancel = CancellationToken::new();
        let child = cancel.child_token();
        let status = self.status.clone();

        // Bind synchronously so Settings can show bind failures immediately.
        let std_listener = std::net::TcpListener::bind(addr)
            .map_err(|e| anyhow::anyhow!("could not bind {addr}: {e}"))?;
        std_listener.set_nonblocking(true)?;

        let url = format!("http://127.0.0.1:{port}/mcp");
        {
            let mut s = status.lock();
            s.enabled = true;
            s.listening = true;
            s.port = port;
            s.url = Some(url.clone());
            s.message = format!("Listening at {url}");
        }
        tracing::info!(%url, "MCP server listening");

        *self.cancel.lock() = Some(cancel);

        thread::Builder::new()
            .name("rr-mcp".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(2)
                    .thread_name("rr-mcp-worker")
                    .build()
                {
                    Ok(rt) => rt,
                    Err(err) => {
                        let mut s = status.lock();
                        s.listening = false;
                        s.url = None;
                        s.message = format!("MCP runtime failed: {err}");
                        return;
                    }
                };

                rt.block_on(async move {
                    let listener = match tokio::net::TcpListener::from_std(std_listener) {
                        Ok(l) => l,
                        Err(err) => {
                            let mut s = status.lock();
                            s.listening = false;
                            s.url = None;
                            s.message = format!("MCP failed to adopt TCP listener: {err}");
                            return;
                        }
                    };

                    let mut config = StreamableHttpServerConfig::default()
                        .with_cancellation_token(child.clone());
                    // Prefer JSON responses for simple Cursor HTTP clients when available.
                    config = apply_http_config(config);

                    let service = StreamableHttpService::new(
                        move || Ok(server::RaceRefineryMcp::new(db.clone())),
                        LocalSessionManager::default().into(),
                        config,
                    );
                    let router = axum::Router::new().nest_service("/mcp", service);
                    let serve = axum::serve(listener, router).with_graceful_shutdown(async move {
                        child.cancelled().await;
                    });
                    if let Err(err) = serve.await {
                        tracing::error!(%err, "MCP server exited with error");
                        let mut s = status.lock();
                        s.listening = false;
                        s.url = None;
                        s.message = format!("MCP server stopped: {err}");
                    } else {
                        let mut s = status.lock();
                        s.listening = false;
                        s.url = None;
                        if s.enabled {
                            s.message = "MCP server stopped".into();
                        }
                    }
                });
            })
            .map_err(|e| anyhow::anyhow!("failed to spawn MCP thread: {e}"))?;

        Ok(())
    }

    fn stop(&self, message: &str) {
        if let Some(token) = self.cancel.lock().take() {
            token.cancel();
            // Brief pause so the port can free before a restart on the same port.
            thread::sleep(Duration::from_millis(80));
        }
        let mut status = self.status.lock();
        status.enabled = false;
        status.listening = false;
        status.url = None;
        status.message = message.into();
    }
}

impl Default for McpService {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply optional Streamable HTTP knobs without hard-failing if the SDK shape moves.
fn apply_http_config(config: StreamableHttpServerConfig) -> StreamableHttpServerConfig {
    // JSON responses + localhost Host allow-list when the SDK exposes them.
    let config = config.with_json_response(true);
    config.with_allowed_hosts(["127.0.0.1", "localhost", "[::1]"])
}
