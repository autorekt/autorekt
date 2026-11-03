//! `why`: explain. One id: locate a move, measure what else moved in the same window as
//! z-scores, and collect dated news. Several ids (or none): the correlation matrix of returns.

use super::*;
use crate::cli::WhyArgs;

pub async fn why(a: WhyArgs, g: &Global, o: &Opts) -> Result<()> {
    if a.ids.len() == 1 {
        let tf = a.interval.unwrap_or_else(|| "1h".into());
        let range = a.range.unwrap_or_else(|| "7d".into());
        return explain_move(&a.ids[0], a.at, &a.window, &range, &tf, g, o).await;
    }
    let tf = Timeframe::parse(&a.interval.unwrap_or_else(|| "1d".into()))?;
    let secs = parse_duration(&a.range.unwrap_or_else(|| "60d".into()))?;
    corr(a.ids, &tf.to_string(), ((secs / tf.secs) as usize).max(10), g, o).await
}

async fn corr(list: Vec<String>, interval: &str, limit: usize, g: &Global, o: &Opts) -> Result<()> {
    let list = if list.is_empty() {
        ["SPX", "NDX", "BTC", "ETH", "GOLD", "SILVER", "WTI", "COPPER", "US10Y", "DXY", "USDJPY", "VIX", "HYG"].iter().map(|s| s.to_string()).collect()
    } else {
        list
    };
    let tf = Timeframe::parse(interval)?;
    let parsed: Vec<ids::InstrumentId> = list.iter().map(|s| ids::parse(s)).collect::<Result<_>>()?;
    let futs = parsed.iter().map(|id| providers::ohlcv(id, tf, Range::last(limit + 10)));
    let res = futures::future::join_all(futs).await;
    // Align on timestamps present for each pair; returns are log changes (yields/VIX: differences).
    let mut series: Vec<(String, std::collections::BTreeMap<i64, f64>)> = Vec::new();
    for (id, r) in parsed.iter().zip(res) {
        let bars = r.with_context(|| format!("fetching {id}"))?;
        let level_like = matches!(ids::label(id).as_str(), "US10Y" | "US5Y" | "US30Y" | "US3M" | "VIX" | "MOVE") || id.provider == "fred";
        let mut m = std::collections::BTreeMap::new();
        // Daily bars from different sources stamp days differently; key by UTC day for >=1d.
        let key = |t: i64| if tf.secs >= 86400 { t - t.rem_euclid(86_400_000) } else { t };
        for w in bars.windows(2) {
            let v = if level_like { w[1].close - w[0].close } else { (w[1].close / w[0].close).ln() };
            if v.is_finite() {
                m.insert(key(w[1].timestamp), v);
            }
        }
        let keep: Vec<i64> = m.keys().rev().take(limit).cloned().collect();
        m.retain(|k, _| keep.contains(k));
        series.push((ids::label(id), m));
    }
    let n = series.len();
    let mut mat = vec![vec![f64::NAN; n]; n];
    let mut counts = vec![vec![0usize; n]; n];
    for i in 0..n {
        for j in 0..n {
            let (a, b) = (&series[i].1, &series[j].1);
            let pairs: Vec<(f64, f64)> = a.iter().filter_map(|(t, x)| b.get(t).map(|y| (*x, *y))).collect();
            counts[i][j] = pairs.len();
            if pairs.len() >= 8 {
                let (xs, ys): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
                let (mx, my) = (render::mean(&xs), render::mean(&ys));
                let cov: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
                let vx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
                let vy: f64 = ys.iter().map(|y| (y - my).powi(2)).sum();
                mat[i][j] = cov / (vx * vy).sqrt();
            }
        }
    }
    if g.json {
        return print_json(&json!({"ids": series.iter().map(|s| &s.0).collect::<Vec<_>>(), "matrix": mat, "observations": counts}));
    }
    println!("correlation of {} changes, last {} bars ({}; yields/VIX use differences, others log returns)", tf, limit, o.tz_name());
    let mut rows = vec![std::iter::once("".to_string()).chain(series.iter().map(|s| s.0.chars().take(6).collect())).collect::<Vec<_>>()];
    for i in 0..n {
        let mut r = vec![series[i].0.clone()];
        for j in 0..n {
            r.push(if i == j { "1".into() } else if mat[i][j].is_finite() { format!("{:+.2}", mat[i][j]) } else { "-".into() });
        }
        rows.push(r);
    }
    print!("{}", render::table(&rows));
    // Strongest pairs, for quick reading.
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            if mat[i][j].is_finite() {
                pairs.push((mat[i][j], i, j));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.abs().partial_cmp(&a.0.abs()).unwrap());
    let top: Vec<String> = pairs.iter().take(8).map(|(c, i, j)| format!("{}~{} {:+.2}", series[*i].0, series[*j].0, c)).collect();
    println!("strongest: {}", top.join("  "));
    Ok(())
}

