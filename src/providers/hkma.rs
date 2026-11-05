//! Hong Kong Monetary Authority open API (official), daily interbank liquidity:
//! `hkma:HIBOR_ON` and `hkma:HIBOR_1M` (HKD interbank rates, %) and `hkma:AGGREGATE_BALANCE`
//! (banks' clearing balances at the HKMA, HKD bn; it shrinks when the HKMA defends the peg).

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Hkma;

const URL: &str = "https://api.hkma.gov.hk/public/market-data-and-statistics/daily-monetary-statistics/daily-figures-interbank-liquidity";

const LIST: &[(&str, &str)] = &[
    ("HIBOR_ON", "HKD interbank offered rate, overnight, %"),
    ("HIBOR_1M", "HKD interbank offered rate fixing, 1 month, %"),
    ("AGGREGATE_BALANCE", "Hong Kong banks' aggregate balance at the HKMA, HKD bn"),
];

async fn series(symbol: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    let (field, scale) = match s.as_str() {
        "HIBOR_ON" => ("hibor_overnight", 1.0),
        "HIBOR_1M" => ("hibor_fixing_1m", 1.0),
        "AGGREGATE_BALANCE" => ("closing_balance", 1e-3),
        _ => bail!("hkma: unknown series '{symbol}' (HIBOR_ON, HIBOR_1M, AGGREGATE_BALANCE)"),
    };
    let mut out = Vec::new();
    for page in 0..20 {
        let v: Value = get_json(&format!("{URL}?pagesize=1000&offset={}&sortby=end_of_date&sortorder=desc", page * 1000)).await?;
        if v["header"]["success"] != true {
            bail!("hkma: {}", v["header"]["err_msg"].as_str().unwrap_or("error"));
        }
        let rows = v["result"]["records"].as_array().cloned().unwrap_or_default();
        let mut older = rows.is_empty();
        for r in &rows {
            let Some(t) = r["end_of_date"].as_str().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).map(day_ms) else { continue };
            older |= t < start;
            if let Some(x) = num(&r[field]).filter(|_| t >= start) {
                out.push(MetricPoint { timestamp: t, value: x * scale });
            }
        }
        if older {
            break;
        }
    }
    out.sort_by_key(|p| p.timestamp);
    Ok(out)
}

#[async_trait]
impl Provider for Hkma {
    fn id(&self) -> &'static str {
        "hkma"
    }
    fn describe(&self) -> &'static str {
        "Hong Kong Monetary Authority: HIBOR and the banking system's aggregate balance, daily"
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["daily, Hong Kong business days".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, lookback(tf, range)).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, now_ms() - 14 * DAY_MS).await?, "")
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("hkma", LIST, filter))
    }
}
