//! The longer view printed under a single-id chart, built the same way for every series:
//! compressed history (coarser bars further back), swing structure and price levels, range
//! statistics, multi-timeframe state, positioning the source offers, events for the currencies
//! named in the id, the most correlated instruments of the default board, and data-quality
//! flags.

use super::*;
use crate::providers::news as feeds;
use crate::render::candles;
use crate::structure::{self, Pivot};
use std::collections::BTreeMap;

const DAY: i64 = 86_400_000;
const CURRENCIES: &[&str] = &["USD", "EUR", "JPY", "GBP", "AUD", "NZD", "CAD", "CHF", "CNY", "CNH", "HKD", "SGD", "KRW", "INR", "MXN", "SEK", "NOK"];
const POSITIONING: &[(&str, &str)] = &[("funding_rate", "funding"), ("open_interest", "OI"), ("long_short_ratio", "long/short")];

/// Print the context of `s`, whose detailed bars (`detail`) were just shown.
pub async fn print(s: &view::Series, detail: &[Bar], o: &Opts) -> Result<()> {
    let tf = Timeframe::parse(&s.interval)?;
    let series = |interval: &str, range: &str| view::Series { id: s.id.clone(), interval: interval.into(), limit: None, range: Some(range.into()), at: s.at.clone(), news: false, with: vec![], context: false };
    let (d, h) = (series("1d", "500d"), series("1h", "16d"));
    let (daily, hourly) = tokio::join!(view::fetch(&d), view::fetch(&h));
    let daily = daily.context("daily history for the context")?;
    let hourly = hourly.context("hourly bars for the context")?;
    let id = (!expr::is_expression(&s.id)).then(|| ids::parse(&s.id)).transpose()?;
    let (positioning, events, related) = tokio::join!(positioning(id.as_ref()), events(id.as_ref().map_or(s.id.to_uppercase(), |i| i.symbol.to_uppercase())), related(&s.id, &daily));
    let mut out = String::new();
    out.push_str(&history(&daily, tf, detail, o));
    out.push_str(&swings(&daily, detail, tf, o));
    out.push_str(&levels(&daily, o));
    out.push_str(&ranges(&daily));
    out.push_str(&state(&daily, &hourly));
    out.push_str(&positioning?);
    out.push_str(&events?);
    out.push_str(&related?);
    out.push_str(&quality(detail, tf, &daily));
    print!("{out}");
    Ok(())
}

/// Coarser bars before the detailed ones: daily and weekly under intraday charts, weekly and
/// monthly under daily charts, monthly under weekly ones.
fn history(daily: &[Bar], tf: Timeframe, detail: &[Bar], o: &Opts) -> String {
    let week = Timeframe { secs: 7 * 86400 };
    let month = Timeframe { secs: Timeframe::MONTH };
    let tiers: Vec<(Timeframe, usize)> = match tf.secs {
        s if s < 86400 => vec![(Timeframe::D1, 10), (week, 8)],
        86400 => vec![(week, 8), (month, 12)],
        _ => vec![(month, 12)],
    };
    let mut cut = detail.first().map(|b| b.timestamp).unwrap_or(i64::MAX);
    let mut sections = Vec::new();
    for (k, &(t, n)) in tiers.iter().enumerate() {
        // Only bars before the cut: the newest bucket may be partial, ending where the next tier starts.
        let bars = resample(&daily[..daily.partition_point(|b| b.timestamp < cut)], t);
        let mut from = bars.len().saturating_sub(n);
        // Reach back to a bucket boundary of the next coarser tier so the tiers meet without a gap.
        if let (Some(&(coarser, _)), Some(b)) = (tiers.get(k + 1), bars.get(from)) {
            let edge = bucket_start(b.timestamp, coarser);
            from = bars.partition_point(|x| x.timestamp < edge);
        }
        let bars = bars[from..].to_vec();
        if let Some(first) = bars.first() {
            cut = first.timestamp;
            sections.push((t, bars));
        }
    }
    sections.reverse();
    if sections.is_empty() {
        return String::new();
    }
    format!("\nearlier, compressed (coarser bars before the chart above)\n{}", candles::compact(&sections, o))
}

