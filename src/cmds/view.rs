//! Views used by `show`: boards, single-series charts (text candles or sparkline + table),
//! prediction markets, trending odds, headlines and the macro calendar.

use super::*;
use crate::providers::news as feeds;
use crate::freshness;
use crate::render::{candles as candle_view, series};

/// One series request: an id or expression with timeframe and window.
pub struct Series {
    pub id: String,
    pub interval: String,
    pub limit: Option<usize>,
    pub range: Option<String>,
    pub at: Option<String>,
    pub news: bool,
    /// Expressions shown as extra columns; `$` stands for the series itself.
    pub with: Vec<String>,
    /// Print the longer context (history, structure, state, events) under the chart.
    pub context: bool,
}

pub async fn board(items: &[String], name: &str, g: &Global, o: &Opts) -> Result<()> {
    let rows = board::build(items).await;
    if g.json {
        return print_json(&rows);
    }
    print!("{}", board::render(name, &rows, o));
    Ok(())
}

/// Series that carry one value per point render as a line (sparkline + table).
fn is_line(id: &ids::InstrumentId) -> bool {
    id.metric.is_some() || matches!(id.provider.as_str(), "polymarket" | "kalshi") || ids::points(id)
}

fn unit_label(id: &ids::InstrumentId) -> &'static str {
    match id.metric.as_deref() {
        Some("funding_rate") | Some("funding") => "rate per funding interval (0.0001 = 0.01%)",
        Some("open_interest") | Some("oi") => "USD notional",
        Some("long_short_ratio") | Some("top_long_short_ratio") => "longs/shorts (accounts)",
        Some("taker_buy_sell_ratio") => "taker buy / sell volume",
        Some("basis") => "% perp over spot",
        Some("liquidations") | Some("liquidations_long") | Some("liquidations_short") => "USD liquidated",
        _ if matches!(id.provider.as_str(), "polymarket" | "kalshi") => "probability 0-1",
        _ => "",
    }
}

/// The bars of a series (an expression's values as flat bars), as `chart` would show them.
pub async fn fetch(s: &Series) -> Result<Vec<Bar>> {
    let tf = Timeframe::parse(&s.interval)?;
    let line = expr::is_expression(&s.id) || is_line(&ids::parse(&s.id)?);
    fetch_range(s, tf, series_range(s, tf, if line { 72 } else { 48 })?).await
}

async fn fetch_range(s: &Series, tf: Timeframe, r: Range) -> Result<Vec<Bar>> {
    if expr::is_expression(&s.id) {
        let (grid, vals) = expr::evaluate(&s.id, tf, r).await?;
        return Ok(grid.iter().zip(vals.iter()).filter(|(_, v)| v.is_finite()).map(|(t, v)| Bar::point(*t, *v)).collect());
    }
    let id = ids::parse(&s.id)?;
    providers::ohlcv(&id, tf, r).await.with_context(|| format!("fetching {id}"))
}

/// The `--with` expressions evaluated on the bars' timestamps.
async fn columns(s: &Series, tf: Timeframe, r: Range, bars: &[Bar]) -> Result<Vec<render::Column>> {
    let Some(first) = bars.first() else { return Ok(vec![]) };
    let me = if expr::is_expression(&s.id) { format!("({})", s.id) } else { format!("\"{}\"", s.id) };
    let mut out = Vec::new();
    for w in &s.with {
        let range = Range { start: Some(first.timestamp), end: r.end, limit: bars.len() + 5 };
        let (grid, vals) = expr::evaluate(&w.replace('$', &me), tf, range).await.with_context(|| format!("--with {w}"))?;
        let at: std::collections::HashMap<i64, f64> = grid.into_iter().zip(vals).collect();
        out.push(render::Column { label: w.clone(), values: bars.iter().map(|b| at.get(&b.timestamp).copied().unwrap_or(f64::NAN)).collect() });
    }
    Ok(out)
}

