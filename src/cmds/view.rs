//! Views used by `show`: boards, single-series charts (text candles or sparkline + table),
//! prediction markets, trending odds, headlines and the macro calendar.

use super::*;
use crate::providers::news as feeds;
use crate::render::{candles as candle_view, series};

/// One series request: an id or expression with timeframe and window.
pub struct Series {
    pub id: String,
    pub interval: String,
    pub limit: Option<usize>,
    pub range: Option<String>,
    pub at: Option<String>,
    pub news: bool,
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
    id.metric.is_some() || matches!(id.provider.as_str(), "fred" | "alt" | "polymarket" | "kalshi" | "cftc" | "finra" | "llama" | "local")
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

pub async fn chart(s: &Series, g: &Global, o: &Opts) -> Result<()> {
    let tf = Timeframe::parse(&s.interval)?;
    if expr::is_expression(&s.id) {
        let r = series_range(s, tf, 72)?;
        let (grid, vals) = expr::evaluate(&s.id, tf, r).await?;
        let bars: Vec<Bar> = grid.iter().zip(vals.iter()).filter(|(_, v)| v.is_finite()).map(|(t, v)| Bar::point(*t, *v)).collect();
        return line(&s.id, "expression", &bars, tf.ms(), g, o);
    }
    let id = ids::parse(&s.id)?;
    let line_view = is_line(&id);
    let r = series_range(s, tf, if line_view { 72 } else { 48 })?;
    let bars = providers::ohlcv(&id, tf, r).await.with_context(|| format!("fetching {id}"))?;
    if line_view {
        let step = bars.windows(2).map(|w| w[1].timestamp - w[0].timestamp).min().unwrap_or(tf.ms()).max(1);
        return line(&id.to_string(), unit_label(&id), &bars, step, g, o);
    }
    if g.json {
        return print_json(&bars);
    }
    if g.csv {
        println!("timestamp,open,high,low,close,volume");
        for b in &bars {
            println!("{},{},{},{},{},{}", b.timestamp, b.open, b.high, b.low, b.close, b.volume);
        }
        return Ok(());
    }
    if s.at.is_none() {
        let t = providers::ticker(&id).await.with_context(|| format!("quote for {id}"))?;
        print!("{}", series::quotes(&[(ids::label(&id), Ok(t))], o));
    }
    let evs = if s.news {
        let mut v = feeds::google(&format!("{} market", ids::label(&id).trim_start_matches('^'))).await.context("--news: headline search failed")?;
        v.retain(|e| e.severity >= 1);
        v
    } else {
        vec![]
    };
    let notes = providers::get(&id.provider)?.notes(&id.symbol);
    let title = if ids::label(&id) != id.to_string() { format!("{} ({})", id, ids::label(&id)) } else { id.to_string() };
    print!("{}", candle_view::render(&candle_view::CandleView { title, tf_label: tf.to_string(), bars: &bars, step_ms: tf.ms(), notes, events: &evs }, o));
    Ok(())
}

fn line(title: &str, unit: &str, bars: &[Bar], step: i64, g: &Global, o: &Opts) -> Result<()> {
    if g.json {
        let pts: Vec<MetricPoint> = bars.iter().map(|b| MetricPoint { timestamp: b.timestamp, value: b.close }).collect();
        return print_json(&pts);
    }
    if g.csv {
        println!("timestamp,value");
        for b in bars {
            println!("{},{}", b.timestamp, b.close);
        }
        return Ok(());
    }
    print!("{}", series::line(title, unit, bars, step, 24, o));
    Ok(())
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
        chart(&Series { id: ms[0].id.clone(), interval: s.interval.clone(), limit: s.limit, range: s.range.clone(), at: s.at.clone(), news: false }, g, o).await?;
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
