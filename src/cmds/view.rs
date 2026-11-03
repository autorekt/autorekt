//! Read-only views: board, quote, candles, metric, prediction markets, news, calendar,
//! market listings, sources and id resolution. Each renders agent-oriented text, or JSON/CSV.

use super::*;
use crate::cli::SeriesArgs;
use crate::providers::news as feeds;
use crate::render::{candles as candle_view, series};

pub async fn metric(a: SeriesArgs, g: &Global, o: &Opts) -> Result<()> {
    if expr::is_expression(&a.id) {
        metric_expr(a, g, o).await
    } else {
        metric_id(a, g, o).await
    }
}

pub async fn calendar(days: i64, back: i64, min_impact: u8, country: Option<String>, g: &Global, o: &Opts) -> Result<()> {
    calendar_view(feeds::calendar().await?, days, back, min_impact, country, g, o)
}

pub async fn board(set: &str, g: &Global, o: &Opts) -> Result<()> {
    let items = match crate::config::board(&set) {
        Some(b) => b,
        None if set.contains(',') || ids::parse(&set).is_ok() && set != "default" => set.split(',').map(|s| s.trim().to_string()).collect(),
        None => bail!("unknown board '{set}' (have: {})", crate::config::board_names().join(", ")),
    };
    let rows = board::build(&items).await;
    if g.json {
        return print_json(&rows);
    }
    print!("{}", board::render(&set, &rows, &o));
    Ok(())
}