pub async fn chart(s: &Series, g: &Global, o: &Opts) -> Result<()> {
    let tf = Timeframe::parse(&s.interval)?;
    let id = (!expr::is_expression(&s.id)).then(|| ids::parse(&s.id)).transpose()?;
    let line_view = id.as_ref().map_or(true, is_line);
    let r = series_range(s, tf, if line_view { 72 } else if s.context { 24 } else { 48 })?;
    let bars = fetch_range(s, tf, r).await?;
    let extra = columns(s, tf, r, &bars).await?;
    if g.json {
        return print_json(&bars);
    }
    if g.csv {
        print!("{}{}", csv_header(&extra), csv_rows(&s.id, tf, &bars, &extra, o));
        return Ok(());
    }
    match &id {
        Some(id) if !line_view => candles(id, s, tf, &bars, &extra, o).await?,
        Some(id) => {
            let step = bars.windows(2).map(|w| w[1].timestamp - w[0].timestamp).min().unwrap_or(tf.ms()).max(1);
            print!("{}", series::line(&id.to_string(), unit_label(id), &bars, step, 24, &extra, o));
            if let (Some(last), None) = (bars.last(), &s.at) {
                print!("{}", latest_line(&freshness::assess(id, &bars, last.timestamp, false, now_ms()), last.timestamp, o));
            }
        }
        None => print!("{}", series::line(&s.id, "expression", &bars, tf.ms(), 24, &extra, o)),
    }
    if s.context {
        context::print(s, &bars, o).await?;
    }
    Ok(())
}

/// Every bar of several series as one CSV table.
pub async fn csv_many(series: &[Series], o: &Opts) -> Result<()> {
    let mut out = String::new();
    for s in series {
        let tf = Timeframe::parse(&s.interval)?;
        let line = expr::is_expression(&s.id) || is_line(&ids::parse(&s.id)?);
        let r = series_range(s, tf, if line { 72 } else { 48 })?;
        let bars = fetch_range(s, tf, r).await?;
        let extra = columns(s, tf, r, &bars).await?;
        if out.is_empty() {
            out.push_str(&csv_header(&extra));
        }
        out.push_str(&csv_rows(&s.id, tf, &bars, &extra, o));
    }
    print!("{out}");
    Ok(())
}

fn csv_header(extra: &[render::Column]) -> String {
    let mut h = "id,date,timestamp,open,high,low,close,volume".to_string();
    for c in extra {
        h.push(',');
        h.push_str(&csv_cell(&c.label));
    }
    h + "\n"
}

