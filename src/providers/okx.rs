//! OKX: spot, perpetual swaps, dated futures; funding, open interest, long/short and taker flow.

use super::*;
use crate::http::{get_json, num};
use crate::ids::Ccxt;
use serde_json::Value;

pub struct Okx;

fn base() -> String {
    crate::config::base_url("okx", "https://www.okx.com")
}

/// CCXT symbol → OKX instId.
pub fn inst_id(symbol: &str) -> Result<String> {
    let c = Ccxt::parse(symbol)?;
    Ok(match (&c.settle, &c.expiry) {
        (None, _) => format!("{}-{}", c.base, c.quote),
        (Some(_), None) => format!("{}-{}-SWAP", c.base, c.quote),
        (Some(_), Some(e)) => format!("{}-{}-{}", c.base, c.quote, e),
    })
}

fn bar_str(tf: Timeframe) -> Result<&'static str> {
    Ok(match tf.secs {
        60 => "1m",
        180 => "3m",
        300 => "5m",
        900 => "15m",
        1800 => "30m",
        3600 => "1H",
        7200 => "2H",
        14400 => "4H",
        21600 => "6Hutc",
        43200 => "12Hutc",
        86400 => "1Dutc",
        604800 => "1Wutc",
        2592000 => "1Mutc",
        _ => bail!("okx: unsupported timeframe {tf}"),
    })
}

fn rubik_period(tf: Timeframe) -> &'static str {
    match tf.secs {
        s if s <= 300 => "5m",
        s if s < 86400 => "1H",
        _ => "1D",
    }
}

async fn data(path: &str) -> Result<Vec<Value>> {
    let url = format!("{}{}", base(), path);
    let v: Value = get_json(&url).await?;
    if v["code"].as_str().unwrap_or("0") != "0" {
        bail!("okx: {} ({})", v["msg"].as_str().unwrap_or("error"), path);
    }
    Ok(v["data"].as_array().cloned().unwrap_or_default())
}

fn parse_candles(rows: &[Value], swap: bool) -> Vec<Bar> {
    rows.iter()
        .filter_map(|r| {
            let a = r.as_array()?;
            Some(Bar {
                timestamp: num(&a[0])? as i64,
                open: num(&a[1])?,
                high: num(&a[2])?,
                low: num(&a[3])?,
                close: num(&a[4])?,
                volume: if swap { num(&a[6])? } else { num(&a[5])? },
                closed: a.get(8).and_then(|x| x.as_str()) != Some("0"),
            })
        })
        .collect()
}

fn ccy(symbol: &str) -> Result<String> {
    Ok(Ccxt::parse(symbol)?.base)
}

