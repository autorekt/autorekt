//! Local SQLite store: events (rule firings, monitor alerts), recorded series (monitor
//! `record()`, sampled metrics such as Hyperliquid OI), per-agent cursors for `diff`, and the
//! fetch cache: the latest bars and quotes of every series read, when each was fetched, which
//! series were asked for recently (the daemon keeps those fresh), and shared links.

use crate::model::{Bar, Event, MetricPoint, Ticker};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub fn open() -> Result<Connection> {
    let dir = crate::config::home();
    std::fs::create_dir_all(&dir)?;
    let c = Connection::open(dir.join("autorekt.db"))?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    c.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL, kind TEXT NOT NULL, source TEXT NOT NULL, title TEXT NOT NULL,
            body TEXT NOT NULL DEFAULT '', url TEXT, severity INTEGER NOT NULL DEFAULT 0,
            symbols TEXT NOT NULL DEFAULT '', rule TEXT);
         CREATE INDEX IF NOT EXISTS events_ts ON events(ts);
         CREATE TABLE IF NOT EXISTS points (series TEXT NOT NULL, ts INTEGER NOT NULL, value REAL NOT NULL,
            PRIMARY KEY(series, ts));
         CREATE TABLE IF NOT EXISTS cursors (agent TEXT NOT NULL, name TEXT NOT NULL, value TEXT NOT NULL,
            PRIMARY KEY(agent, name));
         CREATE TABLE IF NOT EXISTS rule_state (name TEXT PRIMARY KEY, state TEXT NOT NULL, since INTEGER,
            last_fired INTEGER, last_value REAL, last_eval INTEGER, error TEXT);
         CREATE TABLE IF NOT EXISTS bars (id TEXT NOT NULL, tf INTEGER NOT NULL, ts INTEGER NOT NULL,
            o REAL NOT NULL, h REAL NOT NULL, l REAL NOT NULL, c REAL NOT NULL, v REAL NOT NULL, closed INTEGER NOT NULL,
            PRIMARY KEY(id, tf, ts));
         CREATE TABLE IF NOT EXISTS fetches (key TEXT PRIMARY KEY, ok_at INTEGER, n INTEGER NOT NULL DEFAULT 0,
            err_at INTEGER, error TEXT);
         CREATE TABLE IF NOT EXISTS tickers (id TEXT PRIMARY KEY, json TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS wanted (id TEXT PRIMARY KEY, at INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS shares (token TEXT PRIMARY KEY, key TEXT NOT NULL, kind TEXT NOT NULL,
            title TEXT NOT NULL, spec TEXT NOT NULL, parent TEXT, origin TEXT NOT NULL, created INTEGER NOT NULL,
            expires INTEGER NOT NULL, views INTEGER NOT NULL DEFAULT 0, last_view INTEGER);
         CREATE INDEX IF NOT EXISTS shares_key ON shares(key);",
    )?;
    Ok(c)
}

