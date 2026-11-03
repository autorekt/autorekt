//! Source adapters. Each maps one provider onto the unified types; method names follow CCXT.

use crate::ids::InstrumentId;
use crate::model::*;
use anyhow::{bail, Result};
use async_trait::async_trait;

pub mod alt;
pub mod binance;
pub mod cftc;
pub mod coinbase;
pub mod deribit;
pub mod finra;
pub mod fred;
pub mod hyperliquid;
pub mod kalshi;
pub mod llama;
pub mod local;
pub mod news;
pub mod okx;
pub mod polymarket;
pub mod yahoo;

/// Range request for history. `end` defaults to now; providers page as needed.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub limit: usize,
}

impl Range {
    pub fn last(limit: usize) -> Range {
        Range { start: None, end: None, limit }
    }
    /// Start time implied by `limit` bars of `tf` back from `end`.
    pub fn start_ms(&self, tf: Timeframe) -> i64 {
        self.start.unwrap_or_else(|| self.end_ms() - tf.ms() * self.limit as i64)
    }
    pub fn end_ms(&self) -> i64 {
        self.end.unwrap_or_else(now_ms)
    }
}

/// Trim bars to the requested range, ascending, at most `limit` (keeping the newest).
pub fn finish(mut bars: Vec<Bar>, r: Range) -> Vec<Bar> {
    bars.sort_by_key(|b| b.timestamp);
    bars.dedup_by_key(|b| b.timestamp);
    if let Some(s) = r.start {
        bars.retain(|b| b.timestamp >= s);
    }
    if let Some(e) = r.end {
        bars.retain(|b| b.timestamp <= e);
    }
    if bars.len() > r.limit {
        bars.drain(..bars.len() - r.limit);
    }
    bars
}