fn pivot_path(p: &[Pivot], o: &Opts, date: &str) -> String {
    let mut parts = Vec::new();
    for (i, x) in p.iter().enumerate() {
        let kind = if x.high { "H" } else { "L" };
        let chg = if i > 0 { render::pct(x.price, p[i - 1].price).map(|c| format!(" ({c:+.1}%)")).unwrap_or_default() } else { String::new() };
        parts.push(format!("{} {kind} {}{chg}", o.dt(x.timestamp).format(date), render::fmt_auto(x.price)));
    }
    parts.join(" → ")
}

/// Swing points of the last year (daily, reversals of 3 daily ATRs) and of the detailed bars.
fn swings(daily: &[Bar], detail: &[Bar], tf: Timeframe, o: &Opts) -> String {
    let mut out = String::new();
    let year: Vec<Bar> = daily.iter().filter(|b| b.timestamp >= daily.last().map_or(0, |l| l.timestamp) - 365 * DAY).cloned().collect();
    if let Some(a) = structure::atr(&year, 14) {
        let p = structure::zigzag(&year, 3.0 * a);
        out.push_str(&format!("\nswings (1d, latest of the last year, reversals ≥ 3 ATR = {}; last point still forming)\n  {}\n", render::fmt_sig(3.0 * a), pivot_path(&p[p.len().saturating_sub(9)..], o, "%Y-%m-%d")));
    }
    if tf.secs < 86400 {
        if let Some(a) = structure::atr(detail, 14) {
            let p = structure::zigzag(detail, 3.0 * a);
            if p.len() >= 2 {
                out.push_str(&format!("swings, chart ({tf}, reversals ≥ 3 ATR = {})\n  {}\n", render::fmt_sig(3.0 * a), pivot_path(&p, o, "%m-%d %H:%M")));
            }
        }
    }
    out
}

/// Price zones the daily swings of the last year returned to, nearest first, plus the
/// previous day's and week's range.
fn levels(daily: &[Bar], o: &Opts) -> String {
    let (Some(last), Some(a)) = (daily.last(), structure::atr(daily, 14)) else { return String::new() };
    let year: Vec<Bar> = daily.iter().filter(|b| b.timestamp >= last.timestamp - 365 * DAY).cloned().collect();
    let mut pivots = structure::zigzag(&year, 3.0 * a);
    pivots.pop();
    let zones = structure::levels(&pivots, 0.5 * a);
    let describe = |l: &structure::Level| format!("{} ({}x, last {}, {:+.1} ATR)", render::fmt_auto(l.price), l.touches, o.dt(l.last).format("%Y-%m-%d"), (l.price - last.close) / a);
    let mut above: Vec<&structure::Level> = zones.iter().filter(|l| l.price > last.close).collect();
    let mut below: Vec<&structure::Level> = zones.iter().filter(|l| l.price <= last.close).collect();
    above.sort_by(|x, y| x.price.total_cmp(&y.price));
    below.sort_by(|x, y| y.price.total_cmp(&x.price));
    let list = |v: &[&structure::Level]| if v.is_empty() { "none in the last year".to_string() } else { v.iter().take(3).map(|l| describe(l)).collect::<Vec<_>>().join(" · ") };
    let mut out = format!("levels (daily swing zones)\n  above: {}\n  below: {}\n", list(&above), list(&below));
    // The sessions before the latest one; only bars with a range have highs and lows to show.
    let weeks = resample(daily, Timeframe { secs: 7 * 86400 });
    if let (Some(d), Some(w)) = (daily.iter().rev().nth(1).filter(|d| d.high > d.low), weeks.iter().rev().nth(1)) {
        out.push_str(&format!(
            "  previous day {} H {} L {} · previous week H {} L {}\n",
            o.dt(d.timestamp).format("%m-%d"),
            render::fmt_auto(d.high),
            render::fmt_auto(d.low),
            render::fmt_auto(w.high),
            render::fmt_auto(w.low)
        ));
    }
    out
}

