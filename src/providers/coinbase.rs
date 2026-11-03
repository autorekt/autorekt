//! Coinbase Exchange spot (USD pairs): `coinbase:BTC/USD`. Useful for the "Coinbase premium"
//! (US spot demand vs offshore): `(coinbase:BTC/USD / okx:BTC/USDT - 1) * 10000` in bp.

use super::*;
use crate::http::{get_json, num};
use crate::ids::Ccxt;
use serde_json::Value;

pub struct Coinbase;
const BASE: &str = "https://api.exchange.coinbase.com";

fn product(symbol: &str) -> Result<String> {
    let c = Ccxt::parse(symbol)?;
    if !c.is_spot() {
        bail!("coinbase: spot only (e.g. coinbase:BTC/USD)");
    }
    Ok(format!("{}-{}", c.base, c.quote))
}

#[async_trait]
impl Provider for Coinbase {
    fn id(&self) -> &'static str {
        "coinbase"
    }
    fn describe(&self) -> &'static str {
        "Coinbase Exchange spot (USD pairs; Coinbase premium vs offshore)"
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let p = product(symbol)?;
        let (gran, resample_to) = match tf.secs {
            60 | 300 | 900 | 3600 | 21600 | 86400 => (tf.secs, None),
            s if s < 3600 => (60, Some(tf)),
            s if s < 86400 => (3600, Some(tf)),
            _ => (86400, Some(tf)),
        };
        let end = range.end_ms();
        let start = range.start_ms(tf);
        let mut bars = Vec::new();
        let mut cur_end = end;
        for _ in 0..30 {
            let cur_start = (cur_end - gran * 1000 * 299).max(start);
            let iso = |ms: i64| chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().to_rfc3339();
            let v: Vec<Value> = get_json(&format!("{BASE}/products/{p}/candles?granularity={gran}&start={}&end={}", iso(cur_start), iso(cur_end))).await?;
            let now = now_ms();
            for k in &v {
                let t = k[0].as_i64().unwrap_or(0) * 1000;
                bars.push(Bar { timestamp: t, low: num(&k[1]).unwrap_or(0.0), high: num(&k[2]).unwrap_or(0.0), open: num(&k[3]).unwrap_or(0.0), close: num(&k[4]).unwrap_or(0.0), volume: num(&k[5]).unwrap_or(0.0), closed: t + gran * 1000 <= now });
            }
            if cur_start <= start || v.is_empty() {
                break;
            }
            cur_end = cur_start;
        }
        bars.sort_by_key(|b| b.timestamp);
        let bars = match resample_to {
            Some(t) => resample(&bars, t),
            None => bars,
        };
        Ok(finish(bars, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let p = product(symbol)?;
        let (tu, su) = (format!("{BASE}/products/{p}/ticker"), format!("{BASE}/products/{p}/stats"));
        let (t, s): (Value, Value) = tokio::try_join!(get_json(&tu), get_json(&su))?;
        let last = num(&t["price"]).unwrap_or(f64::NAN);
        Ok(Ticker {
            symbol: symbol.into(),
            timestamp: now_ms(),
            last,
            bid: num(&t["bid"]),
            ask: num(&t["ask"]),
            high: num(&s["high"]),
            low: num(&s["low"]),
            open: num(&s["open"]),
            base_volume: num(&s["volume"]),
            quote_volume: num(&s["volume"]).map(|v| v * last),
            ..Default::default()
        })
    }
}