pub fn finish_metric(mut pts: Vec<MetricPoint>, r: Range) -> Vec<MetricPoint> {
    pts.sort_by_key(|p| p.timestamp);
    pts.dedup_by_key(|p| p.timestamp);
    if let Some(s) = r.start {
        pts.retain(|p| p.timestamp >= s);
    }
    if let Some(e) = r.end {
        pts.retain(|p| p.timestamp <= e);
    }
    if pts.len() > r.limit {
        pts.drain(..pts.len() - r.limit);
    }
    pts
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn describe(&self) -> &'static str;
    /// Metric names this provider can serve via `<instrument>#<metric>`.
    fn metrics(&self) -> &'static [&'static str] {
        &[]
    }
    /// Notes shown next to data from this provider (e.g. "delayed").
    fn notes(&self, _symbol: &str) -> Vec<String> {
        vec![]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>>;
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker>;
    async fn fetch_metric(&self, symbol: &str, metric: &str, _tf: Timeframe, _range: Range) -> Result<Vec<MetricPoint>> {
        bail!("{}: metric '{metric}' not supported for {symbol} (supported: {})", self.id(), self.metrics().join(", "))
    }
    /// List symbols (markets) this provider offers, optionally filtered by a substring.
    async fn list_markets(&self, _filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        bail!("{}: market listing not supported", self.id())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MarketInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

pub fn get(provider: &str) -> Result<Box<dyn Provider>> {
    Ok(match provider {
        "okx" => Box::new(okx::Okx),
        "binance" => Box::new(binance::Binance),
        "hyperliquid" => Box::new(hyperliquid::Hyperliquid),
        "yahoo" => Box::new(yahoo::Yahoo),
        "fred" => Box::new(fred::Fred),
        "alt" => Box::new(alt::Alt),
        "polymarket" => Box::new(polymarket::Polymarket),
        "kalshi" => Box::new(kalshi::Kalshi),
        "local" => Box::new(local::Local),
        "deribit" => Box::new(deribit::Deribit),
        "finra" => Box::new(finra::Finra),
        "cftc" => Box::new(cftc::Cftc),
        "coinbase" => Box::new(coinbase::Coinbase),
        "llama" => Box::new(llama::Llama),
        _ => bail!("provider '{provider}' is not implemented yet"),
    })
}

pub fn all() -> Vec<Box<dyn Provider>> {
    ["okx", "binance", "hyperliquid", "deribit", "coinbase", "yahoo", "fred", "cftc", "finra", "llama", "alt", "polymarket", "kalshi", "local"]
        .iter()
        .filter_map(|p| get(p).ok())
        .collect()
}

/// Fetch bars for any id. For a `#metric` id, metric points are returned as flat bars so that
/// every series can be charted and evaluated the same way.
pub async fn ohlcv(id: &InstrumentId, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
    // Short-lived cache for "latest N bars" requests, so rules and boards sharing a series
    // (and daily series re-checked every minute) do not refetch it.
    if range.start.is_none() && range.end.is_none() {
        let ttl = match tf.secs {
            s if s <= 60 => 8_000,
            s if s <= 900 => 20_000,
            s if s < 86400 => 45_000,
            _ => 300_000,
        };
        let key = format!("{id}|{}|{}", tf.secs, range.limit);
        {
            let c = CACHE.lock().unwrap();
            if let Some((t, bars)) = c.get(&key) {
                if now_ms() - t < ttl {
                    return Ok(bars.clone());
                }
            }
        }
        let bars = ohlcv_uncached(id, tf, range).await?;
        let mut c = CACHE.lock().unwrap();
        if c.len() > 500 {
            c.retain(|_, (t, _)| now_ms() - *t < 600_000);
        }
        c.insert(key, (now_ms(), bars.clone()));
        return Ok(bars);
    }
    ohlcv_uncached(id, tf, range).await
}

static CACHE: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, (i64, Vec<Bar>)>>> = std::sync::LazyLock::new(Default::default);

async fn ohlcv_uncached(id: &InstrumentId, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
    let p = get(&id.provider)?;
    match &id.metric {
        None => p.fetch_ohlcv(&id.symbol, tf, range).await,
        Some(m) => {
            let pts = p.fetch_metric(&id.symbol, m, tf, range).await?;
            Ok(pts.into_iter().map(|pt| Bar::point(pt.timestamp, pt.value)).collect())
        }
    }
}

pub async fn ticker(id: &InstrumentId) -> Result<Ticker> {
    let p = get(&id.provider)?;
    match &id.metric {
        None => {
            let mut t = p.fetch_ticker(&id.symbol).await?;
            t.symbol = id.to_string();
            t.notes.extend(p.notes(&id.symbol));
            t.fill_change();
            Ok(t)
        }
        Some(m) => {
            let pts = p.fetch_metric(&id.symbol, m, Timeframe::H1, Range::last(48)).await?;
            let last = pts.last().ok_or_else(|| anyhow::anyhow!("no data for {id}"))?;
            let day_ago = last.timestamp - 86_400_000;
            let open = pts.iter().find(|x| x.timestamp >= day_ago).map(|x| x.value);
            let mut t = Ticker { symbol: id.to_string(), timestamp: last.timestamp, last: last.value, open, ..Default::default() };
            t.fill_change();
            Ok(t)
        }
    }
}

/// Prediction-market sources.
#[async_trait]
pub trait ProbSource: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<ProbMarket>>;
    /// A market, or every market of an event.
    async fn show(&self, id: &str) -> Result<Vec<ProbMarket>>;
    async fn trending(&self, limit: usize) -> Result<Vec<ProbMarket>>;
}

pub fn prob_source(provider: &str) -> Result<Box<dyn ProbSource>> {
    Ok(match provider {
        "polymarket" => Box::new(polymarket::Polymarket),
        "kalshi" => Box::new(kalshi::Kalshi),
        _ => bail!("'{provider}' is not a prediction-market provider (polymarket, kalshi)"),
    })
}
