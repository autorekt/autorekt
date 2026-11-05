//! USD SOFR swap rates from the trades swap dealers must report publicly (CFTC Part 43,
//! disseminated by DTCC). `dtcc:SOFR10Y` (also 1Y 2Y 3Y 5Y 7Y 15Y 20Y 30Y) is the day's median
//! fixed rate, in percent, of new, spot-starting, standard-tenor SOFR OIS trades executed on a
//! swap execution facility, with no upfront payment and outside packages; days with fewer than
//! three such trades are skipped. `#trades`
//! is how many trades the median used. One value per business day, from the end-of-day file
//! published after the session; the archive covers about the last year. Swap spread:
//! `(dtcc:SOFR10Y - fred:DGS10) * 100` in basis points.

use super::*;
use crate::http::client;
use chrono::Datelike;
use futures::StreamExt;
use std::collections::BTreeMap;

pub struct Dtcc;

const TENORS: &[u32] = &[1, 2, 3, 5, 7, 10, 15, 20, 30];
const MIN_TRADES: usize = 3;
/// The public archive keeps daily files for a little over a year.
const MAX_DAYS: i64 = 400;

/// tenor in years → (median fixed rate %, trade count)
type Day = BTreeMap<u32, (f64, usize)>;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

fn parse_day(zip_bytes: &[u8]) -> Result<Day> {
    // Stream the CSV out of the archive: a day's file is tens of MB uncompressed.
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))?;
    let mut rd = csv::Reader::from_reader(z.by_index(0)?);
    let h = rd.headers()?.clone();
    let col = |name: &str| h.iter().position(|c| c == name).ok_or_else(|| anyhow::anyhow!("dtcc: column '{name}' missing from the daily file"));
    let (under, action, event, upfront, package, platform, exec, eff, exp, fx1, fx2) = (
        col("UPI Underlier Name")?,
        col("Action type")?,
        col("Event type")?,
        col("Other payment type")?,
        col("Package indicator")?,
        col("Platform identifier")?,
        col("Execution Timestamp")?,
        col("Effective Date")?,
        col("Expiration Date")?,
        col("Fixed rate-Leg 1")?,
        col("Fixed rate-Leg 2")?,
    );
    let date = |s: &str| chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok();
    let mut rates: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    for rec in rd.records() {
        let r = rec?;
        // Off-platform reports include compressions, backloads and swaption exercises at stale rates.
        let off_platform = matches!(&r[platform], "" | "BILT" | "XXXX" | "XOFF");
        if &r[under] != "USD-SOFR-OIS Compound" || &r[action] != "NEWT" || &r[event] != "TRAD" || !r[upfront].is_empty() || &r[package] == "TRUE" || off_platform {
            continue;
        }
        let (Some(x), Some(e), Some(m)) = (date(&r[exec]), date(&r[eff]), date(&r[exp])) else { continue };
        if (e - x).num_days() > 5 {
            continue;
        }
        // Fixed rates are reported as fractions; anything outside 0-20% is a misreport.
        let Some(rate) = r[fx1].parse::<f64>().ok().or_else(|| r[fx2].parse::<f64>().ok()).filter(|x| *x > 0.0 && *x < 0.2) else { continue };
        let years = (m - e).num_days() as f64 / 365.25;
        if let Some(t) = TENORS.iter().find(|t| (years - **t as f64).abs() < 0.03) {
            rates.entry(*t).or_default().push(rate * 100.0);
        }
    }
    // A median of a handful of trades (holidays, thin tenors) is noise.
    Ok(rates.into_iter().filter(|(_, v)| v.len() >= MIN_TRADES).map(|(t, v)| (t, (median(v.clone()), v.len()))).collect())
}

