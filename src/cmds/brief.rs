//! `brief`: a one-shot market briefing that combines the cross-asset board, volatility,
//! dollar liquidity, crypto positioning, key odds, the calendar, headlines and local events.

use super::*;
use crate::providers::{kalshi, news};

pub async fn brief(hours: i64, g: &Global, o: &Opts) -> Result<()> {
    let main_ids = config::default_board();
    let fear_ids: Vec<String> = ["VIX9D", "VIX", "VIX3M", "VVIX", "MOVE", "OVX", "DVOL", "CNNFNG", "FNG"].iter().map(|s| s.to_string()).collect();
    let liq_ids: Vec<String> = ["fred:SOFR - fred:IORB", "fred:SOFR99 - fred:IORB", "RRP", "RESERVES", "TGA", "HYOAS", "IGOAS", "NFCI", "STABLES"].iter().map(|s| s.to_string()).collect();
    let crypto_ids: Vec<String> = [
        "BTC#funding_rate", "BTC#open_interest", "BTC#long_short_ratio", "BTC#top_long_short_ratio", "BTC#taker_buy_sell_ratio",
        "ETH#funding_rate", "deribit:BTC#skew_10pct", "deribit:BTC#put_call_volume", "CB_PREMIUM_BP|(coinbase:BTC/USD / okx:BTC/USDT - 1) * 10000",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let odds_cfg = config::get().brief_odds.clone().unwrap_or_else(|| vec!["series:KXFEDDECISION".into()]);
    let odds_f = async {
        let mut out = Vec::new();
        for id in &odds_cfg {
            let r = if let Some(series) = id.strip_prefix("series:") {
                kalshi::next_event(series).await
            } else {
                match ids::parse(id) {
                    Ok(iid) => match providers::prob_source(&iid.provider) {
                        Ok(src) => src.show(&iid.symbol).await,
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(e),
                }
            };
            match r {
                Ok(ms) => out.push(ms),
                Err(e) => eprintln!("warning: odds {id}: {e}"),
            }
        }
        out
    };
    let since = now_ms() - hours * 3_600_000;
    let (main, fear, crypto, odds, newsr, cal) = tokio::join!(
        board::build(&main_ids),
        board::build(&fear_ids),
        board::build(&crypto_ids),
        odds_f,
        news::fetch_all(None),
        news::calendar()
    );
    // Liquidity: expressions go through the evaluator, plain ids through the board.
    let (liq_plain, liq_expr): (Vec<String>, Vec<String>) = liq_ids.into_iter().partition(|s| !expr::is_expression(s));
    let liq = board::build(&liq_plain).await;
    let mut spreads = Vec::new();
    for e in &liq_expr {
        if let Ok((g_, v)) = expr::evaluate(e, Timeframe::D1, Range::last(40)).await {
            let pts: Vec<(i64, f64)> = g_.into_iter().zip(v).filter(|(_, x)| x.is_finite()).collect();
            if let Some(last) = pts.last() {
                let wk = pts.iter().rev().find(|p| p.0 <= last.0 - 7 * 86_400_000).map(|p| p.1);
                spreads.push((e.clone(), last.1, wk, pts.iter().map(|p| p.1).collect::<Vec<_>>()));
            }
        }
    }
    for e in &newsr.1 {
        eprintln!("warning: feed {e}");
    }
    let mut headlines: Vec<Event> = newsr.0.into_iter().filter(|e| e.timestamp >= since && e.severity >= 2).collect();
    headlines.truncate(15);
    let now = now_ms();
    let cal: Vec<Event> = cal.context("calendar")?.into_iter().filter(|e| e.timestamp >= now - 86_400_000 && e.timestamp <= now + 3 * 86_400_000 && e.severity >= 3).collect();
    let local = store::events_since(&store::open()?, since, 30)?;

    if g.json {
        return print_json(&json!({
            "time": now, "board": main, "fear": fear, "liquidity": liq,
            "spreads": spreads.iter().map(|s| json!({"expr": s.0, "last": s.1, "week_ago": s.2})).collect::<Vec<_>>(),
            "crypto_positioning": crypto, "odds": odds, "calendar": cal, "headlines": headlines,
            "events": local.iter().map(|(_, e, _)| e).collect::<Vec<_>>(),
        }));
    }
    println!("briefing  {} {}  — numbers first; interpret with the caveats at the end\n", o.full_time(now), o.tz_name());
    println!("## 1. Cross-asset");
    print!("{}", board::render("default", &main, o).lines().skip(1).map(|l| format!("{l}\n")).collect::<String>());
    // Movers: biggest absolute day moves among % rows.
    let mut movers: Vec<(&str, f64)> = main.iter().filter(|r| r.unit.is_none() && r.error.is_none()).filter_map(|r| r.chg_day.map(|c| (r.label.as_str(), c))).collect();
    movers.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    println!("top movers (day): {}", movers.iter().take(6).map(|(l, c)| format!("{l} {c:+.2}%")).collect::<Vec<_>>().join(", "));
    println!("\n## 2. Volatility & sentiment");
    print!("{}", board::render("fear", &fear, o).lines().skip(1).map(|l| format!("{l}\n")).collect::<String>());
    let get = |rows: &[board::BoardRow], l: &str| rows.iter().find(|r| r.label == l).map(|r| r.last);
    if let (Some(a), Some(b), Some(c)) = (get(&fear, "VIX9D"), get(&fear, "VIX"), get(&fear, "VIX3M")) {
        let shape = if a > b && b > c {
            "inverted (stress now)"
        } else if a > b || b > c {
            "partly inverted (front-loaded event risk)"
        } else {
            "contango (normal)"
        };
        println!("VIX curve: 9D {a:.2} / 30D {b:.2} / 3M {c:.2} → {shape}");
    }
    println!("\n## 3. Dollar liquidity (FRED; lagged 1-5 days)");
    for (e, last, wk, hist) in &spreads {
        println!(
            "{:<26} {:+.3}  (1w ago {})  {}  {}",
            e,
            last,
            wk.map(|w| format!("{w:+.3}")).unwrap_or("-".into()),
            render::spark(&render::squeeze(hist, 24), o.ascii),
            if *last > 0.0 { "repo above IORB: reserves getting scarce" } else { "repo below IORB: ample" }
        );
    }
    print!("{}", board::render("liquidity", &liq, o).lines().skip(1).map(|l| format!("{l}\n")).collect::<String>());
    println!("\n## 4. Crypto positioning (OKX perps, Deribit options)");
    print!("{}", board::render("crypto", &crypto, o).lines().skip(1).map(|l| format!("{l}\n")).collect::<String>());
    if let Ok(liq_bars) = providers::ohlcv(&ids::parse("BTC#liquidations")?, Timeframe::H1, Range::last(24)).await {
        let tot: f64 = liq_bars.iter().map(|b| b.close).sum();
        if tot > 0.0 {
            println!("BTC liquidations collected (OKX, since daemon start, last 24h): ${}", render::fmt_qty(tot));
        }
    }
    println!("\n## 5. Key odds");
    for ms in &odds {
        print!("{}", render::series::prob_markets(&compact_event(ms), true));
    }
    println!("\n## 6. Calendar (high impact, -1d..+3d)");
    if cal.is_empty() {
        println!("none");
    }
    for e in &cal {
        println!("{} {:<4} {}{}", o.dt(e.timestamp).format("%a %m-%d %H:%M"), e.source, e.title, if e.body.is_empty() { String::new() } else { format!("  ({})", e.body) });
    }
    println!("\n## 7. Headlines (severity ≥2, last {hours}h)");
    print!("{}", render::series::events(&headlines, o, false));
    if !local.is_empty() {
        println!("\n## 8. Your rules & monitors (last {hours}h)");
        let list: Vec<Event> = local.into_iter().map(|(_, e, _)| e).collect();
        print!("{}", render::series::events(&list, o, false));
    }
    println!("\ncaveats: Yahoo quotes may be ~15m delayed; FRED lags; crypto positioning is OKX-only; headlines are keyword-scored. Drill down with candles/metric/news --about.");
    Ok(())
}

/// Collapse an event's binary markets into one market whose outcomes are the markets' Yes prices.
fn compact_event(ms: &[ProbMarket]) -> Vec<ProbMarket> {
    if ms.len() <= 1 {
        return ms.to_vec();
    }
    let outcomes = ms
        .iter()
        .map(|m| {
            let o = &m.outcomes[0];
            Outcome { name: if o.name == "Yes" { m.question.clone() } else { o.name.clone() }, price: o.price, bid: o.bid, ask: o.ask, token: None }
        })
        .collect();
    vec![ProbMarket {
        id: ms[0].event.clone().map(|e| format!("{}:{e}", ms[0].id.split(':').next().unwrap_or(""))).unwrap_or_else(|| ms[0].id.clone()),
        question: ms[0].question.clone(),
        outcomes,
        volume_24h: Some(ms.iter().filter_map(|m| m.volume_24h).sum()),
        volume: None,
        change_24h: None,
        end_date: ms[0].end_date.clone(),
        event: None,
    }]
}
