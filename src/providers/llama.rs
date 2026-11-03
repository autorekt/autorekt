//! DefiLlama stablecoin supply (daily): `llama:stablecoins` (all USD-pegged, USD) or a
//! single coin by symbol, `llama:USDT`, `llama:USDC`. Growing supply = fresh dollars entering
//! crypto; shrinking = redemptions (liquidity leaving).

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Llama;
const BASE: &str = "https://stablecoins.llama.fi";

async fn coin_id(symbol: &str) -> Result<String> {
    let v: Value = get_json(&format!("{BASE}/stablecoins?includePrices=false")).await?;
    v["peggedAssets"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|x| x["symbol"].as_str().map_or(false, |s| s.eq_ignore_ascii_case(symbol)))
        .max_by(|a, b| num(&a["circulating"]["peggedUSD"]).unwrap_or(0.0).partial_cmp(&num(&b["circulating"]["peggedUSD"]).unwrap_or(0.0)).unwrap())
        .and_then(|x| x["id"].as_str().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("llama: unknown stablecoin '{symbol}'"))
}

async fn series(symbol: &str) -> Result<Vec<MetricPoint>> {
    let url = if symbol.eq_ignore_ascii_case("stablecoins") || symbol.eq_ignore_ascii_case("all") {
        format!("{BASE}/stablecoincharts/all")
    } else {
        format!("{BASE}/stablecoincharts/all?stablecoin={}", coin_id(symbol).await?)
    };
    let v: Vec<Value> = get_json(&url).await?;
    Ok(v.iter()
        .filter_map(|x| Some(MetricPoint { timestamp: num(&x["date"])? as i64 * 1000, value: num(&x["totalCirculatingUSD"]["peggedUSD"]).or(num(&x["totalCirculating"]["peggedUSD"]))? }))
        .collect())
}

#[async_trait]
impl Provider for Llama {
    fn id(&self) -> &'static str {
        "llama"
    }
    fn describe(&self) -> &'static str {
        "DefiLlama stablecoin supply (daily): llama:stablecoins, llama:USDT, llama:USDC"
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["daily, USD".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let pts = series(symbol).await?;
        let mut bars: Vec<Bar> = pts.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect();
        if tf.secs > 86400 {
            bars = resample(&bars, tf);
        }
        let r = Range { start: range.start.or_else(|| Some(range.end_ms() - tf.secs.max(86400) * 1000 * range.limit as i64)), ..range };
        Ok(finish(bars, r))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let pts = series(symbol).await?;
        let last = pts.last().ok_or_else(|| anyhow::anyhow!("llama: no data"))?;
        let prev = pts.len().checked_sub(2).map(|i| pts[i].value);
        Ok(Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.value, open: prev, ..Default::default() })
    }
}