pub fn add_event(c: &Connection, e: &Event, rule: Option<&str>) -> Result<i64> {
    c.execute(
        "INSERT INTO events (ts, kind, source, title, body, url, severity, symbols, rule) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![e.timestamp, e.kind, e.source, e.title, e.body, e.url, e.severity as i64, e.symbols.join(","), rule],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn events_after(c: &Connection, after_id: i64, limit: usize) -> Result<Vec<(i64, Event, Option<String>)>> {
    let mut st = c.prepare("SELECT id, ts, kind, source, title, body, url, severity, symbols, rule FROM events WHERE id > ?1 ORDER BY id ASC LIMIT ?2")?;
    let rows = st.query_map(params![after_id, limit as i64], row_event)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn events_since(c: &Connection, since_ts: i64, limit: usize) -> Result<Vec<(i64, Event, Option<String>)>> {
    let mut st = c.prepare("SELECT id, ts, kind, source, title, body, url, severity, symbols, rule FROM events WHERE ts >= ?1 ORDER BY id DESC LIMIT ?2")?;
    let rows = st.query_map(params![since_ts, limit as i64], row_event)?;
    let mut v: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    v.reverse();
    Ok(v)
}

fn row_event(r: &rusqlite::Row) -> rusqlite::Result<(i64, Event, Option<String>)> {
    let syms: String = r.get(8)?;
    Ok((
        r.get(0)?,
        Event {
            timestamp: r.get(1)?,
            kind: r.get(2)?,
            source: r.get(3)?,
            title: r.get(4)?,
            body: r.get(5)?,
            url: r.get(6)?,
            severity: r.get::<_, i64>(7)? as u8,
            symbols: if syms.is_empty() { vec![] } else { syms.split(',').map(|s| s.to_string()).collect() },
        },
        r.get(9)?,
    ))
}

pub fn max_event_id(c: &Connection) -> Result<i64> {
    Ok(c.query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |r| r.get(0))?)
}

pub fn record(c: &Connection, series: &str, pts: &[MetricPoint]) -> Result<()> {
    let tx = c.unchecked_transaction()?;
    {
        let mut st = tx.prepare("INSERT OR REPLACE INTO points (series, ts, value) VALUES (?1, ?2, ?3)")?;
        for p in pts {
            st.execute(params![series, p.timestamp, p.value])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn points(c: &Connection, series: &str, start: i64, end: i64) -> Result<Vec<MetricPoint>> {
    let mut st = c.prepare("SELECT ts, value FROM points WHERE series = ?1 AND ts >= ?2 AND ts <= ?3 ORDER BY ts")?;
    let rows = st.query_map(params![series, start, end], |r| Ok(MetricPoint { timestamp: r.get(0)?, value: r.get(1)? }))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn series_list(c: &Connection) -> Result<Vec<(String, i64, i64, i64)>> {
    let mut st = c.prepare("SELECT series, COUNT(*), MIN(ts), MAX(ts) FROM points GROUP BY series ORDER BY series")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_cursor(c: &Connection, agent: &str, name: &str) -> Result<Option<String>> {
    Ok(c.query_row("SELECT value FROM cursors WHERE agent = ?1 AND name = ?2", params![agent, name], |r| r.get(0)).optional()?)
}

pub fn set_cursor(c: &Connection, agent: &str, name: &str, value: &str) -> Result<()> {
    c.execute("INSERT OR REPLACE INTO cursors (agent, name, value) VALUES (?1, ?2, ?3)", params![agent, name, value])?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct RuleState {
    pub name: String,
    pub state: String,
    pub since: Option<i64>,
    pub last_fired: Option<i64>,
    pub last_value: Option<f64>,
    pub last_eval: Option<i64>,
    pub error: Option<String>,
}

pub fn get_rule_state(c: &Connection, name: &str) -> Result<RuleState> {
    Ok(c
        .query_row("SELECT name, state, since, last_fired, last_value, last_eval, error FROM rule_state WHERE name = ?1", params![name], |r| {
            Ok(RuleState { name: r.get(0)?, state: r.get(1)?, since: r.get(2)?, last_fired: r.get(3)?, last_value: r.get(4)?, last_eval: r.get(5)?, error: r.get(6)? })
        })
        .optional()?
        .unwrap_or(RuleState { name: name.into(), state: "inactive".into(), ..Default::default() }))
}

pub fn set_rule_state(c: &Connection, s: &RuleState) -> Result<()> {
    c.execute(
        "INSERT OR REPLACE INTO rule_state (name, state, since, last_fired, last_value, last_eval, error) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![s.name, s.state, s.since, s.last_fired, s.last_value, s.last_eval, s.error],
    )?;
    Ok(())
}

/// The last fetch of a cached series: when it last succeeded (and for how many bars), and when
/// an attempt last failed (the error itself is kept in the table for inspection).
#[derive(Debug, Clone, Default)]
pub struct Fetch {
    pub ok_at: Option<i64>,
    pub n: usize,
    pub err_at: Option<i64>,
}

pub fn fetch_state(c: &Connection, key: &str) -> Result<Fetch> {
    Ok(c
        .query_row("SELECT ok_at, n, err_at FROM fetches WHERE key = ?1", params![key], |r| Ok(Fetch { ok_at: r.get(0)?, n: r.get::<_, i64>(1)? as usize, err_at: r.get(2)? }))
        .optional()?
        .unwrap_or_default())
}

pub fn fetch_ok(c: &Connection, key: &str, at: i64, n: usize) -> Result<()> {
    c.execute(
        "INSERT INTO fetches (key, ok_at, n) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET ok_at = ?2, n = ?3, err_at = NULL, error = NULL",
        params![key, at, n as i64],
    )?;
    Ok(())
}

pub fn fetch_err(c: &Connection, key: &str, at: i64, error: &str) -> Result<()> {
    c.execute("INSERT INTO fetches (key, err_at, error) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET err_at = ?2, error = ?3", params![key, at, error])?;
    Ok(())
}

pub fn save_bars(c: &Connection, id: &str, tf: i64, bars: &[Bar]) -> Result<()> {
    let tx = c.unchecked_transaction()?;
    {
        let mut st = tx.prepare("INSERT OR REPLACE INTO bars (id, tf, ts, o, h, l, c, v, closed) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)")?;
        for b in bars {
            st.execute(params![id, tf, b.timestamp, b.open, b.high, b.low, b.close, b.volume, b.closed])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// The newest `limit` cached bars, oldest first.
pub fn load_bars(c: &Connection, id: &str, tf: i64, limit: usize) -> Result<Vec<Bar>> {
    let mut st = c.prepare("SELECT ts, o, h, l, c, v, closed FROM bars WHERE id = ?1 AND tf = ?2 ORDER BY ts DESC LIMIT ?3")?;
    let rows = st.query_map(params![id, tf, limit as i64], |r| {
        Ok(Bar { timestamp: r.get(0)?, open: r.get(1)?, high: r.get(2)?, low: r.get(3)?, close: r.get(4)?, volume: r.get(5)?, closed: r.get(6)? })
    })?;
    let mut v: Vec<Bar> = rows.filter_map(|r| r.ok()).collect();
    v.reverse();
    Ok(v)
}

pub fn save_ticker(c: &Connection, id: &str, t: &Ticker) -> Result<()> {
    c.execute("INSERT OR REPLACE INTO tickers (id, json) VALUES (?1, ?2)", params![id, serde_json::to_string(t)?])?;
    Ok(())
}

pub fn load_ticker(c: &Connection, id: &str) -> Result<Option<Ticker>> {
    let j: Option<String> = c.query_row("SELECT json FROM tickers WHERE id = ?1", params![id], |r| r.get(0)).optional()?;
    Ok(j.map(|j| serde_json::from_str(&j)).transpose()?)
}

/// Note that `id` was asked for, so the daemon keeps it fresh for a while.
pub fn want(c: &Connection, id: &str, at: i64) -> Result<()> {
    c.execute("INSERT OR REPLACE INTO wanted (id, at) VALUES (?1, ?2)", params![id, at])?;
    Ok(())
}

pub fn wanted_since(c: &Connection, since: i64) -> Result<Vec<String>> {
    let mut st = c.prepare("SELECT id FROM wanted WHERE at >= ?1 ORDER BY id")?;
    let rows = st.query_map(params![since], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Drop cached bars older than `before`.
pub fn prune_bars(c: &Connection, before: i64) -> Result<usize> {
    Ok(c.execute("DELETE FROM bars WHERE ts < ?1", params![before])?)
}

/// A shared link: a chart (`spec` is its view) or a document (`spec` holds its markdown and
/// chart tokens). Charts inside a document have `parent` set. `key` identifies the source, so
/// sharing the same thing again updates the link instead of making a new one.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Share {
    pub token: String,
    pub key: String,
    pub kind: String,
    pub title: String,
    pub spec: String,
    pub parent: Option<String>,
    pub origin: String,
    pub created: i64,
    pub expires: i64,
    pub views: i64,
    pub last_view: Option<i64>,
}

fn row_share(r: &rusqlite::Row) -> rusqlite::Result<Share> {
    Ok(Share {
        token: r.get(0)?,
        key: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        spec: r.get(4)?,
        parent: r.get(5)?,
        origin: r.get(6)?,
        created: r.get(7)?,
        expires: r.get(8)?,
        views: r.get(9)?,
        last_view: r.get(10)?,
    })
}

const SHARE_COLS: &str = "token, key, kind, title, spec, parent, origin, created, expires, views, last_view";

/// The live share with this key, if any.
pub fn share_by_key(c: &Connection, key: &str, now: i64) -> Result<Option<Share>> {
    Ok(c.query_row(&format!("SELECT {SHARE_COLS} FROM shares WHERE key = ?1 AND expires > ?2"), params![key, now], row_share).optional()?)
}

pub fn share_get(c: &Connection, token: &str, now: i64) -> Result<Option<Share>> {
    Ok(c.query_row(&format!("SELECT {SHARE_COLS} FROM shares WHERE token = ?1 AND expires > ?2"), params![token, now], row_share).optional()?)
}

/// Insert or replace a share (keeping its views), and drop expired ones.
pub fn share_put(c: &Connection, s: &Share) -> Result<()> {
    c.execute("DELETE FROM shares WHERE expires <= ?1", params![s.created])?;
    c.execute(
        "INSERT INTO shares (token, key, kind, title, spec, parent, origin, created, expires) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(token) DO UPDATE SET title = ?4, spec = ?5, expires = ?9",
        params![s.token, s.key, s.kind, s.title, s.spec, s.parent, s.origin, s.created, s.expires],
    )?;
    Ok(())
}

/// Remove the charts inside a document share.
pub fn share_drop_children(c: &Connection, token: &str) -> Result<()> {
    c.execute("DELETE FROM shares WHERE parent = ?1", params![token])?;
    Ok(())
}

/// Remove a share and the charts inside it; returns how many rows went.
pub fn share_remove(c: &Connection, token: &str) -> Result<usize> {
    Ok(c.execute("DELETE FROM shares WHERE token = ?1 OR parent = ?1", params![token])?)
}

pub fn share_viewed(c: &Connection, token: &str, at: i64) -> Result<()> {
    c.execute("UPDATE shares SET views = views + 1, last_view = ?2 WHERE token = ?1", params![token, at])?;
    Ok(())
}

/// Live top-level shares (not the charts inside documents), newest first.
pub fn shares(c: &Connection, now: i64) -> Result<Vec<Share>> {
    let mut st = c.prepare(&format!("SELECT {SHARE_COLS} FROM shares WHERE parent IS NULL AND expires > ?1 ORDER BY created DESC"))?;
    let rows = st.query_map(params![now], row_share)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
