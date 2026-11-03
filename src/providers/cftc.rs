//! CFTC Commitments of Traders (weekly, as of Tuesday, released Friday). Ids are CFTC contract
//! codes: `cftc:13874A` (E-mini S&P 500), `cftc:043602` (10Y note), `cftc:088691` (gold) — or
//! aliases COT_ES, COT_NQ, COT_ZN, COT_GOLD, COT_WTI, COT_JPY, COT_BTC... Find codes with
//! `autorekt markets cftc <name>`.
//!
//! Metrics (legacy report): `#noncommercial_net` (default), `#noncommercial_long`,
//! `#noncommercial_short`, `#commercial_net`, `#open_interest`. Financial futures also have the
//! TFF report: `#lev_funds_net`, `#asset_mgr_net`, `#dealer_net` (and `_long` / `_short`).

use super::*;
use crate::http::{get_json, num, urlencode};
use serde_json::Value;

pub struct Cftc;

const LEGACY: &str = "https://publicreporting.cftc.gov/resource/6dca-aqww.json";
const TFF: &str = "https://publicreporting.cftc.gov/resource/gpe5-46if.json";

fn field_pair(metric: &str) -> Option<(&'static str, &'static str, &'static str)> {
    // (dataset, long field prefix, short field prefix)
    Some(match metric.trim_end_matches("_net").trim_end_matches("_long").trim_end_matches("_short") {
        "noncommercial" => ("legacy", "noncomm_positions_long_all", "noncomm_positions_short_all"),
        "commercial" => ("legacy", "comm_positions_long_all", "comm_positions_short_all"),
        "nonreportable" => ("legacy", "nonrept_positions_long_all", "nonrept_positions_short_all"),
        "lev_funds" => ("tff", "lev_money_positions_long", "lev_money_positions_short"),
        "asset_mgr" => ("tff", "asset_mgr_positions_long", "asset_mgr_positions_short"),
        "dealer" => ("tff", "dealer_positions_long_all", "dealer_positions_short_all"),
        _ => return None,
    })
}

fn get_f(row: &Value, k: &str) -> Option<f64> {
    num(&row[k]).or_else(|| num(&row[format!("{k}_all")]))
}

async fn history(code: &str, dataset: &str, weeks: usize) -> Result<Vec<Value>> {
    let base = if dataset == "tff" { TFF } else { LEGACY };
    let url = format!("{base}?cftc_contract_market_code={}&$order=report_date_as_yyyy_mm_dd%20DESC&$limit={}", urlencode(&code.to_uppercase()), weeks.max(1));
    let v: Vec<Value> = get_json(&url).await?;
    if v.is_empty() {
        bail!("cftc: no {dataset} rows for contract code '{code}' (find codes with `autorekt markets cftc <name>`)");
    }
    Ok(v)
}

fn ts(row: &Value) -> i64 {
    row["report_date_as_yyyy_mm_dd"].as_str().and_then(|d| chrono::NaiveDate::parse_from_str(&d[..10], "%Y-%m-%d").ok()).map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis()).unwrap_or(0)
}

#[async_trait]
impl Provider for Cftc {
    fn id(&self) -> &'static str {
        "cftc"
    }
    fn describe(&self) -> &'static str {
        "CFTC Commitments of Traders: weekly futures positioning (speculators, funds, dealers)"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["noncommercial_net", "noncommercial_long", "noncommercial_short", "commercial_net", "open_interest", "lev_funds_net", "asset_mgr_net", "dealer_net"]
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["weekly, as of Tuesday".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let pts = self.fetch_metric(symbol, "noncommercial_net", tf, range).await?;
        Ok(pts.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect())
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let pts = self.fetch_metric(symbol, "noncommercial_net", Timeframe::D1, Range::last(2)).await?;
        let last = pts.last().ok_or_else(|| anyhow::anyhow!("cftc: no data"))?;
        Ok(Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.value, open: pts.first().filter(|p| p.timestamp != last.timestamp).map(|p| p.value), notes: vec!["non-commercial net, contracts".into()], ..Default::default() })
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, _tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let weeks = match range.start {
            Some(s) => ((range.end_ms() - s) / (7 * 86_400_000)) as usize + 2,
            None => range.limit.min(520),
        };
        let pts: Vec<MetricPoint> = if metric == "open_interest" {
            history(symbol, "legacy", weeks).await?.iter().filter_map(|r| Some(MetricPoint { timestamp: ts(r), value: get_f(r, "open_interest")? })).collect()
        } else {
            let (ds, lk, sk) = field_pair(metric).ok_or_else(|| anyhow::anyhow!("cftc: unknown metric '{metric}' (supported: {})", self.metrics().join(", ")))?;
            history(symbol, ds, weeks)
                .await?
                .iter()
                .filter_map(|r| {
                    let (l, s) = (get_f(r, lk)?, get_f(r, sk)?);
                    let v = if metric.ends_with("_long") { l } else if metric.ends_with("_short") { s } else { l - s };
                    Some(MetricPoint { timestamp: ts(r), value: v })
                })
                .collect()
        };
        Ok(finish_metric(pts, range))
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        let since = (chrono::Utc::now() - chrono::Duration::days(21)).format("%Y-%m-%d");
        let mut url = format!("{LEGACY}?$select=cftc_contract_market_code,market_and_exchange_names&$where=report_date_as_yyyy_mm_dd>'{since}'&$limit=3000");
        if let Some(f) = filter {
            url = format!("{LEGACY}?$select=cftc_contract_market_code,market_and_exchange_names&$where=report_date_as_yyyy_mm_dd>'{since}' AND upper(market_and_exchange_names) like '%25{}%25'&$limit=500", urlencode(&f.to_uppercase()).replace("%25", "%25"));
        }
        let url = url.replace(' ', "%20").replace('\'', "%27").replace('>', "%3E");
        let v: Vec<Value> = get_json(&url).await?;
        let mut out: Vec<MarketInfo> = v
            .iter()
            .filter_map(|x| Some(MarketInfo { id: format!("cftc:{}", x["cftc_contract_market_code"].as_str()?), kind: "futures".into(), note: x["market_and_exchange_names"].as_str().map(|s| s.to_string()) }))
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out.dedup_by(|a, b| a.id == b.id);
        Ok(out)
    }
}