fn news_words(label: &str, id: &ids::InstrumentId) -> String {
    let l = label.to_uppercase();
    let w = match l.as_str() {
        "BTC" => "bitcoin",
        "ETH" => "ethereum",
        "SOL" => "solana",
        "SPX" | "ES" | "SP500" => "stock market S&P 500",
        "NDX" | "NQ" | "NASDAQ" => "Nasdaq tech stocks",
        "GOLD" | "XAU" => "gold price",
        "SILVER" | "XAG" => "silver price",
        "WTI" | "OIL" | "BRENT" => "oil prices",
        "US10Y" | "US2Y" | "US30Y" | "TLT" => "Treasury yields",
        "DXY" => "dollar",
        "USDJPY" => "yen",
        "VIX" => "VIX volatility stocks",
        "COPPER" => "copper price",
        "NIKKEI" | "N225" => "Nikkei",
        "HSI" => "Hang Seng",
        _ => "",
    };
    if !w.is_empty() {
        return w.to_string();
    }
    match id.provider.as_str() {
        "okx" | "binance" | "hyperliquid" => ids::Ccxt::parse(&id.symbol).map(|c| format!("{} crypto", c.base)).unwrap_or(id.symbol.clone()),
        _ => id.symbol.trim_start_matches('^').to_string(),
    }
}

