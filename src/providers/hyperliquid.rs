//! Hyperliquid perps, including HIP-3 builder-deployed markets (24/7 equity, index and
//! commodity perps such as `hyperliquid:xyz:TSLA`, `hyperliquid:xyz:XYZ100`).
//!
//! Ids: `hyperliquid:BTC/USDC:USDC` (CCXT) or the native coin name `hyperliquid:BTC`,
//! HIP-3 markets as `hyperliquid:<dex>:<COIN>`.

use super::*;
use crate::http::{num, post_json};
use serde_json::{json, Value};

pub struct Hyperliquid;

fn url() -> String {
    crate::config::base_url("hyperliquid", "https://api.hyperliquid.xyz") + "/info"
}

/// Id symbol → (coin as the API names it, dex).
fn coin(symbol: &str) -> (String, Option<String>) {
    if let Some((dex, c)) = symbol.split_once(':') {
        if !dex.contains('/') {
            let c = c.split('/').next().unwrap_or(c);
            return (format!("{}:{}", dex.to_lowercase(), c.to_uppercase()), Some(dex.to_lowercase()));
        }
    }
    let base = symbol.split('/').next().unwrap_or(symbol);
    // Hyperliquid uses k-prefixed names for 1000x tokens (kPEPE); keep case for those.
    let c = if base.starts_with('k') && base.len() > 1 && base[1..].chars().all(|c| c.is_ascii_uppercase()) {
        base.to_string()
    } else {
        base.to_uppercase()
    };
    (c, None)
}

fn interval(tf: Timeframe) -> Result<&'static str> {
    Ok(match tf.secs {
        60 => "1m",
        180 => "3m",
        300 => "5m",
        900 => "15m",
        1800 => "30m",
        3600 => "1h",
        7200 => "2h",
        14400 => "4h",
        28800 => "8h",
        43200 => "12h",
        86400 => "1d",
        259200 => "3d",
        604800 => "1w",
        2592000 => "1M",
        _ => bail!("hyperliquid: unsupported timeframe {tf}"),
    })
}

/// (meta entry, asset context) for one coin.
async fn ctx(symbol: &str) -> Result<(Value, Value)> {
    let (c, dex) = coin(symbol);
    let body = match &dex {
        Some(d) => json!({"type": "metaAndAssetCtxs", "dex": d}),
        None => json!({"type": "metaAndAssetCtxs"}),
    };
    let v: Value = post_json(&url(), &body).await?;
    let uni = v[0]["universe"].as_array().cloned().unwrap_or_default();
    let ctxs = v[1].as_array().cloned().unwrap_or_default();
    for (m, cx) in uni.iter().zip(ctxs.iter()) {
        if m["name"].as_str() == Some(c.as_str()) {
            return Ok((m.clone(), cx.clone()));
        }
    }
    bail!("hyperliquid: no market named {c}")
}

