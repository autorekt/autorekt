//! China Foreign Exchange Trade System (chinamoney.com.cn), China's interbank market. The
//! central parity fixing set each morning at 09:15 Beijing time: `cfets:USDCNY` (also EURCNY,
//! JPYCNY per 100 yen, HKDCNY, GBPCNY). The gap to the market rate, in pips, shows how hard the
//! PBoC leans on the yuan: `(USDCNH - cfets:USDCNY) * 10000`. SHIBOR interbank offered rates
//! in percent: `cfets:SHIBOR_ON` (also 1W 2W 1M 3M 6M 9M 1Y).

use super::*;
use crate::http::client;
use serde_json::Value;

pub struct Cfets;

const BASE: &str = "https://www.chinamoney.com.cn/ags/ms";

const LIST: &[(&str, &str)] = &[
    ("USDCNY", "USD/CNY central parity fixing, daily 09:15 Beijing"),
    ("EURCNY", "EUR/CNY central parity fixing"),
    ("JPYCNY", "100 JPY/CNY central parity fixing"),
    ("HKDCNY", "HKD/CNY central parity fixing"),
    ("GBPCNY", "GBP/CNY central parity fixing"),
    ("SHIBOR_ON", "Shanghai interbank offered rate, overnight, %"),
    ("SHIBOR_1W", "SHIBOR 1 week, %"),
    ("SHIBOR_1M", "SHIBOR 1 month, %"),
    ("SHIBOR_3M", "SHIBOR 3 months, %"),
    ("SHIBOR_1Y", "SHIBOR 1 year, %"),
];

fn date(d: chrono::NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

async fn post(url: &str) -> Result<Value> {
    let r = client().post(url).send().await?;
    let status = r.status().as_u16();
    let text = r.text().await?;
    if status != 200 || !text.trim_start().starts_with('{') {
        bail!("cfets: HTTP {status} from chinamoney.com.cn (the site limits request rates; try again later)");
    }
    Ok(serde_json::from_str(&text)?)
}

/// Date windows of `days` from `start` to today, oldest first.
fn windows(start: i64, days: i64) -> Vec<(chrono::NaiveDate, chrono::NaiveDate)> {
    let today = chrono::Utc::now().date_naive();
    let mut a = chrono::DateTime::from_timestamp_millis(start).unwrap_or_default().date_naive();
    let mut out = Vec::new();
    while a <= today {
        let b = (a + chrono::Duration::days(days - 1)).min(today);
        out.push((a, b));
        a = b + chrono::Duration::days(1);
    }
    out
}

fn ts(s: &str) -> Option<i64> {
    Some(day_ms(chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?))
}

async fn fixing(pair: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let mut out = Vec::new();
    for (a, b) in windows(start, 30) {
        let v = post(&format!("{BASE}/cm-u-bk-ccpr/CcprHisNew?startDate={}&endDate={}&currency={pair}&pageNum=1&pageSize=50", date(a), date(b))).await?;
        for r in v["records"].as_array().into_iter().flatten() {
            if let (Some(t), Some(x)) = (r["date"].as_str().and_then(ts), r["values"][0].as_str().and_then(|s| s.parse().ok())) {
                out.push(MetricPoint { timestamp: t, value: x });
            }
        }
    }
    Ok(out)
}

async fn shibor(tenor: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let mut out = Vec::new();
    for (a, b) in windows(start, 180) {
        let v = post(&format!("{BASE}/cm-u-bk-shibor/ShiborHis?lang=CN&startDate={}&endDate={}", date(a), date(b))).await?;
        for r in v["records"].as_array().into_iter().flatten() {
            if let (Some(t), Some(x)) = (r["showDateCN"].as_str().and_then(ts), r[tenor].as_str().and_then(|s| s.parse().ok())) {
                out.push(MetricPoint { timestamp: t, value: x });
            }
        }
    }
    Ok(out)
}

async fn series(symbol: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    let pts = match s.as_str() {
        "USDCNY" => fixing("USD/CNY", start).await?,
        "EURCNY" => fixing("EUR/CNY", start).await?,
        "JPYCNY" => fixing("100JPY/CNY", start).await?,
        "HKDCNY" => fixing("HKD/CNY", start).await?,
        "GBPCNY" => fixing("GBP/CNY", start).await?,
        _ => match s.strip_prefix("SHIBOR_") {
            Some(t @ ("ON" | "1W" | "2W" | "1M" | "3M" | "6M" | "9M" | "1Y")) => shibor(t, start).await?,
            _ => bail!("cfets: unknown series '{symbol}' (USDCNY, EURCNY, JPYCNY, HKDCNY, GBPCNY, SHIBOR_ON, SHIBOR_1W, SHIBOR_1M, SHIBOR_3M, SHIBOR_1Y, ...)"),
        },
    };
    if pts.is_empty() {
        bail!("cfets: no {s} data returned for the requested dates");
    }
    Ok(pts)
}

#[async_trait]
impl Provider for Cfets {
    fn id(&self) -> &'static str {
        "cfets"
    }
    fn describe(&self) -> &'static str {
        "China interbank market (CFETS): yuan central parity fixings, SHIBOR"
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["daily, Beijing business days".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, lookback(tf, range)).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, now_ms() - 10 * DAY_MS).await?, "")
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("cfets", LIST, filter))
    }
}
