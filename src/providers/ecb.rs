//! European Central Bank data portal (official). `ecb:ESTR` is the euro short-term rate, and
//! `ecb:EA10Y` (also EA2Y EA5Y EA30Y) the euro-area AAA government yield curve spot rate, both in
//! percent, daily.

use super::*;
use crate::http::get_text;

pub struct Ecb;

const LIST: &[(&str, &str)] = &[
    ("ESTR", "euro short-term rate (ESTR), %"),
    ("EA2Y", "euro area AAA government yield 2y, %"),
    ("EA5Y", "euro area AAA government yield 5y, %"),
    ("EA10Y", "euro area AAA government yield 10y, %"),
    ("EA30Y", "euro area AAA government yield 30y, %"),
];

async fn series(symbol: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    let key = match s.as_str() {
        "ESTR" => "EST/B.EU000A2X2A25.WT".to_string(),
        "EA2Y" | "EA5Y" | "EA10Y" | "EA30Y" => format!("YC/B.U2.EUR.4F.G_N_A.SV_C_YM.SR_{}", &s[2..]),
        _ => bail!("ecb: unknown series '{symbol}' (ESTR, EA2Y, EA5Y, EA10Y, EA30Y)"),
    };
    let from = chrono::DateTime::from_timestamp_millis(start).unwrap_or_default().format("%Y-%m-%d");
    let text = get_text(&format!("https://data-api.ecb.europa.eu/service/data/{key}?startPeriod={from}&format=csvdata")).await?;
    let mut rd = csv::Reader::from_reader(text.as_bytes());
    let head = rd.headers()?.clone();
    let (Some(ti), Some(vi)) = (head.iter().position(|c| c == "TIME_PERIOD"), head.iter().position(|c| c == "OBS_VALUE")) else {
        bail!("ecb: unexpected response for {s}");
    };
    Ok(rd
        .records()
        .filter_map(|r| {
            let r = r.ok()?;
            Some(MetricPoint { timestamp: day_ms(chrono::NaiveDate::parse_from_str(r.get(ti)?, "%Y-%m-%d").ok()?), value: r.get(vi)?.parse().ok()? })
        })
        .collect())
}

#[async_trait]
impl Provider for Ecb {
    fn id(&self) -> &'static str {
        "ecb"
    }
    fn describe(&self) -> &'static str {
        "European Central Bank: euro short-term rate (ESTR) and the euro-area AAA government yield curve, daily"
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["daily, published next morning".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, lookback(tf, range)).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, now_ms() - 14 * DAY_MS).await?, "%")
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("ecb", LIST, filter))
    }
}
