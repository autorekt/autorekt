//! Company fundamentals as metrics of a stock id (`0788.HK#pe`, `600519.SS#pb`, `AAPL#dy`):
//! Hong Kong stocks from Eastmoney's F10 main indicators, A-shares from Eastmoney's daily
//! valuation table (with history), US stocks from the SEC's XBRL company facts (annual 10-K
//! figures; set `sec_contact` in config.yaml, which the SEC requires in every request).

use crate::http::{get_json, num};
use crate::model::{now_ms, MetricPoint};
use anyhow::{bail, Context, Result};
use serde_json::Value;

pub const METRICS: &[&str] = &["pe", "pb", "dy", "dps", "payout", "mcap", "eps", "bvps", "roe", "debt_ratio", "rev_yoy", "np_yoy"];

/// What a metric means, for help and errors.
pub const HELP: &str = "pe pb (ratios), dy (dividend yield %, last 12 months), dps, payout (% of earnings), mcap, eps, bvps, roe (%), debt_ratio (liabilities / assets %), rev_yoy np_yoy (revenue and profit growth %)";

fn day(s: &str) -> Option<i64> {
    chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok().map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis())
}

/// Hong Kong: the latest report's main indicators (valuations at the latest price).
async fn hong_kong(code: &str, metric: &str) -> Result<Vec<MetricPoint>> {
    let url = format!(
        "https://datacenter.eastmoney.com/securities/api/data/v1/get?reportName=RPT_HKF10_FN_MAININDICATOR&columns=ALL&filter=(SECUCODE=%22{code}.HK%22)&pageSize=1&sortColumns=REPORT_DATE&sortTypes=-1"
    );
    let v: Value = get_json(&url).await?;
    let r = v["result"]["data"].get(0).with_context(|| format!("eastmoney: no fundamentals for {code}.HK"))?;
    let col = match metric {
        "pe" => "PE_TTM",
        "pb" => "PB_TTM",
        "dy" => "DIVIDEND_RATE",
        "dps" => "DPS_HKD",
        "payout" => "DIVI_RATIO",
        "mcap" => "TOTAL_MARKET_CAP",
        "eps" => "EPS_TTM",
        "bvps" => "BPS",
        "roe" => "ROE_YEARLY",
        "debt_ratio" => "DEBT_ASSET_RATIO",
        "rev_yoy" => "OPERATE_INCOME_YOY",
        _ => "HOLDER_PROFIT_YOY",
    };
    let scale = if metric == "payout" { 100.0 } else { 1.0 };
    let value = num(&r[col]).with_context(|| format!("eastmoney: no {metric} for {code}.HK"))? * scale;
    Ok(vec![MetricPoint { timestamp: now_ms(), value }])
}

/// A-shares: daily valuations (pe, pb, mcap) with history.
async fn a_share(code: &str, metric: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let col = match metric {
        "pe" => "PE_TTM",
        "pb" => "PB_MRQ",
        "mcap" => "TOTAL_MARKET_CAP",
        m => bail!("eastmoney: {m} is not available for A-shares (pe, pb, mcap)"),
    };
    let mut out = Vec::new();
    for page in 1..=20 {
        let url = format!(
            "https://datacenter-web.eastmoney.com/api/data/v1/get?reportName=RPT_VALUEANALYSIS_DET&columns=TRADE_DATE,{col}&filter=(SECURITY_CODE=%22{code}%22)&pageSize=500&pageNumber={page}&sortColumns=TRADE_DATE&sortTypes=-1"
        );
        let v: Value = get_json(&url).await?;
        let rows = v["result"]["data"].as_array().cloned().unwrap_or_default();
        let mut older = false;
        for r in &rows {
            let (Some(t), Some(x)) = (r["TRADE_DATE"].as_str().and_then(day), num(&r[col])) else { continue };
            older |= t < start;
            out.push(MetricPoint { timestamp: t, value: x });
        }
        if older || rows.len() < 500 {
            break;
        }
    }
    if out.is_empty() {
        bail!("eastmoney: no valuations for {code}");
    }
    out.sort_by_key(|p| p.timestamp);
    Ok(out)
}

/// The SEC's CIK for a US ticker.
async fn cik(ticker: &str, ua: &str) -> Result<u64> {
    let path = crate::config::home().join("cache").join("sec_tickers.json");
    let fresh = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).is_some_and(|t| t.elapsed().map_or(false, |e| e.as_secs() < 7 * 86400));
    let text = if fresh {
        std::fs::read_to_string(&path)?
    } else {
        let t = crate::http::client().get("https://www.sec.gov/files/company_tickers.json").header("User-Agent", ua).send().await?.error_for_status()?.text().await?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, &t)?;
        t
    };
    let v: Value = serde_json::from_str(&text)?;
    v.as_object()
        .and_then(|m| m.values().find(|e| e["ticker"].as_str().is_some_and(|t| t.eq_ignore_ascii_case(ticker))))
        .and_then(|e| e["cik_str"].as_u64())
        .with_context(|| format!("sec: no company with ticker {ticker}"))
}

