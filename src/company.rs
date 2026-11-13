//! What a company has filed and has coming up: exchange filings (HKEXnews for Hong Kong, SEC
//! EDGAR for the US), headlines about it, and dated events (US earnings and dividends from
//! Nasdaq, Hong Kong dividends from Eastmoney). Used by `show news ID...` and `show calendar ID...`.

use crate::http::get_json;
use crate::model::{now_ms, Event};
use anyhow::{bail, Context, Result};
use serde_json::Value;

/// The market of a stock id and its local code: ("hk", "00788"), ("us", "AAPL").
fn market(id: &str) -> Result<(&'static str, String)> {
    let parsed = crate::ids::parse(id)?;
    if parsed.provider != "yahoo" {
        bail!("{id}: company filings and events are for stock ids (0788.HK, AAPL)");
    }
    let s = parsed.symbol.clone();
    if let Some(code) = s.strip_suffix(".HK") {
        return Ok(("hk", format!("{:0>5}", code)));
    }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Ok(("us", s));
    }
    bail!("{id}: filings and company events cover Hong Kong (.HK) and US stocks")
}

fn sec_agent() -> Result<String> {
    let c = crate::config::get().sec_contact.clone().context("US filings come from the SEC, which requires contact details in each request: set sec_contact in ~/.autorekt/config.yaml")?;
    Ok(format!("autorekt {c}"))
}

async fn sec_json(url: &str) -> Result<Value> {
    let r = crate::http::client().get(url).header("User-Agent", sec_agent()?).send().await?.error_for_status()?;
    Ok(serde_json::from_str(&r.text().await?)?)
}

/// A HKEXnews page; it answers browsers only.
async fn hkex(url: &str) -> Result<String> {
    let r = crate::http::client().get(url).header("User-Agent", crate::http::BROWSER_UA).send().await?.error_for_status()?;
    Ok(r.text().await?)
}

/// HKEXnews's internal id and English name for a stock code.
async fn hkex_stock(code: &str) -> Result<(i64, String)> {
    let t = hkex(&format!("https://www1.hkexnews.hk/search/prefix.do?callback=callback&lang=EN&type=A&name={code}&market=SEHK")).await?;
    let json = t.trim().strip_prefix("callback(").and_then(|s| s.trim_end_matches(';').strip_suffix(')')).context("hkexnews: unexpected answer")?;
    let v: Value = serde_json::from_str(json)?;
    let s = v["stockInfo"].as_array().and_then(|a| a.iter().find(|x| x["code"] == code)).with_context(|| format!("hkexnews: no stock {code}"))?;
    Ok((s["stockId"].as_i64().unwrap_or(0), s["name"].as_str().unwrap_or(code).to_string()))
}

/// Which US restriction lists the stock is on (see lists.rs).
pub async fn lists(id: &str) -> Result<f64> {
    let ticker = match market(id)? {
        ("hk", code) => format!("{} HK", code.trim_start_matches('0')),
        (_, t) => format!("{t} US"),
    };
    let symbol = crate::ids::parse(id)?.symbol;
    crate::lists::check(&ticker, &crate::providers::yahoo::long_name(&symbol).await?).await
}

/// The company's name, for headline searches.
pub async fn name(id: &str) -> Result<String> {
    match market(id)? {
        ("hk", code) => Ok(hkex_stock(&code).await?.1),
        (_, t) => {
            let v = sec_json("https://www.sec.gov/files/company_tickers.json").await?;
            Ok(v.as_object().and_then(|m| m.values().find(|e| e["ticker"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(&t)))).and_then(|e| e["title"].as_str()).unwrap_or(&t).to_string())
        }
    }
}

