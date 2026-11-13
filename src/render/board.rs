//! Board: one line per instrument with changes over several horizons, a trend sparkline and flags.

use super::*;
use crate::ids::{self, InstrumentId};
use crate::model::{Bar, Ticker, Timeframe};
use crate::providers::{self, Range};

#[derive(Debug, Clone, serde::Serialize)]
pub struct BoardRow {
    pub label: String,
    pub id: String,
    pub last: f64,
    pub chg_1h: Option<f64>,
    pub chg_day: Option<f64>,
    pub chg_5d: Option<f64>,
    pub chg_30d: Option<f64>,
    pub trend: Vec<f64>,
    pub flags: Vec<String>,
    pub error: Option<String>,
    pub daily: bool,
    /// When set, changes are differences in this unit (bp, pp, pt, $bn) rather than %.
    pub unit: Option<String>,
    /// Age of the latest value, and whether that is later than normal for the series.
    pub age_ms: Option<i64>,
    pub stale: bool,
}

/// Daily points a row reads. Swap rates take one file of every trade per day, so rows read two
/// weeks of them (enough for the 5-day change); charts ask for more when wanted.
pub fn daily_points(item: &str) -> usize {
    if item.contains("dtcc:") {
        12
    } else {
        40
    }
}

fn daily_only(id: &InstrumentId) -> bool {
    ids::points(id) && id.provider != "local"
}

fn close_at_or_before(bars: &[Bar], t: i64) -> Option<f64> {
    bars.iter().rev().find(|b| b.timestamp <= t).map(|b| b.close)
}

/// A board row for an expression ("RSP / SPY", "fred:SOFR - fred:IORB"): changes are
/// differences when the expression can cross zero, else percentages.
async fn expr_row(input: &str) -> BoardRow {
    let mut r = BoardRow { label: input.to_string(), id: input.to_string(), last: f64::NAN, chg_1h: None, chg_day: None, chg_5d: None, chg_30d: None, trend: vec![], flags: vec!["expr".into()], error: None, daily: false, unit: None, age_ms: None, stale: false };
    let (h, d) = tokio::join!(crate::expr::evaluate(input, Timeframe::H1, Range::last(24 * 6)), crate::expr::evaluate(input, Timeframe::D1, Range::last(daily_points(input))));
    let pts = |x: anyhow::Result<(Vec<i64>, Vec<f64>)>| -> Vec<Bar> {
        x.map(|(g, v)| g.into_iter().zip(v).filter(|(_, y)| y.is_finite()).map(|(t, y)| Bar::point(t, y)).collect()).unwrap_or_default()
    };
    let d_err = d.as_ref().err().map(|e| e.to_string());
    let (hb, db) = (pts(h), pts(d));
    let src = if hb.len() >= 2 { &hb } else { &db };
    let Some(last) = src.last() else {
        r.error = Some(d_err.unwrap_or_else(|| "no data".into()).lines().next().unwrap_or("").to_string());
        return r;
    };
    r.last = last.close;
    let crosses = db.iter().chain(hb.iter()).any(|b| b.close <= 0.0);
    if crosses {
        r.unit = Some("pt".into());
    }
    let rel = |base: Option<f64>| base.and_then(|b| if crosses { Some(r.last - b) } else { pct(r.last, b) });
    r.chg_1h = if hb.len() >= 2 { rel(close_at_or_before(&hb, last.timestamp - 3_600_000)) } else { None };
    r.chg_day = rel(close_at_or_before(&db, last.timestamp - 86_400_000).or_else(|| close_at_or_before(&hb, last.timestamp - 86_400_000)));
    r.chg_5d = rel(close_at_or_before(&db, last.timestamp - 5 * 86_400_000));
    r.chg_30d = rel(close_at_or_before(&db, last.timestamp - 30 * 86_400_000));
    r.trend = src.iter().map(|b| b.close).collect();
    if src.len() < 2 {
        r.daily = true;
    }
    // Daily points are judged by their own rhythm; intraday ones only report their age, since a
    // closed market would make them look late.
    let now = crate::model::now_ms();
    let fresh = if hb.len() >= 2 { Some(crate::freshness::Freshness { age_ms: now - last.timestamp, limit_ms: 0, closed: true }) } else { crate::freshness::assess_points(&db, now) };
    if let Some(f) = fresh {
        r.age_ms = Some(f.age_ms);
        r.stale = f.stale();
        let label = f.label();
        if !label.is_empty() {
            r.flags.insert(0, label);
        }
    }
    r
}

