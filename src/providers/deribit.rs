//! Deribit (crypto options & futures). `deribit:BTC-DVOL` / `deribit:ETH-DVOL` are the 30-day
//! implied volatility indices (crypto's VIX). Instruments use Deribit names
//! (`deribit:BTC-PERPETUAL`). Option-market metrics on `deribit:BTC` / `deribit:ETH`:
//! `#put_call_oi`, `#put_call_volume`, `#skew_10pct` (current values; record them for history).

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Deribit;

const BASE: &str = "https://www.deribit.com/api/v2/public";

async fn result(path: &str) -> Result<Value> {
    let v: Value = get_json(&format!("{BASE}/{path}")).await?;
    if let Some(e) = v.get("error") {
        bail!("deribit: {}", e["message"].as_str().unwrap_or("error"));
    }
    Ok(v["result"].clone())
}

fn resolution(tf: Timeframe) -> Result<String> {
    Ok(match tf.secs {
        60 | 180 | 300 | 600 | 900 | 1800 | 3600 | 7200 | 10800 | 21600 | 43200 => (tf.secs / 60).to_string(),
        86400 => "1D".into(),
        14400 => "60".into(), // resampled below
        _ => bail!("deribit: unsupported timeframe {tf}"),
    })
}

async fn option_summary(ccy: &str) -> Result<Vec<Value>> {
    Ok(result(&format!("get_book_summary_by_currency?currency={ccy}&kind=option")).await?.as_array().cloned().unwrap_or_default())
}

/// Parse "BTC-13NOV26-83000-P" → (expiry ms, strike, is_put)
fn parse_opt(name: &str) -> Option<(i64, f64, bool)> {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() != 4 {
        return None;
    }
    let d = chrono::NaiveDate::parse_from_str(&format!("{:0>7}", parts[1]), "%d%b%y").ok()?;
    let exp = d.and_hms_opt(8, 0, 0)?.and_utc().timestamp_millis();
    Some((exp, parts[2].parse().ok()?, parts[3] == "P"))
}