/// One business day's medians, cached on disk once fetched.
async fn day(d: chrono::NaiveDate) -> Result<Option<Day>> {
    let dir = crate::config::home().join("cache").join("dtcc");
    let path = dir.join(format!("{}.json", d.format("%Y%m%d")));
    if let Ok(t) = std::fs::read_to_string(&path) {
        return Ok(Some(serde_json::from_str(&t)?));
    }
    let url = format!("https://kgc0418-tdw-data-0.s3.amazonaws.com/cftc/eod/CFTC_CUMULATIVE_RATES_{}.zip", d.format("%Y_%m_%d"));
    let mut tries = 0;
    let bytes = loop {
        tries += 1;
        let r = client().get(&url).send().await;
        match r {
            Ok(r) if r.status().as_u16() == 404 || r.status().as_u16() == 403 => return Ok(None), // holiday, or not yet published
            Ok(r) if r.status().is_success() => match r.bytes().await {
                Ok(b) => break b,
                Err(e) if tries >= 3 => return Err(e.into()),
                Err(_) => {}
            },
            Ok(r) if tries >= 3 => bail!("dtcc: HTTP {} for {url}", r.status().as_u16()),
            Err(e) if tries >= 3 => return Err(e.into()),
            _ => {}
        }
        tokio::time::sleep(std::time::Duration::from_millis(500 * tries)).await;
    };
    let v = tokio::task::spawn_blocking(move || parse_day(&bytes)).await??;
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, serde_json::to_string(&v)?);
    Ok(Some(v))
}

async fn series(symbol: &str, metric: Option<&str>, start: i64, end: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    let tenor: u32 = s.strip_prefix("SOFR").and_then(|t| t.strip_suffix('Y')).and_then(|t| t.parse().ok()).filter(|t| TENORS.contains(t)).ok_or_else(|| anyhow::anyhow!("dtcc: unknown series '{symbol}' (SOFR1Y, SOFR2Y, SOFR3Y, SOFR5Y, SOFR7Y, SOFR10Y, SOFR15Y, SOFR20Y, SOFR30Y)"))?;
    if let Some(m) = metric.filter(|m| *m != "trades") {
        bail!("dtcc: unknown metric '{m}' (supported: trades)");
    }
    let today = chrono::Utc::now().date_naive();
    let first = chrono::DateTime::from_timestamp_millis(start).unwrap_or_default().date_naive().max(today - chrono::Duration::days(MAX_DAYS));
    let last = chrono::DateTime::from_timestamp_millis(end).unwrap_or_default().date_naive().min(today.pred_opt().unwrap());
    let dates: Vec<chrono::NaiveDate> = first.iter_days().take_while(|d| *d <= last).filter(|d| d.weekday().number_from_monday() <= 5).collect();
    let days: Vec<(chrono::NaiveDate, Result<Option<Day>>)> = futures::stream::iter(dates).map(|d| async move { (d, day(d).await) }).buffer_unordered(6).collect().await;
    let mut pts = Vec::new();
    for (d, r) in days {
        if let Some((rate, n)) = r?.and_then(|v| v.get(&tenor).copied()) {
            pts.push(MetricPoint { timestamp: day_ms(d), value: if metric.is_some() { n as f64 } else { rate } });
        }
    }
    if pts.is_empty() {
        bail!("dtcc: no SOFR {tenor}Y trades between {first} and {last}");
    }
    pts.sort_by_key(|p| p.timestamp);
    Ok(pts)
}

#[async_trait]
impl Provider for Dtcc {
    fn id(&self) -> &'static str {
        "dtcc"
    }
    fn describe(&self) -> &'static str {
        "USD SOFR swap rates from publicly reported swap trades (DTCC, CFTC Part 43), daily"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["trades"]
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["daily median of reported trades, previous business day".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, None, lookback(tf, range), range.end_ms()).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, None, now_ms() - 7 * DAY_MS, now_ms()).await?, "%")
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        Ok(finish_metric(series(symbol, Some(metric), lookback(tf, range), range.end_ms()).await?, range))
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        const LIST: &[(&str, &str)] = &[
            ("SOFR1Y", "USD SOFR swap rate 1y, %"), ("SOFR2Y", "USD SOFR swap rate 2y, %"), ("SOFR3Y", "USD SOFR swap rate 3y, %"),
            ("SOFR5Y", "USD SOFR swap rate 5y, %"), ("SOFR7Y", "USD SOFR swap rate 7y, %"), ("SOFR10Y", "USD SOFR swap rate 10y, %"),
            ("SOFR15Y", "USD SOFR swap rate 15y, %"), ("SOFR20Y", "USD SOFR swap rate 20y, %"), ("SOFR30Y", "USD SOFR swap rate 30y, %"),
        ];
        Ok(list_static("dtcc", LIST, filter))
    }
}