pub async fn row(input: &str) -> BoardRow {
    // "LABEL|expression-or-id" gives a row a short name.
    if let Some((label, rest)) = input.split_once('|') {
        let mut r = Box::pin(row(rest.trim())).await;
        r.label = label.trim().to_string();
        return r;
    }
    if crate::expr::is_expression(input) {
        return expr_row(input).await;
    }
    let label_in = input.to_string();
    let id = match ids::parse(input) {
        Ok(id) => id,
        Err(e) => {
            return BoardRow { label: label_in, id: String::new(), last: f64::NAN, chg_1h: None, chg_day: None, chg_5d: None, chg_30d: None, trend: vec![], flags: vec![], error: Some(e.to_string()), daily: false, unit: None, age_ms: None, stale: false }
        }
    };
    let upper = match label_in.split_once('#') {
        Some((a, m)) => format!("{}#{m}", a.to_uppercase()),
        None => label_in.to_uppercase(),
    };
    let mut r = BoardRow { label: upper, id: id.to_string(), last: f64::NAN, chg_1h: None, chg_day: None, chg_5d: None, chg_30d: None, trend: vec![], flags: vec![], error: None, daily: daily_only(&id), unit: None, age_ms: None, stale: false };
    if r.label.contains(':') {
        r.label = ids::label(&id);
    }
    let daily = r.daily || id.provider == "polymarket" || id.provider == "kalshi";
    let (tf, n) = if daily { (Timeframe::D1, daily_points(&id.to_string())) } else { (Timeframe::H1, 24 * 6) };
    let bars_f = providers::ohlcv(&id, tf, Range::last(n));
    let tick_f = providers::ticker(&id);
    let daily_f = async {
        if daily {
            Ok(vec![])
        } else {
            providers::ohlcv(&id, Timeframe::D1, Range::last(daily_points(&id.to_string()))).await
        }
    };
    let (bars, tick, dbars): (anyhow::Result<Vec<Bar>>, anyhow::Result<Ticker>, anyhow::Result<Vec<Bar>>) = tokio::join!(bars_f, tick_f, daily_f);
    let bars = match bars {
        Ok(b) if !b.is_empty() => b,
        Ok(_) => {
            r.error = Some("no data".into());
            return r;
        }
        Err(e) => {
            r.error = Some(e.to_string().lines().next().unwrap_or("").to_string());
            return r;
        }
    };
    let last_bar = bars.last().unwrap();
    r.last = last_bar.close;
    let lt = last_bar.timestamp;
    if let Ok(t) = &tick {
        if t.last.is_finite() && t.last != 0.0 {
            r.last = t.last;
        }
        r.chg_day = t.percentage;
        for n in &t.notes {
            if n.starts_with("P(") {
                continue;
            }
            r.flags.push(n.clone());
        }
    }
    let unit = ids::level_unit(&id);
    r.unit = unit.map(|u| u.to_string());
    let rel = |base: Option<f64>, last: f64| -> Option<f64> {
        let b = base?;
        match unit {
            Some("pp") => Some((last - b) * 100.0),
            Some("bp") => Some((last - b) * 100.0),
            Some("rbp") => Some((last - b) * 10_000.0),
            Some(_) => Some(last - b),
            None => pct(last, b),
        }
    };
    if unit.is_some() {
        // The ticker's day change is a percentage; recompute as a difference.
        r.chg_day = tick.as_ref().ok().and_then(|t| t.open).and_then(|o| rel(Some(o), r.last));
    }
    if let Some(n) = ids::unit_note(&id) {
        r.flags.push(n.to_string());
    }
    if daily {
        if r.chg_day.is_none() && bars.len() >= 2 {
            r.chg_day = rel(Some(bars[bars.len() - 2].close), r.last);
        }
        r.chg_5d = rel(close_at_or_before(&bars, lt - 5 * 86_400_000), r.last);
        r.chg_30d = rel(close_at_or_before(&bars, lt - 30 * 86_400_000), r.last);
        r.trend = bars.iter().rev().take(30).rev().map(|b| b.close).collect();
    } else {
        // Use the latest bar's own time as "now" so stale markets compare like with like.
        r.chg_1h = rel(close_at_or_before(&bars, lt - 3_600_000), r.last);
        r.chg_5d = rel(close_at_or_before(&bars, lt - 5 * 86_400_000).or_else(|| bars.first().map(|b| b.close)), r.last);
        if let Ok(d) = &dbars {
            r.chg_30d = rel(close_at_or_before(d, lt - 30 * 86_400_000), r.last);
            let closes: Vec<f64> = d.iter().map(|b| b.close).collect();
            if closes.len() >= 20 {
                let hi20 = closes[closes.len() - 21..closes.len() - 1].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let lo20 = closes[closes.len() - 21..closes.len() - 1].iter().cloned().fold(f64::INFINITY, f64::min);
                if r.last > hi20 {
                    r.flags.push("above 20d high".into());
                } else if r.last < lo20 {
                    r.flags.push("below 20d low".into());
                }
            }
        }
        r.trend = bars.iter().map(|b| b.close).collect();
        // Unusual last-hour move vs. the window's hourly moves.
        let rets: Vec<f64> = bars.windows(2).map(|w| (w[1].close / w[0].close).ln()).filter(|x| x.is_finite()).collect();
        if let Some(z) = zscore_last(&rets) {
            if z.abs() >= 2.5 {
                r.flags.push(format!("1h move z{z:+.1}"));
            }
        }
    }
    let latest = tick.as_ref().ok().map(|t| t.timestamp).filter(|t| *t > 0).unwrap_or(lt);
    let closed = r.flags.iter().any(|f| f.contains("closed"));
    let fresh = crate::freshness::assess(&id, &bars, latest, closed, now_ms_());
    r.age_ms = Some(fresh.age_ms);
    r.stale = fresh.stale();
    let label = fresh.label();
    if !label.is_empty() {
        r.flags.insert(0, label);
    }
    if let Some(d) = if daily { Some(&bars) } else { dbars.as_ref().ok() } {
        r.flags.extend(crate::structure::quality(d));
    }
    if id.provider == "alt" {
        let l = crate::providers::alt::fng_label(r.last).to_string();
        if !r.flags.contains(&l) {
            r.flags.push(l);
        }
    }

    r
}

