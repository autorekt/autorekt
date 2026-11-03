//! Binance spot and USDⓈ-M futures. Spot uses the public market-data mirror
//! (data-api.binance.vision), which also works where api.binance.com is geo-restricted.
//! Futures (fapi) are geo-restricted in some regions (e.g. the US): set
//! `base_urls.binance_futures` in config.yaml if you have another route.

use super::*;
use crate::http::{get_json, num};
use crate::ids::Ccxt;
use serde_json::Value;

pub struct Binance;

fn spot() -> String {
    crate::config::base_url("binance_spot", "https://data-api.binance.vision")
}
fn fut() -> String {
    crate::config::base_url("binance_futures", "https://fapi.binance.com")
}

fn market(symbol: &str) -> Result<(String, bool)> {
    let c = Ccxt::parse(symbol)?;
    if c.expiry.is_some() {
        bail!("binance: dated futures are not supported yet");
    }
    if c.settle.as_deref().map_or(false, |s| s != c.quote) {
        bail!("binance: coin-margined futures are not supported yet");
    }
    Ok((format!("{}{}", c.base, c.quote), !c.is_spot()))
}

fn period(tf: Timeframe) -> &'static str {
    match tf.secs {
        s if s <= 300 => "5m",
        s if s <= 900 => "15m",
        s if s <= 1800 => "30m",
        s if s <= 3600 => "1h",
        s if s <= 7200 => "2h",
        s if s <= 14400 => "4h",
        s if s <= 21600 => "6h",
        s if s <= 43200 => "12h",
        _ => "1d",
    }
}

#[async_trait]
impl Provider for Binance {
    fn id(&self) -> &'static str {
        "binance"
    }
    fn describe(&self) -> &'static str {
        "Binance spot (mirror, works everywhere) and USDⓈ-M perps (geo-restricted in the US)"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["funding_rate", "open_interest", "long_short_ratio", "top_long_short_ratio", "taker_buy_sell_ratio"]
    }

    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let (sym, is_fut) = market(symbol)?;
        let iv = tf.to_string();
        let (base, path, max) = if is_fut { (fut(), "/fapi/v1/klines", 1500) } else { (spot(), "/api/v3/klines", 1000) };
        let mut bars = Vec::new();
        let mut cursor = range.start_ms(tf);
        let end = range.end_ms();
        for _ in 0..20 {
            let url = format!("{base}{path}?symbol={sym}&interval={iv}&startTime={cursor}&endTime={end}&limit={max}");
            let v: Vec<Value> = get_json(&url).await?;
            if v.is_empty() {
                break;
            }
            let now = now_ms();
            for k in &v {
                bars.push(Bar {
                    timestamp: k[0].as_i64().unwrap_or(0),
                    open: num(&k[1]).unwrap_or(0.0),
                    high: num(&k[2]).unwrap_or(0.0),
                    low: num(&k[3]).unwrap_or(0.0),
                    close: num(&k[4]).unwrap_or(0.0),
                    volume: num(&k[5]).unwrap_or(0.0),
                    closed: k[6].as_i64().unwrap_or(0) < now,
                });
            }
            let last = v.last().and_then(|k| k[0].as_i64()).unwrap_or(end);
            if v.len() < max || last >= end {
                break;
            }
            cursor = last + tf.ms();
        }
        Ok(finish(bars, range))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let (sym, is_fut) = market(symbol)?;
        let url = if is_fut { format!("{}/fapi/v1/ticker/24hr?symbol={sym}", fut()) } else { format!("{}/api/v3/ticker/24hr?symbol={sym}", spot()) };
        let t: Value = get_json(&url).await?;
        Ok(Ticker {
            symbol: symbol.into(),
            timestamp: t["closeTime"].as_i64().unwrap_or(0),
            last: num(&t["lastPrice"]).unwrap_or(0.0),
            bid: num(&t["bidPrice"]),
            ask: num(&t["askPrice"]),
            high: num(&t["highPrice"]),
            low: num(&t["lowPrice"]),
            open: num(&t["openPrice"]),
            base_volume: num(&t["volume"]),
            quote_volume: num(&t["quoteVolume"]),
            ..Default::default()
        })
    }

    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let (sym, _) = market(symbol)?;
        let p = period(tf);
        let lim = range.limit.min(500);
        let rows = |v: Vec<Value>, tkey: &str, vkey: &str| -> Vec<MetricPoint> {
            v.iter()
                .filter_map(|x| Some(MetricPoint { timestamp: x[tkey].as_i64()?, value: num(&x[vkey])? }))
                .collect()
        };
        let pts = match metric {
            "funding_rate" | "funding" => rows(get_json(&format!("{}/fapi/v1/fundingRate?symbol={sym}&limit=1000", fut())).await?, "fundingTime", "fundingRate"),
            "open_interest" | "oi" => rows(
                get_json(&format!("{}/futures/data/openInterestHist?symbol={sym}&period={p}&limit={lim}", fut())).await?,
                "timestamp",
                "sumOpenInterestValue",
            ),
            "long_short_ratio" | "lsr" => rows(
                get_json(&format!("{}/futures/data/globalLongShortAccountRatio?symbol={sym}&period={p}&limit={lim}", fut())).await?,
                "timestamp",
                "longShortRatio",
            ),
            "top_long_short_ratio" => rows(
                get_json(&format!("{}/futures/data/topLongShortPositionRatio?symbol={sym}&period={p}&limit={lim}", fut())).await?,
                "timestamp",
                "longShortRatio",
            ),
            "taker_buy_sell_ratio" => rows(
                get_json(&format!("{}/futures/data/takerlongshortRatio?symbol={sym}&period={p}&limit={lim}", fut())).await?,
                "timestamp",
                "buySellRatio",
            ),
            _ => bail!("binance: unknown metric '{metric}' (supported: {})", self.metrics().join(", ")),
        };
        Ok(finish_metric(pts, range))
    }
}