pub async fn quote(list: Vec<String>, g: &Global, o: &Opts) -> Result<()> {
    if list.is_empty() {
        bail!("give one or more ids, e.g. `autorekt quote BTC ETH VIX`");
    }
    let parsed: Vec<_> = list.iter().map(|s| ids::parse(s)).collect();
    let futs = parsed.iter().map(|p| async move {
        match p {
            Ok(id) => providers::ticker(id).await.map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    });
    let res = futures::future::join_all(futs).await;
    if g.json {
        let v: Vec<_> = res.iter().map(|r| match r {
            Ok(t) => serde_json::to_value(t).unwrap(),
            Err(e) => serde_json::json!({"error": e}),
        }).collect();
        return print_json(&v);
    }
    let rows: Vec<(String, std::result::Result<Ticker, String>)> = list
        .iter()
        .zip(parsed.iter())
        .zip(res)
        .map(|((raw, p), r)| (p.as_ref().map(ids::label).unwrap_or_else(|_| raw.clone()), r))
        .collect();
    print!("{}", series::quotes(&rows, &o));
    Ok(())
}

pub async fn candles(a: SeriesArgs, g: &Global, o: &Opts) -> Result<()> {
    let id = ids::parse(&a.id)?;
    let tf = Timeframe::parse(&a.interval)?;
    let r = series_range(&a, tf, 48)?;
    let bars = providers::ohlcv(&id, tf, r).await.with_context(|| format!("fetching {id}"))?;
    if g.json {
        return print_json(&bars);
    }
    if g.csv {
        print_csv(&bars);
        return Ok(());
    }
    let evs = if a.news {
        let q = ids::label(&id);
        let mut v = feeds::google(&format!("{} market", q.trim_start_matches('^'))).await.context("--news: headline search failed")?;
        v.retain(|e| e.severity >= 1);
        v
    } else {
        vec![]
    };
    let notes = providers::get(&id.provider)?.notes(&id.symbol);
    let title = if ids::label(&id) != id.to_string() { format!("{} ({})", id, ids::label(&id)) } else { id.to_string() };
    print!("{}", candle_view::render(&candle_view::CandleView { title, tf_label: tf.to_string(), bars: &bars, step_ms: tf.ms(), notes, events: &evs }, &o));
    Ok(())
}

async fn metric_expr(a: SeriesArgs, g: &Global, o: &Opts) -> Result<()> {
    let tf = Timeframe::parse(&a.interval)?;
    let r = series_range(&a, tf, 72)?;
    let (grid, vals) = expr::evaluate(&a.id, tf, r).await?;
    let bars: Vec<Bar> = grid.iter().zip(vals.iter()).filter(|(_, v)| v.is_finite()).map(|(t, v)| Bar::point(*t, *v)).collect();
    if g.json {
        let pts: Vec<MetricPoint> = bars.iter().map(|b| MetricPoint { timestamp: b.timestamp, value: b.close }).collect();
        return print_json(&pts);
    }
    if g.csv {
        println!("timestamp,value");
        for b in &bars {
            println!("{},{}", b.timestamp, b.close);
        }
        return Ok(());
    }
    print!("{}", series::line(&a.id, "expression", &bars, tf.ms(), a.rows, &o));
    Ok(())
}

async fn metric_id(a: SeriesArgs, g: &Global, o: &Opts) -> Result<()> {
    let id = ids::parse(&a.id)?;
    let tf = Timeframe::parse(&a.interval)?;
    let r = series_range(&a, tf, 72)?;
    let bars = providers::ohlcv(&id, tf, r).await.with_context(|| format!("fetching {id}"))?;
    if g.json {
        let pts: Vec<MetricPoint> = bars.iter().map(|b| MetricPoint { timestamp: b.timestamp, value: b.close }).collect();
        return print_json(&pts);
    }
    if g.csv {
        println!("timestamp,value");
        for b in &bars {
            println!("{},{}", b.timestamp, b.close);
        }
        return Ok(());
    }
    let unit = match id.metric.as_deref() {
        Some("funding_rate") | Some("funding") => "rate per funding interval (0.0001 = 0.01%)",
        Some("open_interest") | Some("oi") => "USD notional",
        Some("long_short_ratio") | Some("top_long_short_ratio") => "longs/shorts (accounts)",
        Some("taker_buy_sell_ratio") => "taker buy / sell volume",
        Some("basis") => "% perp over spot",
        _ if matches!(id.provider.as_str(), "polymarket" | "kalshi") => "probability 0-1",
        _ => "",
    };
    let step = bars.windows(2).map(|w| w[1].timestamp - w[0].timestamp).min().unwrap_or(tf.ms()).max(1);
    print!("{}", series::line(&id.to_string(), unit, &bars, step, a.rows, &o));
    Ok(())
}

pub async fn prob_search(query: Vec<String>, provider: String, limit: usize, g: &Global) -> Result<()> {
    let q = query.join(" ");
    if q.is_empty() {
        bail!("give a search query");
    }
    let provs: Vec<&str> = if provider == "all" { vec!["polymarket", "kalshi"] } else { vec![provider.as_str()] };
    let mut all = Vec::new();
    let mut errs = Vec::new();
    for pv in provs {
        match providers::prob_source(pv)?.search(&q, limit).await {
            Ok(mut v) => all.append(&mut v),
            Err(e) => errs.push(format!("{pv}: {e}")),
        }
    }
    if g.json {
        return print_json(&all);
    }
    print!("{}", series::prob_markets(&all, false));
    for e in errs {
        eprintln!("warning: {e}");
    }
    Ok(())
}

pub async fn prob_show(id: String, g: &Global) -> Result<()> {
    let iid = ids::parse(&id)?;
    let ms = providers::prob_source(&iid.provider)?.show(&iid.symbol).await?;
    if g.json {
        return print_json(&ms);
    }
    print!("{}", series::prob_markets(&ms, true));
    Ok(())
}

pub async fn prob_trending(provider: String, limit: usize, g: &Global) -> Result<()> {
    let ms = providers::prob_source(&provider)?.trending(limit).await?;
    if g.json {
        return print_json(&ms);
    }
    print!("{}", series::prob_markets(&ms, false));
    Ok(())
}

pub async fn news(since: Option<String>, about: Option<String>, topic: Option<String>, min_severity: u8, limit: usize, body: bool, g: &Global, o: &Opts) -> Result<()> {
    let (mut evs, errs) = match &about {
        Some(q) => (feeds::google(q).await?, vec![]),
        None => feeds::fetch_all(topic.as_deref()).await,
    };
    let mut evs_sorted = feeds::dedupe(std::mem::take(&mut evs));
    if let Some(s) = since {
        let cut = now_ms() - parse_duration(&s)? * 1000;
        evs_sorted.retain(|e| e.timestamp >= cut);
    }
    evs_sorted.retain(|e| e.severity >= min_severity);
    evs_sorted.truncate(limit);
    if g.json {
        return print_json(&evs_sorted);
    }
    print!("{}", series::events(&evs_sorted, &o, body));
    for e in errs {
        eprintln!("warning: feed {e}");
    }
    Ok(())
}

pub fn calendar_view(mut evs: Vec<Event>, days: i64, back: i64, min_impact: u8, country: Option<String>, g: &Global, o: &Opts) -> Result<()> {
    let now = now_ms();
    evs.retain(|e| e.timestamp >= now - back * 86_400_000 && e.timestamp <= now + days * 86_400_000 && e.severity >= min_impact);
    if let Some(c) = country {
        let cs: Vec<String> = c.split(',').map(|s| s.trim().to_uppercase()).collect();
        evs.retain(|e| cs.contains(&e.source.to_uppercase()));
    }
    if g.json {
        return print_json(&evs);
    }
    let mut rows = vec![vec!["time".to_string(), "ccy".into(), "impact".into(), "event".into(), "numbers".into()]];
    for e in &evs {
        let past = e.timestamp < now;
        rows.push(vec![
            o.dt(e.timestamp).format("%a %m-%d %H:%M").to_string() + if past { " (past)" } else { "" },
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

pub async fn markets(provider: String, filter: Option<String>, g: &Global) -> Result<()> {
    let p = providers::get(&provider)?;
    let ms = p.list_markets(filter.as_deref()).await?;
    if g.json {
        return print_json(&ms);
    }
    for m in ms.iter().take(500) {
        println!("{}  {}{}", m.id, m.kind, m.note.as_ref().map(|n| format!("  {n}")).unwrap_or_default());
    }
    if ms.len() > 500 {
        println!("... {} more (narrow with a filter)", ms.len() - 500);
    }
    Ok(())
}

pub async fn sources(check: bool) -> Result<()> {
    for p in providers::all() {
        let status = if check {
            let probe = match p.id() {
                "okx" => "okx:BTC/USDT:USDT",
                "binance" => "binance:BTC/USDT",
                "hyperliquid" => "hyperliquid:BTC",
                "yahoo" => "yahoo:^GSPC",
                "fred" => "fred:DGS10",
                "alt" => "alt:fng",
                "polymarket" => "",
                "kalshi" => "kalshi:KXFEDDECISION-26DEC-H0",
                _ => "",
            };
            if probe.is_empty() {
                match providers::prob_source(p.id()) {
                    Ok(s) => match s.trending(1).await {
                        Ok(_) => "ok".to_string(),
                        Err(e) => format!("FAIL {}", e.to_string().lines().next().unwrap_or("")),
                    },
                    Err(_) => "?".into(),
                }
            } else {
                match providers::ticker(&ids::parse(probe)?).await {
                    Ok(_) => "ok".to_string(),
                    Err(e) => format!("FAIL {}", e.to_string().lines().next().unwrap_or("")),
                }
            }
        } else {
            String::new()
        };
        println!("{:<12} {}{}", p.id(), p.describe(), if status.is_empty() { String::new() } else { format!("  [{status}]") });
        if !p.metrics().is_empty() {
            println!("{:<12}   metrics: {}", "", p.metrics().join(", "));
        }
    }
    println!("{:<12} {}", "news", "RSS: cnbc ft bloomberg marketwatch seekingalpha investing fed coindesk cointelegraph theblock decrypt; Google News search");
    println!("{:<12} {}", "calendar", "ForexFactory weekly calendar");
    Ok(())
}

pub fn resolve(list: Vec<String>) -> Result<()> {
    for s in list {
        match ids::parse(&s) {
            Ok(id) => println!("{s} -> {id}"),
            Err(e) => println!("{s} -> error: {e}"),
        }
    }
    Ok(())
}

fn series_range(a: &SeriesArgs, tf: Timeframe, default_n: usize) -> Result<Range> {
    let end = a.end.as_deref().map(parse_time).transpose()?;
    let (start, limit) = match (&a.range, a.limit) {
        (Some(r), n) => {
            let secs = parse_duration(r)?;
            let e = end.unwrap_or_else(now_ms);
            (Some(e - secs * 1000), n.unwrap_or(((secs / tf.secs) as usize + 2).max(1)))
        }
        (None, n) => (None, n.unwrap_or(default_n)),
    };
    Ok(Range { start, end, limit: limit.min(5000) })
}

fn print_csv(bars: &[Bar]) {
    println!("timestamp,open,high,low,close,volume");
    for b in bars {
        println!("{},{},{},{},{},{}", b.timestamp, b.open, b.high, b.low, b.close, b.volume);
    }
}

