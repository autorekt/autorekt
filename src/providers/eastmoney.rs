//! Eastmoney data center (China market statistics). China government bond yields in percent:
//! `eastmoney:CGB10Y` (also 2Y 5Y 30Y). A-share margin debt, CNY bn: `eastmoney:MARGIN` (margin
//! financing plus securities lending balance), metrics `#financing` and `#net_buy` (the day's
//! net margin purchases). Stock Connect, CNY bn: `eastmoney:SOUTHBOUND` (mainland net buying of
//! Hong Kong stocks; `#turnover`), `eastmoney:NORTHBOUND` (turnover only; net flows are no longer
//! published). `eastmoney:BREADTH`: share of A-shares up today, %, with `#advancers` and
//! `#decliners` (current values; record them for history).
//!
//! Quotes by Eastmoney's own market.code id: `eastmoney:116.00788` (Hong Kong), `eastmoney:1.600519`
//! (Shanghai), `eastmoney:0.000001` (Shenzhen), `eastmoney:90.BK0475` (an industry or concept
//! board), `eastmoney:124.HSTECH` (Hang Seng TECH), `eastmoney:133.USDCNH` (offshore yuan). Bars
//! are unadjusted, stamped in China time; `#main_flow` is the day's net buying by large orders, CNY bn
//! (Eastmoney's own measure).

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Eastmoney;

const DC: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";
const HIS: &str = "https://push2his.eastmoney.com/api/qt/stock";

/// Whether `symbol` is a quote id (market.code).
fn is_quote(symbol: &str) -> bool {
    symbol.split_once('.').is_some_and(|(m, c)| !m.is_empty() && m.chars().all(|x| x.is_ascii_digit()) && !c.is_empty())
}

/// A time in a kline ("2026-11-13" or "2026-11-13 10:30"): dates as UTC midnight of the trading
/// day, intraday times from China time.
fn kline_time(s: &str) -> Option<i64> {
    if s.len() == 10 {
        return chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().map(day_ms);
    }
    let t = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").ok()?;
    Some(t.and_utc().timestamp_millis() - 8 * 3_600_000)
}

async fn quote_bars(secid: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
    let klt = match tf.secs {
        60 => 1,
        300 => 5,
        900 => 15,
        1800 => 30,
        3600 => 60,
        86400 => 101,
        604800 => 102,
        2592000 => 103,
        _ => bail!("eastmoney: quotes come in 1m 5m 15m 30m 1h 1d 1w 1M bars, not {tf}"),
    };
    let fmt = |t: i64| chrono::DateTime::from_timestamp_millis(t).unwrap_or_default().format("%Y%m%d").to_string();
    let beg = range.start.map(fmt).unwrap_or_else(|| "0".into());
    let end = range.end.map(fmt).unwrap_or_else(|| "20500101".into());
    let url = format!("{HIS}/kline/get?secid={secid}&fields1=f1,f2,f3&fields2=f51,f52,f53,f54,f55,f56&klt={klt}&fqt=0&beg={beg}&end={end}&lmt={}&ut=fa5fd1943c7b386f172d6893dbfba10b", range.limit + 5);
    let v: Value = get_json(&url).await?;
    let lines = v["data"]["klines"].as_array().ok_or_else(|| anyhow::anyhow!("eastmoney: no quotes for {secid} (market.code, e.g. 116.00788, 1.600519, 90.BK0475)"))?;
    let today = now_ms() - now_ms().rem_euclid(DAY_MS);
    let bars = lines
        .iter()
        .filter_map(|l| {
            let f: Vec<&str> = l.as_str()?.split(',').collect();
            // Intraday klines are stamped at their end; bars here start at their open.
            let t = kline_time(f.first()?)? - if tf.secs < 86400 { tf.ms() } else { 0 };
            let n = |i: usize| f.get(i)?.parse::<f64>().ok();
            Some(Bar { timestamp: t, open: n(1)?, close: n(2)?, high: n(3)?, low: n(4)?, volume: n(5).unwrap_or(0.0), closed: tf.secs < 86400 || t < today })
        })
        .collect();
    Ok(finish(bars, range))
}

/// Daily net buying by large orders, CNY bn (Eastmoney keeps about the last 120 days).
async fn main_flow(secid: &str, range: Range) -> Result<Vec<MetricPoint>> {
    let url = format!("{HIS}/fflow/daykline/get?lmt=0&klt=101&secid={secid}&fields1=f1,f2,f3,f7&fields2=f51,f52");
    let v: Value = get_json(&url).await?;
    let lines = v["data"]["klines"].as_array().ok_or_else(|| anyhow::anyhow!("eastmoney: no fund flow for {secid}"))?;
    let pts = lines
        .iter()
        .filter_map(|l| {
            let (d, x) = l.as_str()?.split_once(',')?;
            Some(MetricPoint { timestamp: kline_time(d)?, value: x.split(',').next()?.parse::<f64>().ok()? * 1e-9 })
        })
        .collect();
    Ok(finish_metric(pts, range))
}

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
        "China market statistics and quotes (Eastmoney): government bond yields, margin debt, Stock Connect flows, A-share breadth; A-share, Hong Kong, board and index quotes with fund flows"
    }
    fn metrics(&self) -> &'static [&'static str] {
        &["financing", "net_buy", "turnover", "advancers", "decliners", "main_flow"]
    }
    fn notes(&self, s: &str) -> Vec<String> {
        if is_quote(s) {
            let stock = ["0.", "1.", "116.", "128."].iter().any(|m| s.starts_with(m));
            return if stock { vec!["unadjusted".into()] } else { vec![] };
        }
        vec![if s.eq_ignore_ascii_case("BREADTH") { "current session".into() } else { "daily, after the close".into() }]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        if is_quote(symbol) {
            return quote_bars(symbol, tf, range).await;
        }
        Ok(point_bars(series(symbol, None, lookback(tf, range)).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        if is_quote(symbol) {
            let bars = quote_bars(symbol, Timeframe::D1, Range::last(2)).await?;
            let last = bars.last().ok_or_else(|| anyhow::anyhow!("eastmoney: no quote for {symbol}"))?;
            let open = bars.len().checked_sub(2).map(|i| bars[i].close);
            let mut t = Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.close, high: Some(last.high), low: Some(last.low), open, base_volume: Some(last.volume), ..Default::default() };
            t.fill_change();
            return Ok(t);
        }
        point_ticker(symbol, series(symbol, None, now_ms() - 14 * DAY_MS).await?, if symbol.to_uppercase().starts_with("CGB") { "%" } else { "" })
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        if is_quote(symbol) {
            if metric != "main_flow" {
                bail!("eastmoney: metric '{metric}' not supported for {symbol} (main_flow)");
            }
            return main_flow(symbol, range).await;
        }
        Ok(finish_metric(series(symbol, Some(metric), lookback(tf, range)).await?, range))
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("eastmoney", LIST, filter))
    }
}