#[async_trait]
impl Provider for Okx {
    fn id(&self) -> &'static str {
        "okx"
    }
    fn describe(&self) -> &'static str {
        "OKX spot, perpetual swaps and futures (real-time, free)"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["funding_rate", "open_interest", "long_short_ratio", "top_long_short_ratio", "taker_buy_sell_ratio", "basis", "liquidations", "liquidations_long", "liquidations_short"]
    }

    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let inst = inst_id(symbol)?;
        let swap = !Ccxt::parse(symbol)?.is_spot();
        let bar = bar_str(tf)?;
        let start = range.start_ms(tf);
        let mut bars: Vec<Bar> = Vec::new();
        // Recent page (up to 300), then older pages via history-candles.
        let mut path = format!("/api/v5/market/candles?instId={inst}&bar={bar}&limit=300");
        if let Some(e) = range.end {
            path.push_str(&format!("&after={}", e + 1));
        }
        let rows = data(&path).await?;
        bars.extend(parse_candles(&rows, swap));
        let mut guard = 0;
        while bars.iter().map(|b| b.timestamp).min().map_or(false, |m| m > start) && bars.len() < range.limit + 2 && guard < 60 {
            guard += 1;
            let oldest = bars.iter().map(|b| b.timestamp).min().unwrap();
            let rows = data(&format!("/api/v5/market/history-candles?instId={inst}&bar={bar}&limit=100&after={oldest}")).await?;
            if rows.is_empty() {
                break;
            }
            bars.extend(parse_candles(&rows, swap));
        }
        Ok(finish(bars, range))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let inst = inst_id(symbol)?;
        let rows = data(&format!("/api/v5/market/ticker?instId={inst}")).await?;
        let t = rows.first().ok_or_else(|| anyhow::anyhow!("okx: no ticker for {inst}"))?;
        let swap = !Ccxt::parse(symbol)?.is_spot();
        let last = num(&t["last"]).unwrap_or(0.0);
        let base_vol = if swap { num(&t["volCcy24h"]) } else { num(&t["vol24h"]) };
        Ok(Ticker {
            symbol: symbol.into(),
            timestamp: num(&t["ts"]).unwrap_or(0.0) as i64,
            last,
            bid: num(&t["bidPx"]),
            ask: num(&t["askPx"]),
            high: num(&t["high24h"]),
            low: num(&t["low24h"]),
            open: num(&t["open24h"]),
            base_volume: base_vol,
            quote_volume: base_vol.map(|v| v * last),
            ..Default::default()
        })
    }

    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let inst = inst_id(symbol)?;
        let period = rubik_period(tf);
        let pairs = |rows: Vec<Value>, idx: usize| -> Vec<MetricPoint> {
            rows.iter()
                .filter_map(|r| {
                    let a = r.as_array()?;
                    Some(MetricPoint { timestamp: num(&a[0])? as i64, value: num(a.get(idx)?)? })
                })
                .collect()
        };
        let pts = match metric {
            "funding_rate" | "funding" => {
                let start = range.start_ms(Timeframe { secs: 8 * 3600 });
                let mut pts: Vec<MetricPoint> = Vec::new();
                let mut after: Option<i64> = range.end;
                for _ in 0..20 {
                    let mut p = format!("/api/v5/public/funding-rate-history?instId={inst}&limit=100");
                    if let Some(a) = after {
                        p.push_str(&format!("&after={a}"));
                    }
                    let rows = data(&p).await?;
                    if rows.is_empty() {
                        break;
                    }
                    for r in &rows {
                        if let (Some(t), Some(v)) = (num(&r["fundingTime"]), num(&r["realizedRate"]).or(num(&r["fundingRate"]))) {
                            pts.push(MetricPoint { timestamp: t as i64, value: v });
                        }
                    }
                    let oldest = pts.iter().map(|p| p.timestamp).min().unwrap_or(0);
                    if oldest <= start || pts.len() >= range.limit {
                        break;
                    }
                    after = Some(oldest);
                }
                // Append the current (predicted) rate as the forming point.
                if range.end.is_none() {
                    if let Ok(cur) = data(&format!("/api/v5/public/funding-rate?instId={inst}")).await {
                        if let Some(c) = cur.first() {
                            if let (Some(t), Some(v)) = (num(&c["fundingTime"]), num(&c["fundingRate"])) {
                                pts.push(MetricPoint { timestamp: t as i64, value: v });
                            }
                        }
                    }
                }
                pts
            }
            "open_interest" | "oi" => {
                // USD notional for this contract.
                let rows = data(&format!("/api/v5/rubik/stat/contracts/open-interest-history?instId={inst}&period={period}&limit=100")).await?;
                pairs(rows, 3)
            }
            "long_short_ratio" | "lsr" => {
                let rows = data(&format!("/api/v5/rubik/stat/contracts/long-short-account-ratio?ccy={}&period={period}", ccy(symbol)?)).await?;
                pairs(rows, 1)
            }
            "top_long_short_ratio" => {
                let rows = data(&format!(
                    "/api/v5/rubik/stat/contracts/long-short-account-ratio-contract-top-trader?instId={inst}&period={period}&limit=100"
                ))
                .await?;
                pairs(rows, 1)
            }
            "taker_buy_sell_ratio" => {
                let rows = data(&format!("/api/v5/rubik/stat/taker-volume?ccy={}&instType=CONTRACTS&period={period}", ccy(symbol)?)).await?;
                rows.iter()
                    .filter_map(|r| {
                        let a = r.as_array()?;
                        let sell = num(&a[1])?;
                        let buy = num(&a[2])?;
                        let ts = num(&a[0])? as i64;
                        (sell > 0.0).then(|| MetricPoint { timestamp: ts, value: buy / sell })
                    })
                    .collect()
            }
            "basis" => {
                // Perp premium over spot, in %, from matching candles.
                let c = Ccxt::parse(symbol)?;
                let spot_sym = format!("{}/{}", c.base, c.quote);
                let perp = self.fetch_ohlcv(symbol, tf, range).await?;
                let spot = self.fetch_ohlcv(&spot_sym, tf, range).await?;
                let sm: std::collections::HashMap<i64, f64> = spot.iter().map(|b| (b.timestamp, b.close)).collect();
                perp.iter()
                    .filter_map(|b| sm.get(&b.timestamp).map(|s| MetricPoint { timestamp: b.timestamp, value: (b.close / s - 1.0) * 100.0 }))
                    .collect()
            }
            "liquidations" | "liquidations_long" | "liquidations_short" => {
                // Collected by the daemon (OKX keeps only the latest orders publicly).
                let c = crate::store::open()?;
                let start = range.start_ms(tf);
                let end = range.end_ms();
                let sides: Vec<&str> = match metric {
                    "liquidations_long" => vec!["long"],
                    "liquidations_short" => vec!["short"],
                    _ => vec!["long", "short"],
                };
                let mut agg: std::collections::BTreeMap<i64, f64> = Default::default();
                // Pre-fill buckets with zero so quiet periods read as 0, not missing.
                let mut t = start - start.rem_euclid(tf.ms());
                while t <= end {
                    agg.insert(t, 0.0);
                    t += tf.ms();
                }
                let mut any = false;
                for sd in sides {
                    for p in crate::store::points(&c, &format!("okx:{symbol}#liquidations_{sd}"), start, end)? {
                        any = true;
                        *agg.entry(p.timestamp - p.timestamp.rem_euclid(tf.ms())).or_default() += p.value;
                    }
                }
                if !any && crate::store::series_list(&c)?.iter().all(|s| !s.0.contains("liquidations")) {
                    bail!("okx: liquidations are collected by the daemon; start it (`autorekt daemon start`) and data accrues from then on");
                }
                agg.into_iter().map(|(t, v)| MetricPoint { timestamp: t, value: v }).collect()
            }
            _ => bail!("okx: unknown metric '{metric}' (supported: {})", self.metrics().join(", ")),
        };
        Ok(finish_metric(pts, range))
    }

    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        let mut out = Vec::new();
        for (kind, label) in [("SWAP", "swap"), ("SPOT", "spot"), ("FUTURES", "future")] {
            let rows = data(&format!("/api/v5/public/instruments?instType={kind}")).await?;
            for r in rows {
                let inst = r["instId"].as_str().unwrap_or("");
                if r["state"].as_str() != Some("live") && r["state"].as_str() != Some("preopen") {
                    continue;
                }
                let parts: Vec<&str> = inst.split('-').collect();
                let id = match (kind, parts.as_slice()) {
                    ("SPOT", [b, q]) => format!("okx:{b}/{q}"),
                    ("SWAP", [b, q, _]) => format!("okx:{b}/{q}:{}", if *q == "USD" { *b } else { *q }),
                    ("FUTURES", [b, q, e]) => format!("okx:{b}/{q}:{}-{e}", if *q == "USD" { *b } else { *q }),
                    _ => continue,
                };
                if let Some(f) = filter {
                    if !id.to_uppercase().contains(&f.to_uppercase()) {
                        continue;
                    }
                }
                let note = (r["state"].as_str() == Some("preopen")).then(|| "pre-open".to_string());
                out.push(MarketInfo { id, kind: label.into(), note });
            }
        }
        Ok(out)
    }
}

/// Pre-market (pre-launch / pre-IPO) perpetuals listed on OKX.
pub async fn premarket() -> Result<Vec<(String, Value)>> {
    let rows = data("/api/v5/public/instruments?instType=SWAP").await?;
    let ids: Vec<String> = rows
        .iter()
        .filter(|r| r["ruleType"].as_str() == Some("pre_market") && r["state"].as_str() != Some("suspend"))
        .filter_map(|r| r["instId"].as_str().map(|s| s.to_string()))
        .collect();
    let tickers = data("/api/v5/market/tickers?instType=SWAP").await?;
    Ok(ids
        .into_iter()
        .filter_map(|i| tickers.iter().find(|t| t["instId"].as_str() == Some(&i)).map(|t| (i.clone(), t.clone())))
        .collect())
}
