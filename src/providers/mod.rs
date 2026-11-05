//! Source adapters. Each maps one provider onto the unified types; method names follow CCXT.

use crate::ids::InstrumentId;
use crate::model::*;
use anyhow::{bail, Result};
use async_trait::async_trait;

pub mod alt;
pub mod binance;
pub mod cboe;
pub mod cfets;
pub mod cftc;
pub mod coinbase;
pub mod deribit;
pub mod dtcc;
pub mod eastmoney;
pub mod ecb;
pub mod farside;
pub mod finra;
pub mod fred;
pub mod hkma;
pub mod hyperliquid;
pub mod kalshi;
pub mod llama;
pub mod local;
pub mod mof;
pub mod news;
pub mod nyfed;
pub mod okx;
pub mod polymarket;
pub mod treasury;
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

pub const DAY_MS: i64 = 86_400_000;

/// Midnight UTC of a calendar date, in ms.
pub fn day_ms(d: chrono::NaiveDate) -> i64 {
    d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis()
}

/// Start time a daily or weekly point series needs to cover `limit` bars of `tf`: business
/// days are about 2/3 of calendar days, and a week of slack covers holidays (or, for intraday
/// bars of a daily series, the latest few points).
pub fn lookback(tf: Timeframe, range: Range) -> i64 {
    let span = tf.ms() * range.limit as i64;
    range.start.unwrap_or_else(|| range.end_ms() - if tf.ms() >= DAY_MS { span * 3 / 2 } else { span } - 7 * DAY_MS)
}

/// Bars for a point series (one value per day or week), resampled to coarser timeframes.
pub fn point_bars(pts: Vec<MetricPoint>, tf: Timeframe, range: Range) -> Vec<Bar> {
    let mut bars: Vec<Bar> = finish_metric(pts, Range { limit: usize::MAX, ..range }).into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect();
    if tf.secs > 86400 {
        bars = resample(&bars, tf);
    }
    finish(bars, range)
}

/// Ticker for a point series: the last value against the one before it.
pub fn point_ticker(symbol: &str, mut pts: Vec<MetricPoint>, unit: &str) -> Result<Ticker> {
    pts.sort_by_key(|p| p.timestamp);
    let last = pts.last().ok_or_else(|| anyhow::anyhow!("no recent data for {symbol}"))?;
    let prev = pts.len().checked_sub(2).map(|i| pts[i].value);
    let date = chrono::DateTime::from_timestamp_millis(last.timestamp).unwrap_or_default().format("%Y-%m-%d");
    let mut notes = vec![format!("as of {date}")];
    if !unit.is_empty() {
        notes.push(unit.to_string());
    }
    Ok(Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.value, open: prev, notes, ..Default::default() })
}

/// Static market listing filtered by a case-insensitive substring of id or note.
pub fn list_static(provider: &str, items: &[(&str, &str)], filter: Option<&str>) -> Vec<MarketInfo> {
    let f = filter.map(|s| s.to_lowercase());
    items
        .iter()
        .filter(|(id, note)| f.as_ref().map_or(true, |f| id.to_lowercase().contains(f) || note.to_lowercase().contains(f)))
        .map(|(id, note)| MarketInfo { id: format!("{provider}:{id}"), kind: "series".into(), note: Some(note.to_string()) })
        .collect()
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
        "nyfed" => Box::new(nyfed::Nyfed),
        "treasury" => Box::new(treasury::Treasury),
        "dtcc" => Box::new(dtcc::Dtcc),
        "cboe" => Box::new(cboe::Cboe),
        "cfets" => Box::new(cfets::Cfets),
        "eastmoney" => Box::new(eastmoney::Eastmoney),
        "mof" => Box::new(mof::Mof),
        "ecb" => Box::new(ecb::Ecb),
        "hkma" => Box::new(hkma::Hkma),
        "farside" => Box::new(farside::Farside),
        _ => bail!("provider '{provider}' is not implemented yet"),
    })
}

pub fn all() -> Vec<Box<dyn Provider>> {
    [
        "okx", "binance", "hyperliquid", "deribit", "coinbase", "yahoo", "fred", "nyfed", "treasury", "dtcc", "cboe", "cfets", "eastmoney", "mof", "ecb", "hkma", "cftc",
        "finra", "farside", "llama", "alt", "polymarket", "kalshi", "local",
    ]
        .iter()
        .filter_map(|p| get(p).ok())
        .collect()
}

