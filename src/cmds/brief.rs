//! `brief`: a one-shot market briefing that combines the cross-asset board, volatility,
//! dollar liquidity, crypto positioning, key odds, the calendar, headlines and local events.

use super::*;
use crate::providers::{kalshi, news};

pub async fn brief(hours: i64, g: &Global, o: &Opts) -> Result<()> {
    let config::Sections { main: main_ids, fear: fear_ids, liq: liq_ids, world: world_ids, crypto: crypto_ids } = config::brief_sections();
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
    let (main, fear, crypto, world, odds, newsr, cal) = tokio::join!(
        board::build(&main_ids),
        board::build(&fear_ids),
        board::build(&crypto_ids),
        board::build(&world_ids),
        odds_f,
        news::fetch_all(None),
        news::calendar()
    );
    // Liquidity: expressions go through the evaluator, plain ids through the board.
    let (liq_plain, liq_expr): (Vec<String>, Vec<String>) = liq_ids.into_iter().partition(|s| !expr::is_expression(s));
    let liq = board::build(&liq_plain).await;
    let mut spreads = Vec::new();
    for e in &liq_expr {
        let r = tokio::time::timeout(std::time::Duration::from_secs(20), expr::evaluate(e, Timeframe::D1, Range::last(board::daily_points(e)))).await;
        let (g_, v) = match r {
            Ok(Ok(x)) => x,
            Ok(Err(err)) => {
                providers::note_issue(format!("missing {e}: {}", err.to_string().lines().next().unwrap_or("")));
                continue;
            }
            Err(_) => {
                providers::note_issue(format!("missing {e}: not ready after 20s (still downloading; the daemon keeps fetching it, try again shortly)"));
                continue;
            }
        };
        {
            let pts: Vec<(i64, f64)> = g_.into_iter().zip(v).filter(|(_, x)| x.is_finite()).collect();
            if let Some(last) = pts.last() {
                let wk = pts.iter().rev().find(|p| p.0 <= last.0 - 7 * 86_400_000).map(|p| p.1);
                let bars: Vec<Bar> = pts.iter().map(|p| Bar::point(p.0, p.1)).collect();
                let age = crate::freshness::assess_points(&bars, now_ms()).map(|f| f.label()).unwrap_or_default();
                spreads.push((e.clone(), last.1, wk, pts.iter().map(|p| p.1).collect::<Vec<_>>(), age));
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
            "time": now, "board": main, "fear": fear, "liquidity": liq, "global_rates": world,
            "spreads": spreads.iter().map(|s| json!({"expr": s.0, "last": s.1, "week_ago": s.2, "age": s.4})).collect::<Vec<_>>(),
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
    if let Some(g) = get(&fear, "SPX_GEX") {
        println!("SPX dealer gamma {g:+.1} $bn per 1% move → {}", if g >= 0.0 { "dealers dampen moves" } else { "dealers amplify moves" });
    }
    println!("\n## 3. Dollar liquidity (NY Fed same day; FRED, DTCC and Treasury lag 1-5 days)");
    for (e, last, wk, hist, age) in &spreads {
        let read = if e.contains("IORB") {
            if *last > 0.0 { "repo above IORB: reserves tight" } else { "repo below IORB: reserves ample" }
        } else {
            "swap spread, bp: more negative = dealers' balance sheets strained by Treasury supply"
        };
        let age = if age.is_empty() { String::new() } else { format!("  [{age}]") };
        println!("{:<36} {:+.3}  (1w ago {})  {}  {read}{age}", e, last, wk.map(|w| format!("{w:+.3}")).unwrap_or("-".into()), render::spark(&render::squeeze(hist, 24), o.ascii));
    }
    print!("{}", board::render("liquidity", &liq, o).lines().skip(1).map(|l| format!("{l}\n")).collect::<String>());
    println!("\n## 4. Global rates, China and Hong Kong (fed funds futures = implied average rate per month)");
    print!("{}", board::render("global_rates", &world, o).lines().skip(1).map(|l| format!("{l}\n")).collect::<String>());
    println!("\n## 5. Crypto positioning (OKX perps, Deribit options, US spot ETF flows)");
    print!("{}", board::render("crypto", &crypto, o).lines().skip(1).map(|l| format!("{l}\n")).collect::<String>());
    if let Ok(liq_bars) = providers::ohlcv(&ids::parse("BTC#liquidations")?, Timeframe::H1, Range::last(24)).await {
        let tot: f64 = liq_bars.iter().map(|b| b.close).sum();
        if tot > 0.0 {
            println!("BTC liquidations collected (OKX, since daemon start, last 24h): ${}", render::fmt_qty(tot));
        }
    }
    println!("\n## 6. Key odds");
    for ms in &odds {
        print!("{}", render::series::prob_markets(&compact_event(ms), true));
    }
    println!("\n## 7. Calendar (high impact, -1d..+3d)");
    if cal.is_empty() {
        println!("none");
    }
    for e in &cal {
        println!("{} {:<4} {}{}", o.dt(e.timestamp).format("%a %m-%d %H:%M"), e.source, e.title, if e.body.is_empty() { String::new() } else { format!("  ({})", e.body) });
    }
    println!("\n## 8. Headlines (severity ≥2, last {hours}h)");
    print!("{}", render::series::events(&headlines, o, false));
    if !local.is_empty() {
        println!("\n## 9. Your rules & monitors (last {hours}h)");
        let list: Vec<Event> = local.into_iter().map(|(_, e, _)| e).collect();
        print!("{}", render::series::events(&list, o, false));
    }
    println!("\ncaveats: Yahoo and Cboe are ~15m delayed; SPX gamma assumes dealers long calls and short puts; FRED lags; swap rates are medians of reported trades; crypto positioning is OKX-only; headlines are keyword-scored. Drill down with candles/metric/news --about.");
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
