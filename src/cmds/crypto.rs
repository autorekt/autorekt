//! Crypto-specific views: `prelaunch` (pre-launch and pre-IPO perps) and `funding`
//! (funding across venues, annualized).

use super::*;
use crate::http::{get_json, num, post_json};

pub async fn prelaunch(g: &Global) -> Result<()> {
    let (okx, vntl) = tokio::join!(crate::providers::okx::premarket(), crate::providers::hyperliquid::preipo());
    let mut rows: Vec<serde_json::Value> = Vec::new();
    match okx {
        Ok(list) => {
            for (inst, t) in list {
                let base = inst.trim_end_matches("-USDT-SWAP").to_string();
                let last = num(&t["last"]).unwrap_or(f64::NAN);
                let open = num(&t["open24h"]).unwrap_or(f64::NAN);
                let vol = num(&t["volCcy24h"]).unwrap_or(0.0) * last;
                rows.push(json!({"id": format!("okx:{base}/USDT:USDT"), "venue": "okx pre-market", "last": last, "chg24h": render::pct(last, open), "vol24h_usd": vol}));
            }
        }
        Err(e) => eprintln!("warning: okx: {e}"),
    }
    match vntl {
        Ok(list) => {
            for (name, c) in list {
                let last = num(&c["midPx"]).or(num(&c["markPx"])).unwrap_or(f64::NAN);
                let prev = num(&c["prevDayPx"]).unwrap_or(f64::NAN);
                rows.push(json!({"id": format!("hyperliquid:{name}"), "venue": format!("hyperliquid {}", name.split(':').next().unwrap_or("")), "last": last, "chg24h": render::pct(last, prev), "vol24h_usd": num(&c["dayNtlVlm"]), "funding_1h": num(&c["funding"]), "oi_usd": num(&c["openInterest"]).map(|o| o * last)}));
            }
        }
        Err(e) => eprintln!("warning: hyperliquid: {e}"),
    }
    if g.json {
        return print_json(&rows);
    }
    let mut t = vec![vec!["id".to_string(), "venue".into(), "last".into(), "24h".into(), "vol24h".into(), "OI".into(), "funding/h".into()]];
    for r in &rows {
        t.push(vec![
            r["id"].as_str().unwrap_or("").to_string(),
            r["venue"].as_str().unwrap_or("").to_string(),
            render::fmt_auto(r["last"].as_f64().unwrap_or(f64::NAN)),
            render::fmt_pct(r["chg24h"].as_f64()),
            r["vol24h_usd"].as_f64().map(render::fmt_qty).unwrap_or("-".into()),
            r["oi_usd"].as_f64().map(render::fmt_qty).unwrap_or("-".into()),
            r["funding_1h"].as_f64().map(|f| format!("{:.4}%", f * 100.0)).unwrap_or("-".into()),
        ]);
    }
    println!("pre-launch / pre-IPO perps (prices are implied valuations per share/token; thin books, wide spreads)");
    print!("{}", render::table(&t));
    println!("note: Binance pre-market perps are geo-restricted from some regions; use `autorekt candles <id>` for history");
    Ok(())
}

async fn venue_funding(coin: &str) -> Vec<(&'static str, Option<(f64, f64)>)> {
    let c = coin.to_uppercase();
    let okx = async {
        let v: serde_json::Value = get_json(&format!("https://www.okx.com/api/v5/public/funding-rate?instId={c}-USDT-SWAP")).await.ok()?;
        let r = &v["data"][0];
        let rate = num(&r["fundingRate"])?;
        let hrs = (num(&r["nextFundingTime"])? - num(&r["fundingTime"])?) / 3_600_000.0;
        Some((rate, if hrs > 0.0 { hrs } else { 8.0 }))
    };
    let hl = async {
        let v: serde_json::Value = post_json("https://api.hyperliquid.xyz/info", &json!({"type": "metaAndAssetCtxs"})).await.ok()?;
        let uni = v[0]["universe"].as_array()?;
        let i = uni.iter().position(|m| m["name"].as_str() == Some(c.as_str()))?;
        Some((num(&v[1][i]["funding"])?, 1.0))
    };
    let deribit = async {
        let v: serde_json::Value = get_json(&format!("https://www.deribit.com/api/v2/public/ticker?instrument_name={c}-PERPETUAL")).await.ok()?;
        Some((num(&v["result"]["funding_8h"])?, 8.0))
    };
    let binance = async {
        let v: serde_json::Value = get_json(&format!("{}/fapi/v1/premiumIndex?symbol={c}USDT", config::base_url("binance_futures", "https://fapi.binance.com"))).await.ok()?;
        Some((num(&v["lastFundingRate"])?, 8.0))
    };
    let (a, b, d, e) = tokio::join!(okx, hl, deribit, binance);
    vec![("okx", a), ("hyperliquid", b), ("deribit", d), ("binance", e)]
}

pub async fn funding(coins: Vec<String>, g: &Global) -> Result<()> {
    let coins = if coins.is_empty() { ["BTC", "ETH", "SOL", "XRP", "DOGE", "HYPE"].iter().map(|s| s.to_string()).collect() } else { coins };
    let res = futures::future::join_all(coins.iter().map(|c| venue_funding(c))).await;
    let apr = |r: f64, h: f64| r * (24.0 / h) * 365.0 * 100.0;
    if g.json {
        let v: Vec<_> = coins
            .iter()
            .zip(&res)
            .map(|(c, vs)| json!({"coin": c, "venues": vs.iter().map(|(n, x)| json!({"venue": n, "rate": x.map(|y| y.0), "interval_h": x.map(|y| y.1), "apr_pct": x.map(|y| apr(y.0, y.1))})).collect::<Vec<_>>()}))
            .collect();
        return print_json(&v);
    }
    println!("funding, annualized %/yr (positive = longs pay shorts); raw rate per interval in brackets");
    let mut t = vec![vec!["coin".to_string(), "okx".into(), "hyperliquid".into(), "deribit".into(), "binance".into(), "spread".into()]];
    for (c, vs) in coins.iter().zip(&res) {
        let mut row = vec![c.to_uppercase()];
        let mut aprs = Vec::new();
        for (_, x) in vs {
            row.push(match x {
                Some((r, h)) => {
                    let a = apr(*r, *h);
                    aprs.push(a);
                    format!("{a:+.1}% [{:.4}%/{}h]", r * 100.0, h)
                }
                None => "-".into(),
            });
        }
        let spread = if aprs.len() >= 2 { aprs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - aprs.iter().cloned().fold(f64::INFINITY, f64::min) } else { f64::NAN };
        row.push(if spread.is_finite() { format!("{spread:.1}pp") } else { "-".into() });
        t.push(row);
    }
    print!("{}", render::table(&t));
    println!("read: ~+11%/yr (0.01%/8h) is the neutral baseline on most venues; well above = crowded longs, negative = crowded shorts; a wide spread between venues is a basis-arbitrage opportunity or a venue-specific squeeze.");
    Ok(())
}
