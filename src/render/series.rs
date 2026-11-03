//! Line series (metrics, probabilities), quotes, prediction markets and events.

use super::*;
use crate::model::{Bar, Event, ProbMarket, Ticker};

/// Sparkline + stats + a compact table of the most recent points.
pub fn line(title: &str, unit: &str, bars: &[Bar], step_ms: i64, rows: usize, o: &Opts) -> String {
    if bars.is_empty() {
        return format!("{title}  no data\n");
    }
    let vals: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let last = *vals.last().unwrap();
    let first = vals[0];
    let dec = decimals_for(hi.abs().max(lo.abs()).max(last.abs())).max(if hi.abs() < 0.01 { 6 } else { 0 });
    let span = o.span(bars[0].timestamp, bars.last().unwrap().timestamp);
    let mut out = format!(
        "{title}  {}  {}..{}  ({} points{})\n",
        o.tz_name(),
        o.full_time(bars[0].timestamp),
        o.full_time(bars.last().unwrap().timestamp),
        bars.len(),
        if unit.is_empty() { String::new() } else { format!(", {unit}") }
    );
    let sp = spark(&squeeze(&vals, o.width.max(10) * 2), o.ascii);
    out.push_str(&format!("spark  {sp}\n"));
    let lo_i = vals.iter().position(|v| *v == lo).unwrap_or(0);
    let hi_i = vals.iter().position(|v| *v == hi).unwrap_or(0);
    let mut stats = format!(
        "stats  last {}  first {}  chg {}  min {} @{}  max {} @{}  mean {}",
        fmt_price(last, dec),
        fmt_price(first, dec),
        if first > 0.0 && last > 0.0 { fmt_pct(pct(last, first)) } else { let d = fmt_price(last - first, dec); if d.starts_with('-') { d } else { format!("+{d}") } },
        fmt_price(lo, dec),
        o.time_label(bars[lo_i].timestamp, step_ms, span),
        fmt_price(hi, dec),
        o.time_label(bars[hi_i].timestamp, step_ms, span),
        fmt_price(mean(&vals), dec),
    );
    if let Some(z) = zscore_last(&vals) {
        stats.push_str(&format!("  z(last) {z:+.1}"));
    }
    out.push_str(&stats);
    out.push('\n');
    let mut t: Vec<Vec<String>> = vec![vec!["time".into(), "value".into(), "chg".into(), "".into()]];
    let start = bars.len().saturating_sub(rows);
    for i in start..bars.len() {
        let b = &bars[i];
        let chg = if i > 0 { fmt_price(b.close - bars[i - 1].close, dec) } else { "-".into() };
        let chg = if chg.starts_with('-') || chg == "-" { chg } else { format!("+{chg}") };
        let mark = if i == hi_i {
            "<- max"
        } else if i == lo_i {
            "<- min"
        } else {
            ""
        };
        t.push(vec![o.time_label(b.timestamp, step_ms, span), fmt_price(b.close, dec), chg, mark.into()]);
    }
    out.push_str(&table(&t));
    out
}

pub fn quotes(rows_in: &[(String, Result<Ticker, String>)], o: &Opts) -> String {
    let mut rows: Vec<Vec<String>> = vec![vec!["id".into(), "last".into(), "chg".into(), "chg%".into(), "bid".into(), "ask".into(), "low".into(), "high".into(), "vol(quote)".into(), "time".into(), "notes".into()]];
    for (label, r) in rows_in {
        match r {
            Ok(t) => {
                let dec = decimals_for(t.last);
                rows.push(vec![
                    label.clone(),
                    fmt_price(t.last, dec),
                    t.change.map(|c| format!("{:+.*}", dec, c)).unwrap_or("-".into()),
                    fmt_pct(t.percentage),
                    t.bid.map(|x| fmt_price(x, dec)).unwrap_or("-".into()),
                    t.ask.map(|x| fmt_price(x, dec)).unwrap_or("-".into()),
                    t.low.map(|x| fmt_price(x, dec)).unwrap_or("-".into()),
                    t.high.map(|x| fmt_price(x, dec)).unwrap_or("-".into()),
                    t.quote_volume.map(fmt_qty).unwrap_or("-".into()),
                    if t.timestamp > 0 { o.dt(t.timestamp).format("%m-%d %H:%M").to_string() } else { "-".into() },
                    t.notes.join("; "),
                ]);
            }
            Err(e) => rows.push(vec![label.clone(), "ERR".into(), "".into(), "".into(), "".into(), "".into(), "".into(), "".into(), "".into(), "".into(), e.lines().next().unwrap_or("").to_string()]),
        }
    }
    format!("quotes  {}  chg vs 24h ago (crypto) or previous close (TradFi)\n{}", o.tz_name(), table(&rows))
}

pub fn prob_markets(ms: &[ProbMarket], detail: bool) -> String {
    let mut out = String::new();
    for m in ms {
        let mut head = format!("{}  {}", m.id, m.question);
        let mut meta = Vec::new();
        if let Some(v) = m.volume_24h {
            meta.push(format!("vol24h {}", fmt_qty(v)));
        }
        if let Some(c) = m.change_24h {
            meta.push(format!("24h {:+.1}pp", c * 100.0));
        }
        if let Some(e) = &m.end_date {
            meta.push(format!("ends {}", e.get(..10).unwrap_or(e)));
        }
        if !meta.is_empty() {
            head.push_str(&format!("  [{}]", meta.join(", ")));
        }
        out.push_str(&head);
        out.push('\n');
        let limit = if detail { m.outcomes.len() } else { 6 };
        for oc in m.outcomes.iter().take(limit) {
            let bar_n = if oc.price.is_finite() { (oc.price * 20.0).round() as usize } else { 0 };
            let ba = match (oc.bid, oc.ask) {
                (Some(b), Some(a)) => format!("  bid {:.1}% ask {:.1}%", b * 100.0, a * 100.0),
                _ => String::new(),
            };
            out.push_str(&format!(
                "  {:>6}  {:<20}  {}{}\n",
                if oc.price.is_finite() { format!("{:.1}%", oc.price * 100.0) } else { "-".into() },
                "#".repeat(bar_n.min(20)),
                oc.name,
                ba
            ));
        }
        if m.outcomes.len() > limit {
            out.push_str(&format!("  ... {} more\n", m.outcomes.len() - limit));
        }
    }
    out
}

pub fn events(evs: &[Event], o: &Opts, show_body: bool) -> String {
    let mut out = String::new();
    for e in evs {
        let sev = match e.severity {
            3 => "!!!",
            2 => "!! ",
            1 => "!  ",
            _ => "   ",
        };
        let t = if e.timestamp > 0 { o.dt(e.timestamp).format("%m-%d %H:%M").to_string() } else { "??-?? ??:??".into() };
        let syms = if e.symbols.is_empty() { String::new() } else { format!(" [{}]", e.symbols.join(",")) };
        out.push_str(&format!("{t} {sev} {:<14} {}{}\n", e.source, e.title, syms));
        if show_body && !e.body.is_empty() {
            out.push_str(&format!("                 {}\n", e.body.chars().take(240).collect::<String>()));
        }
    }
    out
}