#[async_trait]
impl Provider for Deribit {
    fn id(&self) -> &'static str {
        "deribit"
    }
    fn describe(&self) -> &'static str {
        "Deribit: DVOL implied-vol indices (crypto VIX), options put/call and skew, futures"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["put_call_oi", "put_call_volume", "skew_10pct", "atm_iv"]
    }

    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let start = range.start_ms(tf);
        let end = range.end_ms();
        if let Some(ccy) = symbol.strip_suffix("-DVOL") {
            let res = if tf.secs >= 86400 { "1D".to_string() } else { (tf.secs.clamp(60, 43200)).to_string() };
            let mut bars = Vec::new();
            let mut e = end;
            for _ in 0..10 {
                let r = result(&format!("get_volatility_index_data?currency={ccy}&start_timestamp={start}&end_timestamp={e}&resolution={res}")).await?;
                let rows = r["data"].as_array().cloned().unwrap_or_default();
                for x in &rows {
                    bars.push(Bar { timestamp: x[0].as_i64().unwrap_or(0), open: num(&x[1]).unwrap_or(0.0), high: num(&x[2]).unwrap_or(0.0), low: num(&x[3]).unwrap_or(0.0), close: num(&x[4]).unwrap_or(0.0), volume: 0.0, closed: true });
                }
                match r["continuation"].as_i64() {
                    Some(c) if c > start && !rows.is_empty() => e = c,
                    _ => break,
                }
            }
            let bars = if tf.secs > 3600 && tf.secs < 86400 { resample(&bars, tf) } else { bars };
            return Ok(finish(bars, range));
        }
        let res = resolution(tf)?;
        let r = result(&format!("get_tradingview_chart_data?instrument_name={symbol}&start_timestamp={start}&end_timestamp={end}&resolution={res}")).await?;
        let ts = r["ticks"].as_array().cloned().unwrap_or_default();
        let g = |k: &str, i: usize| num(&r[k][i]).unwrap_or(f64::NAN);
        let mut bars: Vec<Bar> = (0..ts.len())
            .map(|i| Bar { timestamp: ts[i].as_i64().unwrap_or(0), open: g("open", i), high: g("high", i), low: g("low", i), close: g("close", i), volume: g("volume", i), closed: ts[i].as_i64().unwrap_or(0) + tf.ms() <= now_ms() })
            .collect();
        if tf.secs == 14400 {
            bars = resample(&bars, tf);
        }
        Ok(finish(bars, range))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        if symbol.ends_with("-DVOL") {
            let bars = self.fetch_ohlcv(symbol, Timeframe::H1, Range::last(30)).await?;
            let last = bars.last().ok_or_else(|| anyhow::anyhow!("deribit: no DVOL data"))?;
            let open = bars.iter().find(|b| b.timestamp >= last.timestamp - 86_400_000).map(|b| b.open);
            return Ok(Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.close, open, ..Default::default() });
        }
        let t = result(&format!("ticker?instrument_name={symbol}")).await?;
        let last = num(&t["last_price"]).unwrap_or(f64::NAN);
        let pc = num(&t["stats"]["price_change"]);
        Ok(Ticker {
            symbol: symbol.into(),
            timestamp: t["timestamp"].as_i64().unwrap_or(0),
            last,
            bid: num(&t["best_bid_price"]),
            ask: num(&t["best_ask_price"]),
            high: num(&t["stats"]["high"]),
            low: num(&t["stats"]["low"]),
            open: pc.map(|p| last / (1.0 + p / 100.0)),
            quote_volume: num(&t["stats"]["volume_usd"]),
            ..Default::default()
        })
    }

    async fn fetch_metric(&self, symbol: &str, metric: &str, _tf: Timeframe, _range: Range) -> Result<Vec<MetricPoint>> {
        let ccy = symbol.split('-').next().unwrap_or(symbol).to_uppercase();
        let opts = option_summary(&ccy).await?;
        let now = now_ms();
        let value = match metric {
            "put_call_oi" | "put_call_volume" => {
                let key = if metric == "put_call_oi" { "open_interest" } else { "volume" };
                let (mut p, mut c) = (0.0, 0.0);
                for o in &opts {
                    let n = o["instrument_name"].as_str().unwrap_or("");
                    let v = num(&o[key]).unwrap_or(0.0);
                    if n.ends_with("-P") {
                        p += v;
                    } else if n.ends_with("-C") {
                        c += v;
                    }
                }
                if c == 0.0 {
                    bail!("deribit: no call {key}");
                }
                p / c
            }
            "skew_10pct" | "atm_iv" => {
                // Expiry closest to 30 days; IV of the strikes nearest 90% (put) and 110% (call) of spot.
                let mut by_exp: std::collections::BTreeMap<i64, Vec<(f64, bool, f64, f64)>> = Default::default();
                for o in &opts {
                    let Some((exp, k, put)) = parse_opt(o["instrument_name"].as_str().unwrap_or("")) else { continue };
                    let (Some(iv), Some(u)) = (num(&o["mark_iv"]), num(&o["underlying_price"])) else { continue };
                    by_exp.entry(exp).or_default().push((k, put, iv, u));
                }
                let target = now + 30 * 86_400_000;
                let (_, chain) = by_exp.iter().filter(|(e, _)| **e > now + 7 * 86_400_000).min_by_key(|(e, _)| (**e - target).abs()).ok_or_else(|| anyhow::anyhow!("deribit: no suitable expiry"))?;
                let spot = chain[0].3;
                let near = |m: f64, want_put: bool| -> Option<f64> {
                    chain.iter().filter(|x| x.1 == want_put).min_by(|a, b| ((a.0 / spot - m).abs()).partial_cmp(&(b.0 / spot - m).abs()).unwrap()).map(|x| x.2)
                };
                if metric == "atm_iv" {
                    near(1.0, false).ok_or_else(|| anyhow::anyhow!("deribit: no ATM call"))?
                } else {
                    near(0.9, true).zip(near(1.1, false)).map(|(p, c)| p - c).ok_or_else(|| anyhow::anyhow!("deribit: no wings"))?
                }
            }
            _ => bail!("deribit: unknown metric '{metric}' (supported: {})", self.metrics().join(", ")),
        };
        Ok(vec![MetricPoint { timestamp: now, value }])
    }
}