fn ranges(daily: &[Bar]) -> String {
    let Some(last) = daily.last() else { return String::new() };
    let year: Vec<Bar> = daily.iter().filter(|b| b.timestamp >= last.timestamp - 365 * DAY).cloned().collect();
    let f = |x: Option<f64>| render::fmt_pct(x);
    let high = year.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
    format!(
        "range  1w {}  1m {}  3m {}  6m {}  1y {}  ·  52w position {}  ·  from 52w high {}\n",
        f(structure::change_since(daily, 7 * DAY)),
        f(structure::change_since(daily, 30 * DAY)),
        f(structure::change_since(daily, 91 * DAY)),
        f(structure::change_since(daily, 182 * DAY)),
        f(structure::change_since(daily, 365 * DAY)),
        structure::range_position(&year).map(|p| format!("{p:.0}%")).unwrap_or("-".into()),
        f(render::pct(last.close, high)),
    )
}

/// Trend per timeframe, momentum, volatility and distance from the 20-day extremes. Intraday
/// trends appear only when the source has intraday bars.
fn state(daily: &[Bar], hourly: &[Bar]) -> String {
    let mut trends = Vec::new();
    let intraday = hourly.windows(2).filter(|w| w[1].timestamp - w[0].timestamp < DAY).count() >= 50;
    if intraday {
        for (label, bars) in [("1h", hourly.to_vec()), ("4h", resample(hourly, Timeframe { secs: 4 * 3600 }))] {
            if let Some(t) = structure::trend(&bars) {
                trends.push(format!("{label} {}", t.label()));
            }
        }
    }
    if let Some(t) = structure::trend(daily) {
        trends.push(format!("1d {}", t.label()));
    }
    let mut parts = vec![format!("trend {}", if trends.is_empty() { "-".to_string() } else { trends.join(" · ") })];
    if let Some(r) = structure::rsi(daily, 14) {
        parts.push(format!("RSI(14,1d) {r:.0}"));
    }
    if let (Some(a), Some(last)) = (structure::atr(daily, 14), daily.last()) {
        let year: Vec<Bar> = daily.iter().filter(|b| b.timestamp >= last.timestamp - 365 * DAY).cloned().collect();
        let pctile = structure::atr_percentile(&year).map(|p| format!(", {p:.0}th pct of 1y")).unwrap_or_default();
        parts.push(format!("ATR(14,1d) {} ({:.2}%{pctile})", render::fmt_sig(a), a / last.close * 100.0));
        let month = &daily[daily.len().saturating_sub(20)..];
        let hi = month.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
        let lo = month.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        parts.push(format!("20d high {:+.1} ATR, low {:+.1} ATR", (hi - last.close) / a, (lo - last.close) / a));
    }
    format!("state  {}\n", parts.join("  ·  "))
}

/// Positioning metrics the id's source provides (perpetual futures: funding, OI, long/short).
async fn positioning(id: Option<&ids::InstrumentId>) -> Result<String> {
    let Some(id) = id.filter(|i| i.metric.is_none()) else { return Ok(String::new()) };
    let offered = providers::get(&id.provider)?.metrics();
    let wanted: Vec<(&str, &str)> = POSITIONING.iter().copied().filter(|(m, _)| offered.contains(m)).collect();
    if wanted.is_empty() {
        return Ok(String::new());
    }
    let metric_ids: Vec<ids::InstrumentId> = wanted.iter().map(|(m, _)| id.with_metric(m)).collect();
    let quotes = futures::future::join_all(metric_ids.iter().map(providers::ticker)).await;
    let parts: Vec<String> = wanted
        .iter()
        .zip(quotes)
        .map(|((_, label), q)| match q {
            Ok(t) => format!("{label} {} ({} 24h)", number(t.last), t.change.map(|c| format!("{}{}", if c >= 0.0 { "+" } else { "" }, number(c))).unwrap_or("-".into())),
            Err(e) => format!("{label} unavailable ({})", e.to_string().lines().next().unwrap_or("")),
        })
        .collect();
    Ok(format!("positioning  {}\n", parts.join("  ·  ")))
}

