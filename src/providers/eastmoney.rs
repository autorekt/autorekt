//! Eastmoney data center (China market statistics). China government bond yields in percent:
//! `eastmoney:CGB10Y` (also 2Y 5Y 30Y). A-share margin debt, CNY bn: `eastmoney:MARGIN` (margin
//! financing plus securities lending balance), metrics `#financing` and `#net_buy` (the day's
//! net margin purchases). Stock Connect, CNY bn: `eastmoney:SOUTHBOUND` (mainland net buying of
//! Hong Kong stocks; `#turnover`), `eastmoney:NORTHBOUND` (turnover only; net flows are no longer
//! published). `eastmoney:BREADTH`: share of A-shares up today, %, with `#advancers` and
//! `#decliners` (current values; record them for history).

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Eastmoney;

const DC: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";

const LIST: &[(&str, &str)] = &[
    ("CGB2Y", "China government bond yield 2y, %"),
    ("CGB5Y", "China government bond yield 5y, %"),
    ("CGB10Y", "China government bond yield 10y, %"),
    ("CGB30Y", "China government bond yield 30y, %"),
    ("MARGIN", "A-share margin financing + securities lending balance, CNY bn; #financing #net_buy"),
    ("SOUTHBOUND", "Stock Connect southbound net buying of HK stocks, CNY bn; #turnover"),
    ("NORTHBOUND", "Stock Connect northbound turnover, CNY bn"),
    ("BREADTH", "share of A-shares up today, %; #advancers #decliners"),
];

/// Rows of a data-center report, newest first, paged until older than `start`.
async fn report(name: &str, columns: &str, date_col: &str, filter: &str, start: i64) -> Result<Vec<(i64, Value)>> {
    let mut out = Vec::new();
    for page in 1..=40 {
        let url = format!("{DC}?reportName={name}&columns={columns},{date_col}{filter}&pageSize=500&pageNumber={page}&sortColumns={date_col}&sortTypes=-1");
        let v: Value = get_json(&url).await?;
        if v["success"] != true {
            bail!("eastmoney: report {name}: {}", v["message"].as_str().unwrap_or("error"));
        }
        let rows = v["result"]["data"].as_array().cloned().unwrap_or_default();
        let mut older = false;
        for r in rows.iter() {
            let Some(t) = r[date_col].as_str().and_then(|s| chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()).map(day_ms) else { continue };
            older |= t < start;
            out.push((t, r.clone()));
        }
        if older || page >= v["result"]["pages"].as_u64().unwrap_or(1) as usize {
            break;
        }
    }
    Ok(out.into_iter().filter(|(t, _)| *t >= start).collect())
}

fn pick(rows: Vec<(i64, Value)>, col: &str, scale: f64) -> Vec<MetricPoint> {
    rows.into_iter().filter_map(|(t, r)| Some(MetricPoint { timestamp: t, value: num(&r[col])? * scale })).collect()
}

async fn breadth(metric: Option<&str>) -> Result<Vec<MetricPoint>> {
    let v: Value = get_json("https://push2.eastmoney.com/api/qt/ulist.np/get?fltt=2&secids=1.000001,0.399001&fields=f12,f104,f105,f106").await?;
    let rows = v["data"]["diff"].as_array().cloned().unwrap_or_default();
    if rows.len() != 2 {
        bail!("eastmoney: breadth unavailable right now (empty answer)");
    }
    let sum = |f: &str| rows.iter().filter_map(|r| num(&r[f])).sum::<f64>();
    let (up, down, flat) = (sum("f104"), sum("f105"), sum("f106"));
    let value = match metric {
        None => up / (up + down + flat) * 100.0,
        Some("advancers") => up,
        Some("decliners") => down,
        Some(m) => bail!("eastmoney: unknown metric '{m}' for BREADTH (advancers, decliners)"),
    };
    Ok(vec![MetricPoint { timestamp: now_ms(), value }])
}

async fn series(symbol: &str, metric: Option<&str>, start: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    let bad = |m: &str| anyhow::anyhow!("eastmoney: unknown metric '{m}' for {s}");
    let mut pts = match (s.as_str(), metric) {
        ("CGB2Y" | "CGB5Y" | "CGB10Y" | "CGB30Y", None) => {
            let col = match s.as_str() {
                "CGB2Y" => "EMM00588704",
                "CGB5Y" => "EMM00166462",
                "CGB10Y" => "EMM00166466",
                _ => "EMM00166469",
            };
            pick(report("RPTA_WEB_TREASURYYIELD", col, "SOLAR_DATE", "", start).await?, col, 1.0)
        }
        ("MARGIN", m) => {
            let col = match m {
                None => "RZRQYE",
                Some("financing") => "RZYE",
                Some("net_buy") => "RZJME",
                Some(m) => return Err(bad(m)),
            };
            pick(report("RPTA_RZRQ_LSHJ", col, "DIM_DATE", "", start).await?, col, 1e-9)
        }
        ("SOUTHBOUND" | "NORTHBOUND", m) => {
            let (kind, col) = match (s.as_str(), m) {
                ("SOUTHBOUND", None) => ("006", "NET_DEAL_AMT"),
                ("SOUTHBOUND", Some("turnover")) => ("006", "DEAL_AMT"),
                ("NORTHBOUND", None) => ("005", "DEAL_AMT"),
                (_, Some(m)) => return Err(bad(m)),
                _ => unreachable!(),
            };
            let filter = format!("&filter=(MUTUAL_TYPE=%22{kind}%22)");
            pick(report("RPT_MUTUAL_DEAL_HISTORY", col, "TRADE_DATE", &filter, start).await?, col, 1e-3)
        }
        ("BREADTH", m) => breadth(m).await?,
        (_, Some(m)) if LIST.iter().any(|(id, _)| *id == s) => return Err(bad(m)),
        _ => bail!("eastmoney: unknown series '{symbol}' (CGB2Y, CGB5Y, CGB10Y, CGB30Y, MARGIN, SOUTHBOUND, NORTHBOUND, BREADTH)"),
    };
    pts.sort_by_key(|p| p.timestamp);
    Ok(pts)
}

#[async_trait]
impl Provider for Eastmoney {
    fn id(&self) -> &'static str {
        "eastmoney"
    }
    fn describe(&self) -> &'static str {
        "China market statistics (Eastmoney): government bond yields, margin debt, Stock Connect flows, A-share breadth"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["financing", "net_buy", "turnover", "advancers", "decliners"]
    }
    fn notes(&self, s: &str) -> Vec<String> {
        vec![if s.eq_ignore_ascii_case("BREADTH") { "current session".into() } else { "daily, after the close".into() }]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, None, lookback(tf, range)).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, None, now_ms() - 14 * DAY_MS).await?, if symbol.to_uppercase().starts_with("CGB") { "%" } else { "" })
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        Ok(finish_metric(series(symbol, Some(metric), lookback(tf, range)).await?, range))
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("eastmoney", LIST, filter))
    }
}