async fn explain_move(input: &str, at: Option<String>, window: &str, lookback: &str, interval: &str, g: &Global, o: &Opts) -> Result<()> {
    let id = ids::parse(input)?;
    let tf = Timeframe::parse(interval)?;
    let win = parse_duration(window)? * 1000;
    let label = ids::label(&id);
    // 1. Locate the move.
    let center = match at {
        Some(a) => parse_time(&a)?,
        None => {
            let lb = parse_duration(lookback)?;
            let bars = providers::ohlcv(&id, tf, Range::last((lb / tf.secs) as usize + 1)).await?;
            let k = ((win / tf.ms()) as usize).max(1);
            let mut best: Option<(i64, f64)> = None;
            for i in k..bars.len() {
                if let Some(c) = render::pct(bars[i].close, bars[i - k].close) {
                    if best.map_or(true, |b| c.abs() > b.1.abs()) {
                        best = Some((bars[i].timestamp + tf.ms() - win / 2, c));
                    }
                }
            }
            best.ok_or_else(|| anyhow::anyhow!("not enough data to find a move"))?.0
        }
    };
    let (w0, w1) = (center - win / 2, center + win / 2);
    // 2. The move itself, with context bars on each side.
    let ctx_bars = providers::ohlcv(&id, tf, Range { start: Some(w0 - 3 * win), end: Some(w1 + win), limit: 2000 }).await?;
    let in_w: Vec<&Bar> = ctx_bars.iter().filter(|b| b.timestamp >= w0 && b.timestamp < w1).collect();
    let before = ctx_bars.iter().filter(|b| b.timestamp < w0).last().map(|b| b.close);
    let after = in_w.last().map(|b| b.close);
    let move_pct = before.zip(after).and_then(|(a, b)| render::pct(b, a));
    // 3. What else moved, scaled by each asset's own typical move over the window.
    // Hyperliquid HIP-3 perps trade 24/7, so they show reactions while cash markets are shut
    // (after-hours earnings, weekends, holidays).
    let universe: Vec<String> = ["BTC", "ETH", "SPX", "NDX", "NQ", "hyperliquid:xyz:XYZ100", "VIX", "US10Y", "DXY", "USDJPY", "GOLD", "hyperliquid:xyz:GOLD", "SILVER", "WTI", "COPPER", "HYG", "NIKKEI", "HSI", "DAX"]
        .iter()
        .map(|s| s.to_string())
        .filter(|s| ids::parse(s).map(|x| x.base() != id.base()).unwrap_or(false))
        .collect();
    let futs = universe.iter().map(|s| async move {
        let iid = ids::parse(s).ok()?;
        let bars = providers::ohlcv(&iid, Timeframe::H1, Range { start: Some(w0 - 20 * 86_400_000), end: Some(w1 + 3_600_000), limit: 5000 }).await.ok()?;
        let b0 = bars.iter().filter(|b| b.timestamp < w0).last()?.close;
        let b1 = bars.iter().filter(|b| b.timestamp < w1).last()?.close;
        let level = ids::level_unit(&iid).is_some();
        let chg = if level { b1 - b0 } else { (b1 / b0 - 1.0) * 100.0 };
        // Typical move of the same length: stdev of window-length changes over the prior 20 days.
        let k = ((win / 3_600_000) as usize).max(1);
        let hist: Vec<f64> = bars
            .iter()
            .filter(|b| b.timestamp < w0)
            .collect::<Vec<_>>()
            .windows(k + 1)
            .map(|w| if level { w[k].close - w[0].close } else { (w[k].close / w[0].close - 1.0) * 100.0 })
            .collect();
        let sd = render::stdev(&hist);
        Some((ids::label(&iid), chg, if sd > 0.0 { chg / sd } else { f64::NAN }, level))
    });
    let mut moves: Vec<(String, f64, f64, bool)> = futures::future::join_all(futs).await.into_iter().flatten().collect();
    moves.sort_by(|a, b| b.2.abs().partial_cmp(&a.2.abs()).unwrap_or(std::cmp::Ordering::Equal));
    // 4. News from the window (Google News date operators) + local events.
    let d0 = chrono::DateTime::from_timestamp_millis(w0 - 86_400_000).unwrap_or_default().format("%Y-%m-%d");
    let d1 = chrono::DateTime::from_timestamp_millis(w1 + 86_400_000).unwrap_or_default().format("%Y-%m-%d");
    let q = format!("{} after:{d0} before:{d1}", news_words(&label, &id));
    let mut headlines = crate::providers::news::google(&q).await.context("news search for the window failed")?;
    headlines.retain(|e| e.timestamp >= w0 - 12 * 3_600_000 && e.timestamp <= w1 + 24 * 3_600_000);
    headlines.sort_by_key(|e| (std::cmp::Reverse(e.severity), e.timestamp));
    headlines.truncate(14);
    headlines.sort_by_key(|e| e.timestamp);
    let local = store::events_since(&store::open()?, w0, 50)?.into_iter().filter(|(_, e, _)| e.timestamp <= w1).map(|(_, e, _)| e).collect::<Vec<_>>();
    if g.json {
        return print_json(&json!({
            "id": id.to_string(), "window": [w0, w1], "move_pct": move_pct,
            "bars": in_w, "cross_asset": moves.iter().map(|m| json!({"id": m.0, "change": m.1, "z": m.2, "unit": if m.3 { "level" } else { "pct" }})).collect::<Vec<_>>(),
            "news": headlines, "events": local,
        }));
    }
    println!(
        "why {}  window {} .. {} {}  move {}",
        label,
        o.full_time(w0),
        o.full_time(w1),
        o.tz_name(),
        move_pct.map(|m| format!("{m:+.2}%")).unwrap_or("-".into())
    );
    let view: Vec<Bar> = ctx_bars.iter().filter(|b| b.timestamp >= w0 - win && b.timestamp < w1 + win / 2).cloned().collect();
    print!("{}", render::candles::render(&render::candles::CandleView { title: id.to_string(), tf_label: tf.to_string(), bars: &view, step_ms: tf.ms(), notes: vec![], events: &local }, o));
    println!("\nwhat else moved in the window (z = move / typical move of that length over the prior 20d):");
    let mut t = vec![vec!["id".to_string(), "change".into(), "z".into(), "".into()]];
    for (l, c, z, level) in &moves {
        let tag = if z.abs() >= 2.0 { "<- unusual" } else { "" };
        t.push(vec![l.clone(), if *level { format!("{c:+.3}") } else { format!("{c:+.2}%") }, if z.is_finite() { format!("{z:+.1}") } else { "-".into() }, tag.into()]);
    }
    print!("{}", render::table(&t));
    println!("\nnews around the window (Google News: {q}):");
    if headlines.is_empty() {
        println!("  none found — try `autorekt search \"...\" --kind news` with other words");
    }
    print!("{}", render::series::events(&headlines, o, false));
    println!("\nread: a move shared by many assets with high |z| is macro (rates, dollar, risk appetite); a move alone in {label} is idiosyncratic — look for asset-specific news.");
    Ok(())
}
