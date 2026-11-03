//! Sentiment indices: `alt:fng` (alternative.me crypto Fear & Greed, daily) and
//! `alt:cnn_fng` (CNN Fear & Greed for US stocks, unofficial endpoint) with its seven
//! components as metrics: `alt:cnn_fng#put_call_options`, `#junk_bond_demand`, ...

use super::*;
use crate::http::{client, get_json, num, BROWSER_UA};
use serde_json::Value;

pub struct Alt;

const CNN_COMPONENTS: &[&str] = &[
    "market_momentum_sp500",
    "stock_price_strength",
    "stock_price_breadth",
    "put_call_options",
    "market_volatility_vix",
    "junk_bond_demand",
    "safe_haven_demand",
];

async fn cnn(start_ms: i64) -> Result<Value> {
    let day = chrono::DateTime::from_timestamp_millis(start_ms).unwrap_or_default().format("%Y-%m-%d");
    let url = format!("https://production.dataviz.cnn.io/index/fearandgreed/graphdata/{day}");
    let resp = client()
        .get(&url)
        .header("User-Agent", BROWSER_UA)
        .header("Referer", "https://www.cnn.com/")
        .header("Origin", "https://www.cnn.com")
        .header("Accept", "application/json, text/plain, */*")
        .send()
        .await?;
    if !resp.status().is_success() {
        bail!("cnn fear&greed: HTTP {}", resp.status());
    }
    Ok(resp.json().await?)
}

fn xy(v: &Value) -> Vec<MetricPoint> {
    v["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| Some(MetricPoint { timestamp: num(&p["x"])? as i64, value: num(&p["y"])? }))
        .collect()
}

pub fn fng_label(v: f64) -> &'static str {
    match v as i64 {
        0..=24 => "extreme fear",
        25..=44 => "fear",
        45..=55 => "neutral",
        56..=75 => "greed",
        _ => "extreme greed",
    }
}

impl Alt {
    async fn series(&self, symbol: &str, range: Range) -> Result<Vec<MetricPoint>> {
        let days = ((range.end_ms() - range.start_ms(Timeframe::D1)) / 86_400_000 + 2).max(2);
        match symbol {
            "fng" => {
                let v: Value = get_json(&format!("https://api.alternative.me/fng/?limit={days}")).await?;
                Ok(v["data"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|d| Some(MetricPoint { timestamp: (num(&d["timestamp"])? as i64) * 1000, value: num(&d["value"])? }))
                    .collect())
            }
            "cnn_fng" => Ok(xy(&cnn(range.start_ms(Timeframe::D1)).await?["fear_and_greed_historical"])),
            _ => bail!("alt: unknown series '{symbol}' (fng, cnn_fng)"),
        }
    }
}

#[async_trait]
impl Provider for Alt {
    fn id(&self) -> &'static str {
        "alt"
    }
    fn describe(&self) -> &'static str {
        "Sentiment: alt:fng (crypto Fear & Greed), alt:cnn_fng (CNN Fear & Greed + components)"
    }
    fn metrics(&self) -> &'static [&'static str] {
        CNN_COMPONENTS
    }

    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let r = Range { start: Some(range.start_ms(tf.max_d1())), ..range };
        let pts = self.series(symbol, r).await?;
        Ok(finish(pts.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect(), range))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let pts = self.series(symbol, Range::last(10)).await?;
        let mut sorted = pts.clone();
        sorted.sort_by_key(|p| p.timestamp);
        let last = sorted.last().ok_or_else(|| anyhow::anyhow!("alt: no data"))?;
        let prev = sorted.iter().rev().find(|p| p.timestamp <= last.timestamp - 20 * 3_600_000).map(|p| p.value);
        Ok(Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.value, open: prev, notes: vec![fng_label(last.value).into()], ..Default::default() })
    }

    async fn fetch_metric(&self, symbol: &str, metric: &str, _tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        if symbol != "cnn_fng" || !CNN_COMPONENTS.contains(&metric) {
            bail!("alt: metrics exist only on cnn_fng: {}", CNN_COMPONENTS.join(", "));
        }
        let v = cnn(range.start_ms(Timeframe::D1)).await?;
        Ok(finish_metric(xy(&v[metric]), range))
    }
}

trait MaxD1 {
    fn max_d1(self) -> Timeframe;
}
impl MaxD1 for Timeframe {
    fn max_d1(self) -> Timeframe {
        if self.secs < 86400 {
            Timeframe::D1
        } else {
            self
        }
    }
}