fn number(x: f64) -> String {
    if x.abs() >= 1e4 {
        render::fmt_qty(x)
    } else {
        render::fmt_sig(x)
    }
}

/// High-impact releases in the next three days for the currencies named in `text` (a symbol
/// or an expression), or all high-impact releases when it names none.
async fn events(text: String) -> Result<String> {
    let codes: Vec<&str> = CURRENCIES.iter().copied().filter(|c| text.contains(c)).collect();
    let now = now_ms();
    let evs: Vec<Event> = feeds::calendar()
        .await?
        .into_iter()
        .filter(|e| e.severity >= 3 && e.timestamp >= now && e.timestamp <= now + 3 * DAY)
        .filter(|e| codes.is_empty() || codes.iter().any(|c| c.eq_ignore_ascii_case(&e.source)))
        .collect();
    let scope = if codes.is_empty() { "all currencies".to_string() } else { codes.join(", ") };
    if evs.is_empty() {
        return Ok(format!("events ({scope}, next 3d)  none high-impact\n"));
    }
    let list: Vec<String> = evs.iter().take(6).map(|e| format!("{} {} {}", chrono::DateTime::from_timestamp_millis(e.timestamp).unwrap_or_default().format("%a %H:%M"), e.source, e.title)).collect();
    Ok(format!("events ({scope}, next 3d, UTC)  {}\n", list.join(" · ")))
}

fn daily_returns(bars: &[Bar]) -> BTreeMap<i64, f64> {
    bars.windows(2).filter(|w| w[0].close > 0.0).map(|w| (w[1].timestamp, (w[1].close / w[0].close).ln())).collect()
}

/// The default board's instruments most correlated with this one (60 daily returns).
async fn related(id: &str, daily: &[Bar]) -> Result<String> {
    let mine = daily_returns(&daily[daily.len().saturating_sub(61)..]);
    let others: Vec<String> = config::default_board().into_iter().filter(|o| o != id).collect();
    let requests: Vec<view::Series> = others.iter().map(|o| view::Series { id: o.clone(), interval: "1d".into(), limit: Some(70), range: None, at: None, news: false, with: vec![], context: false }).collect();
    let fetched = futures::future::join_all(requests.iter().map(view::fetch)).await;
    let mut scored: Vec<(f64, &String)> = Vec::new();
    let mut unavailable = 0;
    for (other, bars) in others.iter().zip(fetched) {
        let Ok(bars) = bars else {
            unavailable += 1;
            continue;
        };
        let theirs = daily_returns(&bars);
        let pairs: Vec<(f64, f64)> = mine.iter().filter_map(|(t, x)| theirs.get(t).map(|y| (*x, *y))).collect();
        if pairs.len() < 30 {
            continue;
        }
        let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
        let rho = render::correlation(&xs, &ys);
        if rho.is_finite() && rho.abs() < 0.999 {
            scored.push((rho, other));
        }
    }
    scored.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
    let top: Vec<String> = scored.iter().take(4).map(|(r, o)| format!("{o} {r:+.2}")).collect();
    let missing = if unavailable > 0 { format!(" ({unavailable} board ids unavailable)") } else { String::new() };
    Ok(format!("related (60d corr with the default board{missing})  {}\n", if top.is_empty() { "-".to_string() } else { top.join(" · ") }))
}

fn quality(detail: &[Bar], tf: Timeframe, daily: &[Bar]) -> String {
    let now = now_ms();
    let mut flags = structure::quality(detail, tf, now);
    for f in structure::quality(daily, Timeframe::D1, now) {
        if !flags.contains(&f) {
            flags.push(format!("{f} (daily)"));
        }
    }
    format!("data quality  {}\n", if flags.is_empty() { "no issues found".to_string() } else { flags.join("; ") })
}
