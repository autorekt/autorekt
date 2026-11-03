//! `local:<series>` — series recorded by monitors (`record()`) or by the daemon's `record:`
//! config (e.g. `local:hyperliquid:BTC#open_interest`).

use super::*;
use crate::store;

pub struct Local;

fn key(symbol: &str, metric: Option<&str>) -> String {
    match metric {
        Some(m) => format!("{symbol}#{m}"),
        None => symbol.to_string(),
    }
}

impl Local {
    fn read(&self, series: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let c = store::open()?;
        let start = range.start.unwrap_or(0);
        let pts = store::points(&c, series, start, range.end_ms())?;
        if pts.is_empty() {
            let have: Vec<String> = store::series_list(&c)?.into_iter().map(|s| s.0).take(30).collect();
            bail!("local: no recorded series '{series}' (have: {})", if have.is_empty() { "none".into() } else { have.join(", ") });
        }
        // Bucket to the timeframe, last value wins.
        let mut out: Vec<MetricPoint> = Vec::new();
        for p in pts {
            let b = p.timestamp - p.timestamp.rem_euclid(tf.ms());
            match out.last_mut() {
                Some(l) if l.timestamp == b => l.value = p.value,
                _ => out.push(MetricPoint { timestamp: b, value: p.value }),
            }
        }
        Ok(finish_metric(out, range))
    }
}

#[async_trait]
impl Provider for Local {
    fn id(&self) -> &'static str {
        "local"
    }
    fn describe(&self) -> &'static str {
        "Series recorded locally by monitors and the daemon (local:<name>)"
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(self.read(symbol, tf, range)?.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect())
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let pts = self.read(symbol, Timeframe { secs: 1 }, Range::last(100000))?;
        let last = pts.last().unwrap();
        let open = pts.iter().find(|p| p.timestamp >= last.timestamp - 86_400_000).map(|p| p.value);
        Ok(Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.value, open, ..Default::default() })
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        self.read(&key(symbol, Some(metric)), tf, range)
    }
}