/// CSV rows; `date` is the trading day for daily and longer bars, else the local time (--tz).
fn csv_rows(id: &str, tf: Timeframe, bars: &[Bar], extra: &[render::Column], o: &Opts) -> String {
    let mut out = String::new();
    for (i, b) in bars.iter().enumerate() {
        let date = if tf.secs >= 86400 {
            chrono::DateTime::from_timestamp_millis(b.timestamp).unwrap_or_default().format("%Y-%m-%d").to_string()
        } else {
            o.dt(b.timestamp).format("%Y-%m-%dT%H:%M:%S%:z").to_string()
        };
        out.push_str(&format!("{},{date},{},{},{},{},{},{}", csv_cell(id), b.timestamp, b.open, b.high, b.low, b.close, b.volume));
        for c in extra {
            let v = c.values.get(i).copied().unwrap_or(f64::NAN);
            out.push(',');
            if v.is_finite() {
                out.push_str(&v.to_string());
            }
        }
        out.push('\n');
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

async fn candles(id: &ids::InstrumentId, s: &Series, tf: Timeframe, bars: &[Bar], extra: &[render::Column], o: &Opts) -> Result<()> {
    let mut latest = None;
    if s.at.is_none() {
        let t = providers::ticker(id).await.with_context(|| format!("quote for {id}"))?;
        let closed = t.notes.iter().any(|n| n.contains("closed"));
        latest = Some((freshness::assess(id, bars, t.timestamp, closed, now_ms()), t.timestamp));
        print!("{}", series::quotes(&[(ids::label(id), Ok(t))], o));
    }
    let evs = if s.news {
        let mut v = feeds::google(&format!("{} market", ids::label(id).trim_start_matches('^'))).await.context("--news: headline search failed")?;
        v.retain(|e| e.severity >= 1);
        v
    } else {
        vec![]
    };
    let notes = providers::get(&id.provider)?.notes(&id.symbol);
    let title = if ids::label(id) != id.to_string() { format!("{} ({})", id, ids::label(id)) } else { id.to_string() };
    print!("{}", candle_view::render(&candle_view::CandleView { title, tf_label: tf.to_string(), bars, step_ms: tf.ms(), notes, events: &evs, extra }, o));
    if let Some((f, t)) = latest {
        print!("{}", latest_line(&f, t, o));
    }
    Ok(())
}

/// When the latest value is from, and whether that is late for this series.
fn latest_line(f: &freshness::Freshness, t: i64, o: &Opts) -> String {
    let label = f.label();
    format!("latest  {} {}  ({})\n", o.full_time(t), o.tz_name(), if label.is_empty() { "live" } else { &label })
}

/// A prediction market or event: outcomes, then the history of a single market.
pub async fn market(id: &ids::InstrumentId, s: &Series, g: &Global, o: &Opts) -> Result<()> {
    let ms = providers::prob_source(&id.provider)?.show(&id.symbol).await?;
    if g.json {
        return print_json(&ms);
    }
    print!("{}", series::prob_markets(&ms, true));
    if ms.len() == 1 {
        println!();
        chart(&Series { id: ms[0].id.clone(), interval: s.interval.clone(), limit: s.limit, range: s.range.clone(), at: s.at.clone(), news: false, with: s.with.clone(), context: false }, g, o).await?;
    }
    Ok(())
}

pub async fn odds(limit: usize, g: &Global) -> Result<()> {
    let (pm, ks) = (providers::prob_source("polymarket")?, providers::prob_source("kalshi")?);
    let (p, k) = tokio::join!(pm.trending(limit), ks.trending(limit));
    let (p, k) = (p.context("polymarket")?, k.context("kalshi")?);
    if g.json {
        return print_json(&json!({"polymarket": p, "kalshi": k}));
    }
    println!("polymarket (by 24h volume)");
    print!("{}", series::prob_markets(&p, false));
    println!("\nkalshi (by 24h volume)");
    print!("{}", series::prob_markets(&k, false));
    Ok(())
}

pub async fn news(topic: Option<&str>, since: Option<i64>, min_severity: u8, limit: usize, g: &Global, o: &Opts) -> Result<()> {
    if let Some(t) = topic {
        if !feeds::FEEDS.iter().any(|f| f.topic == t) {
            bail!("unknown news topic '{t}' (markets, macro, crypto); to search headlines use `autorekt search WORDS --kind news`");
        }
    }
    let (evs, errs) = feeds::fetch_all(topic).await;
    let mut evs = feeds::dedupe(evs);
    if let Some(cut) = since {
        evs.retain(|e| e.timestamp >= cut);
    }
    evs.retain(|e| e.severity >= min_severity);
    evs.truncate(limit);
    for e in errs {
        eprintln!("warning: feed {e}");
    }
    if g.json {
        return print_json(&evs);
    }
    print!("{}", series::events(&evs, o, false));
    Ok(())
}

/// Macro releases from yesterday through next week (ForexFactory publishes two weeks).
pub async fn calendar(ccys: &[String], min_impact: u8, g: &Global, o: &Opts) -> Result<()> {
    let mut evs = feeds::calendar().await?;
    let now = now_ms();
    evs.retain(|e| e.timestamp >= now - 86_400_000 && e.severity >= min_impact);
    if !ccys.is_empty() {
        evs.retain(|e| ccys.iter().any(|c| c.eq_ignore_ascii_case(&e.source)));
    }
    if g.json {
        return print_json(&evs);
    }
    let mut rows = vec![vec!["time".to_string(), "ccy".into(), "impact".into(), "event".into(), "numbers".into()]];
    for e in &evs {
        rows.push(vec![
            o.dt(e.timestamp).format("%a %m-%d %H:%M").to_string() + if e.timestamp < now { " (past)" } else { "" },
            e.source.clone(),
            ["", "low", "MED", "HIGH"][e.severity.min(3) as usize].to_string(),
            e.title.clone(),
            e.body.clone(),
        ]);
    }
    println!("calendar  {}  (ForexFactory; actuals may lag — check news)", o.tz_name());
    print!("{}", render::table(&rows));
    Ok(())
}

fn series_range(s: &Series, tf: Timeframe, default_n: usize) -> Result<Range> {
    let end = s.at.as_deref().map(parse_time).transpose()?;
    let (start, limit) = match (&s.range, s.limit) {
        (Some(r), n) => {
            let secs = parse_duration(r)?;
            let e = end.unwrap_or_else(now_ms);
            (Some(e - secs * 1000), n.unwrap_or(((secs / tf.secs) as usize + 2).max(1)))
        }
        (None, n) => (None, n.unwrap_or(default_n)),
    };
    Ok(Range { start, end, limit: limit.min(5000) })
}
