//! Cboe delayed option chains (every listed option of an underlying, with greeks and open
//! interest). Ids are underlyings: `cboe:SPX`, `cboe:NDX`, `cboe:SPY`, `cboe:QQQ`, `cboe:AAPL`.
//! Metrics, all computed from the current chain (record them for history):
//! `#gex` dealer gamma exposure in $bn per 1% move, assuming dealers are long calls and short
//! puts (positive: dealers dampen moves; negative: they amplify them); `#put_call_oi`,
//! `#put_call_volume`; `#skew_25d` 25-delta put minus call implied vol, vol points, at the
//! expiry nearest 30 days; `#zero_dte_pct` share of today's volume in options expiring today;
//! `#iv30` the underlying's 30-day implied vol. The plain id is `#gex`.

use super::*;
use crate::http::{client, BROWSER_UA};
use serde::Deserialize;

pub struct Cboe;

#[derive(Deserialize)]
struct Chain {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    current_price: f64,
    #[serde(default)]
    iv30: f64,
    options: Vec<Opt>,
}

#[derive(Deserialize)]
struct Opt {
    option: String,
    #[serde(default)]
    iv: f64,
    #[serde(default)]
    open_interest: f64,
    #[serde(default)]
    volume: f64,
    #[serde(default)]
    delta: f64,
    #[serde(default)]
    gamma: f64,
}

/// OCC symbol tail: YYMMDD, C/P, strike × 1000 in 8 digits → (expiry date, is_call).
fn parse(sym: &str) -> Option<(chrono::NaiveDate, bool)> {
    let tail = sym.get(sym.len().checked_sub(15)?..)?;
    let d = chrono::NaiveDate::parse_from_str(&tail[..6], "%y%m%d").ok()?;
    Some((d, &tail[6..7] == "C"))
}

/// Chains are large (tens of MB for SPX): keep each for a minute so the metrics of one
/// underlying on a board share a download.
static CACHE: std::sync::LazyLock<tokio::sync::Mutex<std::collections::HashMap<String, (i64, std::sync::Arc<Data>)>>> = std::sync::LazyLock::new(Default::default);

async fn chain(symbol: &str) -> Result<std::sync::Arc<Data>> {
    let s = symbol.to_uppercase();
    let mut cache = CACHE.lock().await;
    if let Some((t, d)) = cache.get(&s) {
        if now_ms() - t < 60_000 {
            return Ok(d.clone());
        }
    }
    let path = if ["SPX", "NDX", "RUT", "VIX", "XSP", "DJX"].contains(&s.as_str()) { format!("_{s}") } else { s.clone() };
    let url = format!("https://cdn.cboe.com/api/global/delayed_quotes/options/{path}.json");
    let r = client().get(&url).header("User-Agent", BROWSER_UA).send().await?;
    if !r.status().is_success() {
        bail!("cboe: HTTP {} for {s} (US-listed underlyings only)", r.status().as_u16());
    }
    let bytes = r.bytes().await?;
    let c: Chain = serde_json::from_slice(&bytes).map_err(|e| anyhow::anyhow!("cboe: unexpected response for {s}: {e}"))?;
    let d = std::sync::Arc::new(c.data);
    cache.insert(s, (now_ms(), d.clone()));
    Ok(d)
}

fn metric(d: &Data, m: &str) -> Result<f64> {
    let today = chrono::Utc::now().with_timezone(&chrono_tz::America::New_York).date_naive();
    let legs: Vec<(&Opt, chrono::NaiveDate, bool)> = d.options.iter().filter_map(|o| parse(&o.option).map(|(e, c)| (o, e, c))).collect();
    let sum = |call: bool, f: &dyn Fn(&Opt) -> f64| legs.iter().filter(|l| l.2 == call).map(|l| f(l.0)).sum::<f64>();
    Ok(match m {
        "gex" => {
            let g = |o: &Opt| o.gamma * o.open_interest;
            (sum(true, &g) - sum(false, &g)) * 100.0 * d.current_price * d.current_price * 0.01 / 1e9
        }
        "put_call_oi" => sum(false, &|o| o.open_interest) / sum(true, &|o| o.open_interest),
        "put_call_volume" => sum(false, &|o| o.volume) / sum(true, &|o| o.volume),
        "zero_dte_pct" => {
            let total: f64 = legs.iter().map(|l| l.0.volume).sum();
            legs.iter().filter(|l| l.1 == today).map(|l| l.0.volume).sum::<f64>() / total * 100.0
        }
        "iv30" => d.iv30,
        "skew_25d" => {
            let target = today + chrono::Duration::days(30);
            let expiry = legs.iter().map(|l| l.1).filter(|e| *e > today).min_by_key(|e| (*e - target).num_days().abs()).ok_or_else(|| anyhow::anyhow!("cboe: no expiries"))?;
            let near = |call: bool, delta: f64| legs.iter().filter(|l| l.1 == expiry && l.2 == call && l.0.iv > 0.0).min_by(|a, b| (a.0.delta - delta).abs().partial_cmp(&(b.0.delta - delta).abs()).unwrap()).map(|l| l.0.iv);
            match (near(false, -0.25), near(true, 0.25)) {
                (Some(p), Some(c)) => (p - c) * 100.0,
                _ => bail!("cboe: no 25-delta options at {expiry}"),
            }
        }
        _ => bail!("cboe: unknown metric '{m}' (supported: gex, put_call_oi, put_call_volume, skew_25d, zero_dte_pct, iv30)"),
    })
}

async fn point(symbol: &str, m: &str) -> Result<MetricPoint> {
    let d = chain(symbol).await?;
    Ok(MetricPoint { timestamp: now_ms(), value: metric(&d, m)? })
}

#[async_trait]
impl Provider for Cboe {
    fn id(&self) -> &'static str {
        "cboe"
    }
    fn describe(&self) -> &'static str {
        "Cboe delayed option chains: dealer gamma exposure, put/call ratios, 25-delta skew, 0DTE share"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["gex", "put_call_oi", "put_call_volume", "skew_25d", "zero_dte_pct", "iv30"]
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["current chain, ~15m delayed; history via the daemon's record config".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, _tf: Timeframe, _range: Range) -> Result<Vec<Bar>> {
        let p = point(symbol, "gex").await?;
        Ok(vec![Bar::point(p.timestamp, p.value)])
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let p = point(symbol, "gex").await?;
        Ok(Ticker { symbol: symbol.into(), timestamp: p.timestamp, last: p.value, notes: vec!["dealer gamma, $bn per 1% move".into()], ..Default::default() })
    }
    async fn fetch_metric(&self, symbol: &str, m: &str, _tf: Timeframe, _range: Range) -> Result<Vec<MetricPoint>> {
        Ok(vec![point(symbol, m).await?])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_occ_symbols() {
        assert_eq!(parse("SPXW261117C07930000"), Some((chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap(), true)));
        assert_eq!(parse("AAPL261218P00200000"), Some((chrono::NaiveDate::from_ymd_opt(2026, 12, 18).unwrap(), false)));
    }
}
