//! FINRA Reg SHO daily short-sale volume for off-exchange (TRF/ADF/ORF) trades — the public
//! raw material of "dark pool" indicators such as DIX. `finra:SPY` is the daily short-volume
//! ratio (short volume / total off-exchange volume); metrics: `#short_volume_ratio`,
//! `#short_volume`, `#total_volume`. Published each trading day after ~18:00 ET.

use super::*;
use crate::http::client;
use chrono::Datelike;

pub struct Finra;

/// One day's file (cached on disk once the day is complete).
async fn day_file(date: chrono::NaiveDate) -> Result<Option<String>> {
    let ymd = date.format("%Y%m%d").to_string();
    let dir = crate::config::home().join("cache").join("finra");
    let path = dir.join(format!("{ymd}.txt"));
    if let Ok(t) = std::fs::read_to_string(&path) {
        return Ok(Some(t));
    }
    let url = format!("https://cdn.finra.org/equity/regsho/daily/CNMSshvol{ymd}.txt");
    let r = client().get(&url).send().await?;
    if !r.status().is_success() {
        return Ok(None); // weekend, holiday, or not yet published
    }
    let t = r.text().await?;
    if !t.starts_with("Date|") {
        return Ok(None);
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, &t);
    Ok(Some(t))
}

async fn rows(symbol: &str, days: usize) -> Result<Vec<(i64, f64, f64)>> {
    let sym = symbol.to_uppercase();
    let mut out = Vec::new();
    let mut d = chrono::Utc::now().date_naive();
    let mut tried = 0;
    // Fetch business days going back until we have `days` files.
    let mut dates = Vec::new();
    while dates.len() < days && tried < days * 2 + 10 {
        tried += 1;
        if d.weekday().number_from_monday() <= 5 {
            dates.push(d);
        }
        d = d.pred_opt().unwrap();
    }
    let files = futures::future::join_all(dates.iter().map(|d| day_file(*d))).await;
    for (date, f) in dates.iter().zip(files) {
        let Ok(Some(text)) = f else { continue };
        let needle = format!("|{sym}|");
        if let Some(line) = text.lines().find(|l| l.contains(&needle)) {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 5 {
                let short: f64 = p[2].parse().unwrap_or(0.0);
                let total: f64 = p[4].parse().unwrap_or(0.0);
                let ts = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis();
                out.push((ts, short, total));
            }
        }
    }
    if out.is_empty() {
        bail!("finra: no short-volume rows for '{sym}' (US-listed symbols only; data starts the evening after each session)");
    }
    out.sort_by_key(|r| r.0);
    Ok(out)
}

#[async_trait]
impl Provider for Finra {
    fn id(&self) -> &'static str {
        "finra"
    }
    fn describe(&self) -> &'static str {
        "FINRA daily off-exchange (dark pool / TRF) short-sale volume per US stock"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["short_volume_ratio", "short_volume", "total_volume"]
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["off-exchange only, T+0 evening".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let pts = self.fetch_metric(symbol, "short_volume_ratio", tf, range).await?;
        Ok(pts.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect())
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let r = rows(symbol, 3).await?;
        let last = r.last().unwrap();
        let prev = r.len().checked_sub(2).map(|i| r[i].1 / r[i].2);
        Ok(Ticker { symbol: symbol.into(), timestamp: last.0, last: last.1 / last.2, open: prev, base_volume: Some(last.2), notes: vec!["short volume ratio".into()], ..Default::default() })
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, _tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let days = match range.start {
            Some(s) => (((range.end_ms() - s) / 86_400_000) as usize * 5 / 7).max(1) + 1,
            None => range.limit.min(260),
        };
        let r = rows(symbol, days).await?;
        let pts = r
            .into_iter()
            .map(|(t, s, tot)| MetricPoint {
                timestamp: t,
                value: match metric {
                    "short_volume" => s,
                    "total_volume" => tot,
                    _ => if tot > 0.0 { s / tot } else { f64::NAN },
                },
            })
            .collect();
        Ok(finish_metric(pts, range))
    }
}