fn now_ms_() -> i64 {
    crate::model::now_ms()
}

/// Seconds a row may take; slower ones (a first download of a long history) are reported as
/// not ready, and the daemon goes on fetching them.
const ROW_TIMEOUT_S: u64 = 20;

pub async fn build(items: &[String]) -> Vec<BoardRow> {
    let futs = items.iter().map(|s| async move {
        match tokio::time::timeout(std::time::Duration::from_secs(ROW_TIMEOUT_S), row(s)).await {
            Ok(r) => r,
            Err(_) => BoardRow {
                label: s.split('|').next().unwrap_or(s).to_string(),
                id: s.clone(),
                last: f64::NAN,
                chg_1h: None,
                chg_day: None,
                chg_5d: None,
                chg_30d: None,
                trend: vec![],
                flags: vec![],
                error: Some(format!("not ready after {ROW_TIMEOUT_S}s (still downloading; the daemon keeps fetching it, try again shortly)")),
                daily: false,
                unit: None,
                age_ms: None,
                stale: false,
            },
        }
    });
    futures::future::join_all(futs).await
}

pub fn render(name: &str, rows: &[BoardRow], o: &Opts) -> String {
    render_with(name, rows, &[], &[], o)
}

/// The board with extra columns: `headers`, and per row (in order) its `cells`.
pub fn render_with(name: &str, rows: &[BoardRow], headers: &[String], cells: &[Vec<String>], o: &Opts) -> String {
    let mut t: Vec<Vec<String>> = vec![vec!["id".into(), "last".into(), "1h".into(), "day".into(), "5d".into(), "30d".into(), "trend".into()]];
    t[0].extend(headers.iter().cloned());
    t[0].push("flags".into());
    let f = |x: Option<f64>| match x {
        Some(v) if v.is_finite() => format!("{v:+.2}%"),
        _ => "-".into(),
    };
    let mut missing = String::new();
    for (i, r) in rows.iter().enumerate() {
        if let Some(e) = &r.error {
            missing.push_str(&format!("missing  {}: {}\n", r.label, e));
            continue;
        }
        let pp = r.unit.as_deref() == Some("pp");
        let fmtc = |x: Option<f64>| match (r.unit.as_deref(), x) {
            (_, None) => "-".to_string(),
            (Some("bp"), Some(v)) => format!("{v:+.0}bp"),
            (Some("rbp"), Some(v)) => format!("{v:+.2}bp"),
            (Some("pp"), Some(v)) => format!("{v:+.1}pp"),
            (Some("$bn"), Some(v)) => format!("{v:+.1}bn"),
            (Some("ct"), Some(v)) => format!("{v:+.0}"),
            (Some(_), Some(v)) => format!("{v:+.2}"),
            (None, x) => f(x),
        };
        let last = if pp { format!("{:.1}%", r.last * 100.0) } else if r.last.abs() >= 1e7 { fmt_qty(r.last) } else { fmt_auto(r.last) };
        let mut line = vec![r.label.clone(), last, fmtc(r.chg_1h), fmtc(r.chg_day), fmtc(r.chg_5d), fmtc(r.chg_30d), spark(&squeeze(&r.trend, 24), o.ascii)];
        line.extend(cells.get(i).cloned().unwrap_or_default());
        line.push(r.flags.join(", "));
        t.push(line);
    }
    let now = o.full_time(crate::model::now_ms());
    format!(
        "board {name}  {now} {}  (day = 24h for crypto, vs previous close for TradFi; rates/spreads in bp, vol & sentiment indices in points, odds in pp; trend = last 6d hourly, or 30d for daily series)\n{}{missing}",
        o.tz_name(),
        table(&t)
    )
}
