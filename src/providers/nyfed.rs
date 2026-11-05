//! New York Fed Markets API (official, same day). Reference rates in percent: `nyfed:SOFR`,
//! `nyfed:EFFR`, `nyfed:OBFR`, `nyfed:TGCR`, `nyfed:BGCR`, with the day's distribution as metrics
//! `#p1`, `#p25`, `#p75`, `#p99` and `#volume` ($bn). Facility use per day in $bn: `nyfed:SRF`
//! (standing repo facility) and `nyfed:RRP` (overnight reverse repo). Primary dealer positions
//! by series key, weekly, $mn: `nyfed:PDPOSGST-TOT` (Treasuries excluding TIPS, net).

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Nyfed;

const BASE: &str = "https://markets.newyorkfed.org/api";

const LIST: &[(&str, &str)] = &[
    ("SOFR", "secured overnight financing rate, %, metrics #p1 #p25 #p75 #p99 #volume"),
    ("TGCR", "tri-party general collateral rate, %"),
    ("BGCR", "broad general collateral rate, %"),
    ("EFFR", "effective federal funds rate, %"),
    ("OBFR", "overnight bank funding rate, %"),
    ("SRF", "standing repo facility, accepted per day, $bn"),
    ("RRP", "overnight reverse repo facility, accepted per day, $bn"),
    ("PDPOSGST-TOT", "primary dealer net positions in Treasuries excluding TIPS, weekly, $mn"),
    ("PDPOSCS-TOT", "primary dealer net positions in corporate securities, weekly, $mn"),
    ("PDPOSMBS-TOT", "primary dealer net positions in agency MBS, weekly, $mn"),
];

fn date(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().format("%Y-%m-%d").to_string()
}

fn ts(d: &Value) -> Option<i64> {
    Some(day_ms(chrono::NaiveDate::parse_from_str(d.as_str()?, "%Y-%m-%d").ok()?))
}

async fn rates(symbol: &str, metric: Option<&str>, start: i64) -> Result<Vec<MetricPoint>> {
    let kind = match symbol {
        "SOFR" | "TGCR" | "BGCR" => "secured",
        "EFFR" | "OBFR" => "unsecured",
        _ => unreachable!(),
    };
    let field = match metric {
        None => "percentRate",
        Some("p1") => "percentPercentile1",
        Some("p25") => "percentPercentile25",
        Some("p75") => "percentPercentile75",
        Some("p99") => "percentPercentile99",
        Some("volume") => "volumeInBillions",
        Some(m) => bail!("nyfed: unknown metric '{m}' for {symbol} (supported: p1, p25, p75, p99, volume)"),
    };
    let url = format!("{BASE}/rates/{kind}/{}/search.json?startDate={}&endDate={}", symbol.to_lowercase(), date(start), date(now_ms()));
    let v: Value = get_json(&url).await?;
    Ok(v["refRates"].as_array().into_iter().flatten().filter_map(|r| Some(MetricPoint { timestamp: ts(&r["effectiveDate"])?, value: num(&r[field])? })).collect())
}

/// Accepted amounts summed per operation date, in $bn. Up to three operations a day.
async fn operations(kind: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let n = ((now_ms() - start) / DAY_MS * 3).clamp(10, 3000);
    let url = format!("{BASE}/rp/{kind}/all/results/last/{n}.json");
    let v: Value = get_json(&url).await?;
    let mut by_day: std::collections::BTreeMap<i64, f64> = Default::default();
    for o in v["repo"]["operations"].as_array().into_iter().flatten() {
        if let (Some(t), Some(a)) = (ts(&o["operationDate"]), num(&o["totalAmtAccepted"])) {
            if t >= start {
                *by_day.entry(t).or_default() += a / 1e9;
            }
        }
    }
    Ok(by_day.into_iter().map(|(timestamp, value)| MetricPoint { timestamp, value }).collect())
}

/// Weekly, published about ten days after the as-of date: reach back further than `start`.
async fn dealer(key: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let start = start - 21 * DAY_MS;
    let v: Value = get_json(&format!("{BASE}/pd/get/{key}.json")).await?;
    let pts: Vec<MetricPoint> = v["pd"]["timeseries"].as_array().into_iter().flatten().filter_map(|r| Some(MetricPoint { timestamp: ts(&r["asofdate"])?, value: num(&r["value"])? })).filter(|p| p.timestamp >= start).collect();
    if pts.is_empty() {
        bail!("nyfed: no primary dealer series '{key}' (list keys with `autorekt search dealer --kind instruments`)");
    }
    Ok(pts)
}

async fn series(symbol: &str, metric: Option<&str>, start: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    match s.as_str() {
        "SOFR" | "TGCR" | "BGCR" | "EFFR" | "OBFR" => rates(&s, metric, start).await,
        _ if metric.is_some() => bail!("nyfed: {s} has no metrics"),
        "SRF" => operations("repo", start).await,
        "RRP" => operations("reverserepo", start).await,
        k if k.starts_with("PD") => dealer(k, start).await,
        _ => bail!("nyfed: unknown series '{symbol}' (SOFR, TGCR, BGCR, EFFR, OBFR, SRF, RRP, or a primary dealer key like PDPOSGST-TOT)"),
    }
}

fn is_rate(symbol: &str) -> bool {
    matches!(symbol.to_uppercase().as_str(), "SOFR" | "TGCR" | "BGCR" | "EFFR" | "OBFR")
}

#[async_trait]
impl Provider for Nyfed {
    fn id(&self) -> &'static str {
        "nyfed"
    }
    fn describe(&self) -> &'static str {
        "New York Fed: SOFR/EFFR with their distribution, standing repo and reverse repo use, primary dealer positions"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["p1", "p25", "p75", "p99", "volume"]
    }
    fn notes(&self, s: &str) -> Vec<String> {
        vec![if s.to_uppercase().starts_with("PD") { "weekly, about ten days behind".into() } else { "daily, same day".into() }]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, None, lookback(tf, range)).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, None, now_ms() - 45 * DAY_MS).await?, if is_rate(symbol) { "%" } else { "" })
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        Ok(finish_metric(series(symbol, Some(metric), lookback(tf, range)).await?, range))
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("nyfed", LIST, filter))
    }
}