#[async_trait]
impl Provider for Hyperliquid {
    fn id(&self) -> &'static str {
        "hyperliquid"
    }
    fn describe(&self) -> &'static str {
        "Hyperliquid perps incl. HIP-3 24/7 equity/commodity perps (real-time, free)"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["funding_rate", "premium", "open_interest"]
    }

    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let (c, _) = coin(symbol);
        let iv = interval(tf)?;
        let end = range.end_ms();
        let start = range.start_ms(tf);
        let mut bars = Vec::new();
        let mut cursor = start;
        for _ in 0..10 {
            let v: Value =
                post_json(&url(), &json!({"type":"candleSnapshot","req":{"coin":c,"interval":iv,"startTime":cursor,"endTime":end}})).await?;
            let arr = v.as_array().cloned().unwrap_or_default();
            if arr.is_empty() {
                break;
            }
            let now = now_ms();
            for x in &arr {
                let close_t = x["T"].as_i64().unwrap_or(0);
                bars.push(Bar {
                    timestamp: x["t"].as_i64().unwrap_or(0),
                    open: num(&x["o"]).unwrap_or(0.0),
                    high: num(&x["h"]).unwrap_or(0.0),
                    low: num(&x["l"]).unwrap_or(0.0),
                    close: num(&x["c"]).unwrap_or(0.0),
                    volume: num(&x["v"]).unwrap_or(0.0),
                    closed: close_t < now,
                });
            }
            let last = arr.last().and_then(|x| x["t"].as_i64()).unwrap_or(end);
            if arr.len() < 5000 || last >= end {
                break;
            }
            cursor = last + tf.ms();
        }
        Ok(finish(bars, range))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let (_, cx) = ctx(symbol).await?;
        let last = num(&cx["midPx"]).or(num(&cx["markPx"])).unwrap_or(0.0);
        let (bid, ask) = match cx["impactPxs"].as_array() {
            Some(a) if a.len() == 2 => (num(&a[0]), num(&a[1])),
            _ => (None, None),
        };
        Ok(Ticker {
            symbol: symbol.into(),
            timestamp: now_ms(),
            last,
            bid,
            ask,
            open: num(&cx["prevDayPx"]),
            base_volume: num(&cx["dayBaseVlm"]),
            quote_volume: num(&cx["dayNtlVlm"]),
            ..Default::default()
        })
    }

    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let (c, _) = coin(symbol);
        match metric {
            "funding_rate" | "funding" | "premium" => {
                let field = if metric == "premium" { "premium" } else { "fundingRate" };
                // Hourly funding; walk forward 500 at a time.
                let start = range.start.unwrap_or_else(|| range.end_ms() - 3_600_000 * range.limit.max(1) as i64);
                let mut pts = Vec::new();
                let mut cursor = start;
                for _ in 0..20 {
                    let v: Value = post_json(&url(), &json!({"type":"fundingHistory","coin":c,"startTime":cursor})).await?;
                    let arr = v.as_array().cloned().unwrap_or_default();
                    if arr.is_empty() {
                        break;
                    }
                    for x in &arr {
                        if let (Some(t), Some(val)) = (x["time"].as_i64(), num(&x[field])) {
                            pts.push(MetricPoint { timestamp: t, value: val });
                        }
                    }
                    let last = arr.last().and_then(|x| x["time"].as_i64()).unwrap_or(i64::MAX);
                    if arr.len() < 500 || last >= range.end_ms() {
                        break;
                    }
                    cursor = last + 1;
                }
                let pts = if tf.secs > 3600 { downsample_last(pts, tf) } else { pts };
                Ok(finish_metric(pts, range))
            }
            "open_interest" | "oi" => {
                // Only the current value is public; the daemon records history over time.
                let (_, cx) = ctx(symbol).await?;
                let oi = num(&cx["openInterest"]).unwrap_or(0.0) * num(&cx["markPx"]).unwrap_or(0.0);
                Ok(vec![MetricPoint { timestamp: now_ms(), value: oi }])
            }
            _ => bail!("hyperliquid: unknown metric '{metric}' (supported: {})", self.metrics().join(", ")),
        }
    }

    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        let mut out = Vec::new();
        let dexes: Value = post_json(&url(), &json!({"type":"perpDexs"})).await?;
        let mut names: Vec<Option<String>> = vec![None];
        for d in dexes.as_array().cloned().unwrap_or_default() {
            if let Some(n) = d["name"].as_str() {
                names.push(Some(n.to_string()));
            }
        }
        for dex in names {
            let body = match &dex {
                Some(d) => json!({"type":"meta","dex":d}),
                None => json!({"type":"meta"}),
            };
            let v: Value = match post_json(&url(), &body).await {
                Ok(v) => v,
                Err(_) => continue,
            };
            for m in v["universe"].as_array().cloned().unwrap_or_default() {
                if m["isDelisted"].as_bool() == Some(true) {
                    continue;
                }
                let name = m["name"].as_str().unwrap_or("");
                let id = if dex.is_some() { format!("hyperliquid:{name}") } else { format!("hyperliquid:{name}/USDC:USDC") };
                if let Some(f) = filter {
                    if !id.to_uppercase().contains(&f.to_uppercase()) {
                        continue;
                    }
                }
                let note = dex.as_ref().map(|d| format!("HIP-3 dex {d}"));
                out.push(MarketInfo { id, kind: "swap".into(), note });
            }
        }
        Ok(out)
    }
}

fn downsample_last(pts: Vec<MetricPoint>, tf: Timeframe) -> Vec<MetricPoint> {
    let mut out: Vec<MetricPoint> = Vec::new();
    for p in pts {
        let b = p.timestamp - p.timestamp.rem_euclid(tf.ms());
        match out.last_mut() {
            Some(l) if l.timestamp == b => l.value += p.value,
            _ => out.push(MetricPoint { timestamp: b, value: p.value }),
        }
    }
    out
}

/// All markets of one HIP-3 dex with their contexts: (name, ctx).
pub async fn dex_markets(dex: &str) -> Result<Vec<(String, Value)>> {
    let v: Value = post_json(&url(), &json!({"type": "metaAndAssetCtxs", "dex": dex})).await?;
    let uni = v[0]["universe"].as_array().cloned().unwrap_or_default();
    let ctxs = v[1].as_array().cloned().unwrap_or_default();
    Ok(uni
        .iter()
        .zip(ctxs.iter())
        .filter(|(m, _)| m["isDelisted"].as_bool() != Some(true))
        .map(|(m, c)| (m["name"].as_str().unwrap_or("").to_string(), c.clone()))
        .collect())
}

/// Names of every HIP-3 dex.
pub async fn dexes() -> Result<Vec<String>> {
    let v: Value = post_json(&url(), &json!({"type":"perpDexs"})).await?;
    Ok(v.as_array().cloned().unwrap_or_default().iter().filter_map(|d| d["name"].as_str().map(|s| s.to_string())).collect())
}

/// Pre-IPO style markets across all HIP-3 dexes (matched by name).
pub async fn preipo() -> Result<Vec<(String, Value)>> {
    const NAMES: &[&str] = &["ANTHROPIC", "ANTH", "ANT", "OPENAI", "OAI", "XAI", "STRIPE", "DATABRICKS", "MOONSHOT", "PERPLEXITY", "CURSOR", "ANDURIL", "KALSHI", "POLYMARKET", "DISCORD", "REVOLUT", "CEREBRAS", "SPACEX", "FIGURE", "RAMP", "MISTRAL", "SSI", "SCALE"];
    let ds = dexes().await?;
    let futs = ds.iter().map(|d| dex_markets(d));
    let mut out = Vec::new();
    for r in futures::future::join_all(futs).await.into_iter().flatten() {
        for (name, c) in r {
            let short = name.split(':').nth(1).unwrap_or(&name);
            if NAMES.contains(&short) {
                out.push((name, c));
            }
        }
    }
    Ok(out)
}
