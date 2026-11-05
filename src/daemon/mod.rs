//! The background daemon: evaluates rules and timers, runs hooks, supervises monitor scripts,
//! records sampled series, and serves a local HTTP API plus shareable chart pages.
//!
//! State lives in `$AUTOREKT_HOME`: daemon.json (pid, port, token), autorekt.db, logs/.

use crate::model::{now_ms, Bar, Event, MetricPoint, Timeframe};
use crate::providers::{self, Range};
use crate::{config, ids, rules, store};
use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use std::io::Write;

mod api;
mod loops;
mod report;
mod share;
mod warm;

use api::*;
use loops::*;
use report::*;
use share::*;
pub use report::{chart_blocks, ChartReq};
pub use share::cloudflared_path;

pub const SDK_PY: &str = include_str!("../../sdk/autorekt.py");

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Info {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub started: i64,
    #[serde(default)]
    pub public_url: Option<String>,
}

pub fn info_path() -> PathBuf {
    config::home().join("daemon.json")
}
pub fn logs() -> PathBuf {
    config::home().join("logs")
}

pub fn read_info() -> Option<Info> {
    let t = std::fs::read_to_string(info_path()).ok()?;
    serde_json::from_str(&t).ok()
}

/// The running daemon's info, if it is alive.
pub fn running() -> Option<Info> {
    let i = read_info()?;
    crate::os::alive(i.pid).then_some(i)
}

/// Start the daemon in the background if it is not running. Returns its info.
pub async fn ensure() -> Result<Info> {
    if let Some(i) = running() {
        return Ok(i);
    }
    std::fs::create_dir_all(logs())?;
    let exe = std::env::current_exe()?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(logs().join("daemon.log"))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["daemon", "run"]).stdin(std::process::Stdio::null()).stdout(log.try_clone()?).stderr(log);
    crate::os::detach(&mut cmd);
    cmd.spawn().context("starting daemon")?;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Some(i) = running() {
            if crate::http::client().get(format!("http://127.0.0.1:{}/health", i.port)).send().await.is_ok() {
                return Ok(i);
            }
        }
    }
    bail!("daemon did not start; see {}", logs().join("daemon.log").display())
}

pub async fn stop() -> Result<bool> {
    match running() {
        Some(i) => {
            api("shutdown", json!({})).await?;
            for _ in 0..30 {
                if !crate::os::alive(i.pid) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = std::fs::remove_file(info_path());
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Call the daemon's local API.
pub async fn api(path: &str, body: Value) -> Result<Value> {
    let i = ensure().await?;
    let r = crate::http::client()
        .post(format!("http://127.0.0.1:{}/api/{path}", i.port))
        .bearer_auth(&i.token)
        .json(&body)
        .send()
        .await?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("daemon: {}", v["error"].as_str().unwrap_or(&status.to_string()));
    }
    Ok(v)
}


struct Shared {
    info: Mutex<Info>,
    shares: Mutex<HashMap<String, ShareSpec>>,
    reports: Mutex<HashMap<String, Report>>,
    cache: Mutex<HashMap<String, (i64, Value)>>,
    monitors: Mutex<HashMap<String, MonitorState>>,
    tunnel: Mutex<Option<tokio::process::Child>>,
    shutdown: tokio::sync::Notify,
}

#[derive(Debug, Clone, Serialize)]
struct MonitorState {
    path: String,
    pid: Option<u32>,
    restarts: u32,
    last_start: i64,
    last_exit: Option<String>,
}

type St = Arc<Shared>;

fn rand_token(n: usize) -> String {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).expect("system random number generator");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}


pub async fn run() -> Result<()> {
    if let Some(i) = running() {
        if i.pid != std::process::id() {
            bail!("daemon already running (pid {})", i.pid);
        }
    }
    let home = config::home();
    std::fs::create_dir_all(home.join("rules"))?;
    std::fs::create_dir_all(home.join("monitors"))?;
    std::fs::create_dir_all(home.join("sdk"))?;
    std::fs::create_dir_all(logs())?;
    std::fs::write(home.join("sdk").join("autorekt.py"), SDK_PY)?;
    let port = config::get().share_port.unwrap_or(0);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let port = listener.local_addr()?.port();
    let info = Info { pid: std::process::id(), port, token: rand_token(16), started: now_ms(), public_url: config::get().share_base_url.clone() };
    std::fs::write(info_path(), serde_json::to_string_pretty(&info)?)?;
    let shares = load_shares();
    let st: St = Arc::new(Shared {
        info: Mutex::new(info.clone()),
        shares: Mutex::new(shares),
        reports: Mutex::new(HashMap::new()),
        cache: Mutex::new(HashMap::new()),
        monitors: Mutex::new(HashMap::new()),
        tunnel: Mutex::new(None),
        shutdown: tokio::sync::Notify::new(),
    });
    log(&format!("daemon started pid={} port={}", info.pid, port));

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/status", post(api_status))
        .route("/api/ohlcv", post(api_ohlcv))
        .route("/api/eval", post(api_eval))
        .route("/api/record", post(api_record))
        .route("/api/event", post(api_event))
        .route("/api/share", post(api_share))
        .route("/api/report", post(api_report))
        .route("/api/shutdown", post(api_shutdown))
        .route("/r/:token", get(report_page))
        .route("/c/:token", get(chart_page))
        .route("/c/:token/data", get(chart_data))
        .with_state(st.clone());

    tokio::spawn(rule_loop());
    tokio::spawn(monitor_loop(st.clone()));
    tokio::spawn(record_loop());
    tokio::spawn(liquidation_loop());
    tokio::spawn(warm::warm_loop());

    let st2 = st.clone();
    let shutdown = async move {
        tokio::select! { _ = crate::os::stop_requested() => {}, _ = st2.shutdown.notified() => {} }
    };
    axum::serve(listener, app).with_graceful_shutdown(shutdown).await?;
    // Cleanup: stop monitors and tunnel.
    for (_, m) in st.monitors.lock().await.iter() {
        if let Some(pid) = m.pid {
            crate::os::end_group(pid);
        }
    }
    if let Some(mut t) = st.tunnel.lock().await.take() {
        let _ = t.kill().await;
    }
    if read_info().map_or(false, |i| i.pid == std::process::id()) {
        let _ = std::fs::remove_file(info_path());
    }
    log("daemon stopped");
    Ok(())
}

pub fn log(msg: &str) {
    let line = format!("{} {msg}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(logs().join("daemon.log")) {
        let _ = f.write_all(line.as_bytes());
    }
}