/// Days between a fact's start and end (none for a point-in-time figure such as equity).
fn span_days(r: &Value) -> Option<i64> {
    let d = |k: &str| chrono::NaiveDate::parse_from_str(r[k].as_str()?, "%Y-%m-%d").ok();
    Some((d("end")? - d("start")?).num_days())
}

/// The latest two fiscal-year values of a fact (newest first), from 10-K filings.
fn annual(facts: &Value, names: &[&str], unit: &str) -> Vec<f64> {
    for n in names {
        let Some(rows) = facts["facts"]["us-gaap"][n]["units"][unit].as_array() else { continue };
        let mut fy: Vec<(String, f64)> = rows
            .iter()
            // Annual figures: from 10-K filings, and for flows a span of about a year.
            .filter(|r| r["form"] == "10-K" && r["fp"] == "FY" && span_days(r).map_or(true, |d| d > 300))
            .filter_map(|r| Some((r["end"].as_str()?.to_string(), num(&r["val"])?)))
            .collect();
        fy.sort_by(|a, b| b.0.cmp(&a.0));
        fy.dedup_by(|a, b| a.0 == b.0);
        if !fy.is_empty() {
            return fy.into_iter().map(|(_, v)| v).collect();
        }
    }
    vec![]
}

/// US: annual 10-K figures, combined with the latest price where a ratio needs one.
async fn us(ticker: &str, metric: &str, price: f64) -> Result<Vec<MetricPoint>> {
    let contact = crate::config::get().sec_contact.clone().context("US fundamentals come from the SEC, which requires contact details in each request: set sec_contact in ~/.autorekt/config.yaml (e.g. sec_contact: \"Your Name you@example.com\")")?;
    let ua = format!("autorekt {contact}");
    let id = cik(ticker, &ua).await?;
    let url = format!("https://data.sec.gov/api/xbrl/companyfacts/CIK{id:010}.json");
    let facts: Value = serde_json::from_str(&crate::http::client().get(&url).header("User-Agent", &ua).send().await?.error_for_status()?.text().await?)?;
    let shares = facts["facts"]["dei"]["EntityCommonStockSharesOutstanding"]["units"]["shares"].as_array().and_then(|r| r.last()).and_then(|r| num(&r["val"]));
    let eps = annual(&facts, &["EarningsPerShareDiluted", "EarningsPerShareBasic"], "USD/shares");
    let dps = annual(&facts, &["CommonStockDividendsPerShareDeclared", "CommonStockDividendsPerShareCashPaid"], "USD/shares");
    let equity = annual(&facts, &["StockholdersEquity"], "USD");
    let income = annual(&facts, &["NetIncomeLoss"], "USD");
    let revenue = annual(&facts, &["Revenues", "RevenueFromContractWithCustomerExcludingAssessedTax", "SalesRevenueNet"], "USD");
    let liabilities = annual(&facts, &["Liabilities"], "USD");
    let assets = annual(&facts, &["Assets"], "USD");
    let first = |v: &[f64]| v.first().copied();
    let growth = |v: &[f64]| (v.len() >= 2 && v[1] != 0.0).then(|| (v[0] / v[1] - 1.0) * 100.0);
    let bvps = first(&equity).zip(shares).map(|(e, s)| e / s);
    let value = match metric {
        "pe" => first(&eps).filter(|e| *e > 0.0).map(|e| price / e),
        "pb" => bvps.filter(|b| *b > 0.0).map(|b| price / b),
        "dy" => Some(first(&dps).unwrap_or(0.0) / price * 100.0),
        "dps" => first(&dps),
        "payout" => first(&dps).zip(first(&eps)).filter(|(_, e)| *e > 0.0).map(|(d, e)| d / e * 100.0),
        "mcap" => shares.map(|s| s * price),
        "eps" => first(&eps),
        "bvps" => bvps,
        "roe" => first(&income).zip(first(&equity)).filter(|(_, e)| *e > 0.0).map(|(i, e)| i / e * 100.0),
        "debt_ratio" => first(&liabilities).zip(first(&assets)).map(|(l, a)| l / a * 100.0),
        "rev_yoy" => growth(&revenue),
        _ => growth(&income),
    };
    let value = value.with_context(|| format!("sec: {ticker} reports no figures for {metric}"))?;
    Ok(vec![MetricPoint { timestamp: now_ms(), value }])
}

/// `metric` of the Yahoo-style ticker `symbol`, from `start` (history only for A-shares).
pub async fn fetch(symbol: &str, metric: &str, start: i64, price: impl std::future::Future<Output = Result<f64>>) -> Result<Vec<MetricPoint>> {
    if !METRICS.contains(&metric) {
        bail!("unknown fundamental '{metric}' ({HELP})");
    }
    if let Some(code) = symbol.strip_suffix(".HK") {
        return hong_kong(&format!("{:0>5}", code), metric).await;
    }
    if let Some(code) = symbol.strip_suffix(".SS").or_else(|| symbol.strip_suffix(".SZ")) {
        return a_share(code, metric, start).await;
    }
    if symbol.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return us(symbol, metric, price.await?).await;
    }
    bail!("no fundamentals for {symbol}: Hong Kong (.HK), A-share (.SS, .SZ) and US stocks only")
}