/// Exchange filings since `since`, newest first.
pub async fn filings(id: &str, since: i64) -> Result<Vec<Event>> {
    let fmt = |t: i64| chrono::DateTime::from_timestamp_millis(t).unwrap_or_default().format("%Y%m%d").to_string();
    match market(id)? {
        ("hk", code) => {
            let (sid, _) = hkex_stock(&code).await?;
            let url = format!(
                "https://www1.hkexnews.hk/search/titleSearchServlet.do?sortDir=0&sortByOptions=DateTime&category=0&market=SEHK&stockId={sid}&documentType=-1&fromDate={}&toDate={}&title=&searchType=0&t1code=-2&t2Gcode=-2&t2code=-2&rowRange=100&lang=E",
                fmt(since),
                fmt(now_ms())
            );
            let v: Value = serde_json::from_str(&hkex(&url).await?)?;
            let rows: Vec<Value> = serde_json::from_str(v["result"].as_str().unwrap_or("[]"))?;
            Ok(rows
                .iter()
                .filter_map(|r| {
                    let t = chrono::NaiveDateTime::parse_from_str(r["DATE_TIME"].as_str()?, "%d/%m/%Y %H:%M").ok()?.and_utc().timestamp_millis() - 8 * 3_600_000;
                    let kind = r["LONG_TEXT"].as_str().unwrap_or("").to_string();
                    let important = ["Results", "Dividend", "Board Meeting", "Inside Information", "Profit Warning", "Connected Transaction"].iter().any(|k| kind.contains(k));
                    Some(Event { timestamp: t, kind: "filing".into(), source: "hkexnews".into(), title: r["TITLE"].as_str()?.to_string(), body: kind, url: r["FILE_LINK"].as_str().map(|l| format!("https://www1.hkexnews.hk{l}")), severity: if important { 2 } else { 1 }, symbols: vec![id.to_string()] })
                })
                .collect())
        }
        (_, t) => {
            let tickers = sec_json("https://www.sec.gov/files/company_tickers.json").await?;
            let cik = tickers.as_object().and_then(|m| m.values().find(|e| e["ticker"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(&t)))).and_then(|e| e["cik_str"].as_u64()).with_context(|| format!("sec: no company with ticker {t}"))?;
            let v = sec_json(&format!("https://data.sec.gov/submissions/CIK{cik:010}.json")).await?;
            let r = &v["filings"]["recent"];
            let n = r["form"].as_array().map_or(0, |a| a.len());
            Ok((0..n)
                .filter_map(|i| {
                    let date = chrono::NaiveDate::parse_from_str(r["filingDate"][i].as_str()?, "%Y-%m-%d").ok()?.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
                    let form = r["form"][i].as_str()?.to_string();
                    let doc = r["primaryDocument"][i].as_str().unwrap_or("");
                    let acc = r["accessionNumber"][i].as_str().unwrap_or("").replace('-', "");
                    let important = ["10-K", "10-Q", "8-K", "DEF 14A"].contains(&form.as_str());
                    (date >= since).then(|| Event { timestamp: date, kind: "filing".into(), source: "sec".into(), title: format!("{form} {}", r["primaryDocDescription"][i].as_str().unwrap_or("")).trim().to_string(), body: r["items"][i].as_str().unwrap_or("").to_string(), url: Some(format!("https://www.sec.gov/Archives/edgar/data/{cik}/{acc}/{doc}")), severity: if important { 2 } else { 1 }, symbols: vec![id.to_string()] })
                })
                .collect())
        }
    }
}

/// Dated company events from `since` on: earnings and dividends.
pub async fn events(id: &str, since: i64) -> Result<Vec<Event>> {
    let ev = |t: i64, title: String, body: String| Event { timestamp: t, kind: "company".into(), source: id.to_string(), title, body, url: None, severity: 2, symbols: vec![id.to_string()] };
    let day = |s: &str, f: &str| chrono::NaiveDate::parse_from_str(s, f).ok().and_then(|d| d.and_hms_opt(0, 0, 0)).map(|d| d.and_utc().timestamp_millis());
    let mut out = Vec::new();
    match market(id)? {
        ("hk", code) => {
            let url = format!("https://datacenter.eastmoney.com/securities/api/data/v1/get?reportName=RPT_HKF10_INFO_DIVIDEND&columns=ALL&filter=(SECUCODE=%22{code}.HK%22)&pageSize=6&sortColumns=NOTICE_DATE&sortTypes=-1");
            let v: Value = get_json(&url).await?;
            for r in v["result"]["data"].as_array().cloned().unwrap_or_default() {
                let plan = r["PLAN_EXPLAIN"].as_str().unwrap_or("").to_string();
                let period = r["ASSIGN_PERIOD"].as_str().unwrap_or("").to_string();
                if let Some(t) = r["EX_DIVIDEND_DATE"].as_str().and_then(|s| day(s.get(..10)?, "%Y-%m-%d")) {
                    out.push(ev(t, "ex-dividend".into(), format!("{period} {plan}")));
                }
                if let Some(t) = r["DIVIDEND_DATE"].as_str().and_then(|s| day(s.get(..10)?, "%Y-%m-%d")) {
                    out.push(ev(t, "dividend paid".into(), period.clone()));
                }
            }
        }
        (_, t) => {
            let get = |u: String| async move {
                let r = crate::http::client().get(&u).header("Accept", "application/json").header("User-Agent", crate::http::BROWSER_UA).send().await?.error_for_status()?;
                anyhow::Ok(r.json::<Value>().await?)
            };
            let v = get(format!("https://api.nasdaq.com/api/analyst/{t}/earnings-date")).await.context("nasdaq earnings date")?;
            let text = v["data"]["reportText"].as_str().unwrap_or("");
            if let Some(d) = text.split_whitespace().find_map(|w| day(w.trim_end_matches('.'), "%m/%d/%Y")) {
                out.push(ev(d, "earnings".into(), text.split(". ").next().unwrap_or("").to_string()));
            }
            let v = get(format!("https://api.nasdaq.com/api/quote/{t}/dividends?assetclass=stocks")).await.context("nasdaq dividends")?;
            for row in v["data"]["dividends"]["rows"].as_array().cloned().unwrap_or_default().iter().take(4) {
                if let Some(d) = row["exOrEffDate"].as_str().and_then(|s| day(s, "%m/%d/%Y")) {
                    out.push(ev(d, "ex-dividend".into(), format!("{} {}", row["amount"].as_str().unwrap_or(""), row["type"].as_str().unwrap_or("")).trim().to_string()));
                }
                if let Some(d) = row["paymentDate"].as_str().and_then(|s| day(s, "%m/%d/%Y")) {
                    out.push(ev(d, "dividend paid".into(), row["amount"].as_str().unwrap_or("").to_string()));
                }
            }
        }
    }
    out.retain(|e| e.timestamp >= since);
    out.sort_by_key(|e| e.timestamp);
    Ok(out)
}