/// How long a fetch stays fresh enough to answer from the local store.
pub fn ttl_ms(id: &InstrumentId, tf: Timeframe) -> i64 {
    match id.provider.as_str() {
        "okx" | "binance" | "hyperliquid" | "coinbase" | "deribit" => {
            if tf.secs < 3600 {
                10_000
            } else {
                30_000
            }
        }
        "polymarket" | "kalshi" => 60_000,
        "yahoo" if tf.secs < 86400 => 120_000,
        "yahoo" => 600_000,
        _ => 1_800_000,
    }
}

static ISSUES: std::sync::LazyLock<std::sync::Mutex<Vec<String>>> = std::sync::LazyLock::new(Default::default);

/// Record something the reader should know about the data shown (once per message).
pub fn note_issue(msg: String) {
    let mut v = ISSUES.lock().unwrap();
    if !v.contains(&msg) {
        v.push(msg);
    }
}

/// The notes recorded so far, emptied.
pub fn take_issues() -> Vec<String> {
    std::mem::take(&mut *ISSUES.lock().unwrap())
}

fn first_line(e: &anyhow::Error) -> String {
    e.to_string().lines().next().unwrap_or("").to_string()
}

/// Fetch bars for any id. For a `#metric` id, metric points are returned as flat bars so that
/// every series can be charted and evaluated the same way. "Latest N bars" requests go through
/// the local store: a recent enough fetch is answered from it, and when a live fetch fails the
/// stored bars are served with a note saying how old they are.
pub async fn ohlcv(id: &InstrumentId, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
    if range.start.is_some() || range.end.is_some() || id.provider == "local" {
        return ohlcv_uncached(id, tf, range).await;
    }
    let name = id.to_string();
    let key = format!("{name}|{}", tf.secs);
    let fetched = {
        let c = crate::store::open()?;
        crate::store::want(&c, &name, now_ms())?;
        let f = crate::store::fetch_state(&c, &key)?;
        if f.ok_at.is_some_and(|t| now_ms() - t < ttl_ms(id, tf)) && f.n >= range.limit {
            let bars = crate::store::load_bars(&c, &name, tf.secs, range.limit)?;
            if !bars.is_empty() {
                return Ok(bars);
            }
        }
        f.ok_at
    };
    let live = ohlcv_uncached(id, tf, range).await;
    let c = crate::store::open()?;
    match live {
        Ok(bars) => {
            crate::store::save_bars(&c, &name, tf.secs, &bars)?;
            crate::store::fetch_ok(&c, &key, now_ms(), range.limit)?;
            Ok(bars)
        }
        Err(e) => {
            crate::store::fetch_err(&c, &key, now_ms(), &first_line(&e))?;
            let stored = crate::store::load_bars(&c, &name, tf.secs, range.limit)?;
            match fetched {
                Some(t) if !stored.is_empty() => {
                    note_issue(format!("{name}: live fetch failed ({}); showing what was fetched {} ago", first_line(&e), crate::freshness::age_text(now_ms() - t)));
                    Ok(stored)
                }
                _ => Err(e),
            }
        }
    }
}

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

/// A quote for any id, through the local store like `ohlcv`.
pub async fn ticker(id: &InstrumentId) -> Result<Ticker> {
    if id.provider == "local" {
        return ticker_uncached(id).await;
    }
    let name = id.to_string();
    let key = format!("{name}|ticker");
    let fetched = {
        let c = crate::store::open()?;
        let f = crate::store::fetch_state(&c, &key)?;
        if f.ok_at.is_some_and(|t| now_ms() - t < ttl_ms(id, Timeframe::H1)) {
            if let Some(t) = crate::store::load_ticker(&c, &name)? {
                return Ok(t);
            }
        }
        f.ok_at
    };
    let live = ticker_uncached(id).await;
    let c = crate::store::open()?;
    match live {
        Ok(t) => {
            crate::store::save_ticker(&c, &name, &t)?;
            crate::store::fetch_ok(&c, &key, now_ms(), 1)?;
            Ok(t)
        }
        Err(e) => {
            crate::store::fetch_err(&c, &key, now_ms(), &first_line(&e))?;
            match (fetched, crate::store::load_ticker(&c, &name)?) {
                (Some(at), Some(t)) => {
                    note_issue(format!("{name}: live quote failed ({}); showing the quote fetched {} ago", first_line(&e), crate::freshness::age_text(now_ms() - at)));
                    Ok(t)
                }
                _ => Err(e),
            }
        }
    }
}

async fn ticker_uncached(id: &InstrumentId) -> Result<Ticker> {
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
