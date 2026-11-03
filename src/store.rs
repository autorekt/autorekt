//! Local SQLite store: events (rule firings, monitor alerts), recorded series (monitor
//! `record()`, sampled metrics such as Hyperliquid OI) and per-agent cursors for `diff`.

use crate::model::{Event, MetricPoint};
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
            last_fired INTEGER, last_value REAL, last_eval INTEGER, error TEXT);",
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
