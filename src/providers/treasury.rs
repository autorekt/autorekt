//! US Treasury FiscalData API (official). `treasury:TGA` is the Treasury General Account's
//! closing balance from the Daily Treasury Statement, $bn, one business day behind (FRED's
//! weekly series is a week average). Auction results by original term, nominal securities only:
//! `treasury:AUCTION_10Y` (also 2Y 3Y 5Y 7Y 20Y 30Y and bills 4W 6W 8W 13W 17W 26W 52W) is the
//! high yield in percent (investment rate for bills), with metrics `#bid_to_cover`,
//! `#indirect_pct` and `#dealer_pct` (shares of the accepted amount) and `#accepted` ($bn).

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Treasury;

const BASE: &str = "https://api.fiscaldata.treasury.gov/services/api/fiscal_service/v1/accounting";

const LIST: &[(&str, &str)] = &[
    ("TGA", "Treasury General Account closing balance, daily, $bn"),
    ("AUCTION_2Y", "2-year note auctions: high yield %, #bid_to_cover #indirect_pct #dealer_pct #accepted"),
    ("AUCTION_3Y", "3-year note auctions"),
    ("AUCTION_5Y", "5-year note auctions"),
    ("AUCTION_7Y", "7-year note auctions"),
    ("AUCTION_10Y", "10-year note auctions (and reopenings)"),
    ("AUCTION_20Y", "20-year bond auctions"),
    ("AUCTION_30Y", "30-year bond auctions"),
    ("AUCTION_4W", "4-week bill auctions: investment rate %"),
    ("AUCTION_13W", "13-week bill auctions"),
    ("AUCTION_26W", "26-week bill auctions"),
    ("AUCTION_52W", "52-week bill auctions"),
];

fn date(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().format("%Y-%m-%d").to_string()
}

fn ts(v: &Value) -> Option<i64> {
    Some(day_ms(chrono::NaiveDate::parse_from_str(v.as_str()?, "%Y-%m-%d").ok()?))
}

async fn rows(path: &str, date_field: &str, filter: &str, fields: &str, start: i64) -> Result<Vec<Value>> {
    let url = format!("{BASE}/{path}?filter={filter},{date_field}:gte:{}&sort={date_field}&page%5Bsize%5D=10000&fields={fields}", date(start));
    let v: Value = get_json(&url).await?;
    Ok(v["data"].as_array().cloned().unwrap_or_default())
}

async fn tga(start: i64) -> Result<Vec<MetricPoint>> {
    let filter = "account_type:eq:Treasury%20General%20Account%20(TGA)%20Closing%20Balance";
    let r = rows("dts/operating_cash_balance", "record_date", filter, "record_date,open_today_bal,close_today_bal", start).await?;
    // The closing-balance row carries its value in whichever balance column is filled.
    Ok(r.iter().filter_map(|x| Some(MetricPoint { timestamp: ts(&x["record_date"])?, value: num(&x["close_today_bal"]).or_else(|| num(&x["open_today_bal"]))? / 1000.0 })).collect())
}

fn term(code: &str) -> Option<String> {
    let (n, u) = code.split_at(code.len().checked_sub(1)?);
    n.parse::<u32>().ok()?;
    Some(match u {
        "Y" => format!("{n}-Year"),
        "W" => format!("{n}-Week"),
        _ => return None,
    })
}

async fn auctions(code: &str, metric: Option<&str>, start: i64) -> Result<Vec<MetricPoint>> {
    if let Some(m) = metric.filter(|m| !["bid_to_cover", "indirect_pct", "dealer_pct", "accepted"].contains(m)) {
        bail!("treasury: unknown metric '{m}' (supported: bid_to_cover, indirect_pct, dealer_pct, accepted)");
    }
    let t = term(code).ok_or_else(|| anyhow::anyhow!("treasury: unknown auction term '{code}' (e.g. AUCTION_10Y, AUCTION_13W)"))?;
    // Bills are filtered by term; notes and bonds by original term, so reopenings count.
    let filter = if t.ends_with("Week") { format!("security_term:eq:{t}") } else { format!("original_security_term:eq:{t},inflation_index_security:eq:No,floating_rate:eq:No") };
    let fields = "auction_date,high_yield,high_investment_rate,bid_to_cover_ratio,total_accepted,indirect_bidder_accepted,primary_dealer_accepted";
    let r = rows("od/auctions_query", "auction_date", &filter, fields, start).await?;
    Ok(r.iter()
        .filter_map(|x| {
            let total = num(&x["total_accepted"]);
            let value = match metric {
                None => num(&x["high_yield"]).or_else(|| num(&x["high_investment_rate"]))?,
                Some("bid_to_cover") => num(&x["bid_to_cover_ratio"])?,
                Some("indirect_pct") => num(&x["indirect_bidder_accepted"])? / total? * 100.0,
                Some("dealer_pct") => num(&x["primary_dealer_accepted"])? / total? * 100.0,
                Some("accepted") => total? / 1e9,
                Some(_) => return None,
            };
            Some(MetricPoint { timestamp: ts(&x["auction_date"])?, value })
        })
        .collect())
}

async fn series(symbol: &str, metric: Option<&str>, start: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    match s.as_str() {
        "TGA" if metric.is_none() => tga(start).await,
        "TGA" => bail!("treasury: TGA has no metrics"),
        _ => match s.strip_prefix("AUCTION_") {
            Some(code) => auctions(code, metric, start).await,
            None => bail!("treasury: unknown series '{symbol}' (TGA, AUCTION_10Y, AUCTION_13W, ...)"),
        },
    }
}

#[async_trait]
impl Provider for Treasury {
    fn id(&self) -> &'static str {
        "treasury"
    }
    fn describe(&self) -> &'static str {
        "US Treasury FiscalData: daily TGA balance, auction results (yield, bid-to-cover, buyer mix)"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["bid_to_cover", "indirect_pct", "dealer_pct", "accepted"]
    }
    fn notes(&self, symbol: &str) -> Vec<String> {
        vec![if symbol.eq_ignore_ascii_case("TGA") { "daily, one business day behind".into() } else { "one point per auction".into() }]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        // Auctions of one term are monthly or weekly: look back further for `limit` of them.
        let start = if symbol.eq_ignore_ascii_case("TGA") { lookback(tf, range) } else { range.start.unwrap_or_else(|| range.end_ms() - range.limit as i64 * 31 * DAY_MS) };
        Ok(point_bars(series(symbol, None, start).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, None, now_ms() - 120 * DAY_MS).await?, if symbol.eq_ignore_ascii_case("TGA") { "" } else { "%" })
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, _tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let start = range.start.unwrap_or_else(|| range.end_ms() - range.limit as i64 * 31 * DAY_MS);
        Ok(finish_metric(series(symbol, Some(metric), start).await?, range))
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("treasury", LIST, filter))
    }
}
