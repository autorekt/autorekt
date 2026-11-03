//! Yahoo Finance chart API (unofficial, free): indices, stocks, ETFs, futures, FX, yields.
//! Some quotes are delayed (exchange-dependent, typically 10–15 min for futures/indices).

use super::*;
use crate::http::{get_json, num, urlencode};
use serde_json::Value;

pub struct Yahoo;

fn base() -> String {
    crate::config::base_url("yahoo", "https://query2.finance.yahoo.com")
}

/// (Yahoo interval, resample target if Yahoo lacks the timeframe natively)
fn interval(tf: Timeframe) -> (&'static str, Option<Timeframe>) {
    match tf.secs {
        60 => ("1m", None),
        120 => ("2m", None),
        300 => ("5m", None),
        900 => ("15m", None),
        1800 => ("30m", None),
        3600 => ("1h", None),
        s if s < 86400 => ("1h", Some(tf)),
        86400 => ("1d", None),
        604800 => ("1wk", None),
        2592000 => ("1mo", None),
        _ => ("1d", Some(tf)),
    }
}

async fn chart(symbol: &str, iv: &str, p1: i64, p2: i64) -> Result<Value> {
    let url = format!(
        "{}/v8/finance/chart/{}?interval={iv}&period1={}&period2={}&includePrePost=false&events=div%2Csplit",
        base(),
        urlencode(symbol),
        p1 / 1000,
        p2 / 1000
    );
    let v: Value = match get_json(&url).await {
        Ok(v) => v,
        Err(e) if e.to_string().contains("HTTP 404") => {
            bail!("yahoo: unknown ticker '{symbol}' — search with `autorekt search {} --kind instruments`", symbol.trim_start_matches('^'))
        }
        Err(e) => return Err(e),
    };
    if let Some(e) = v["chart"]["error"].as_object() {
        bail!("yahoo: {} ({symbol})", e.get("description").and_then(|d| d.as_str()).unwrap_or("error"));
    }
    v["chart"]["result"].get(0).cloned().ok_or_else(|| anyhow::anyhow!("yahoo: no data for {symbol}"))
}

fn max_span_ms(iv: &str) -> i64 {
    let day = 86_400_000;
    match iv {
        "1m" => 7 * day,
        "2m" | "5m" | "15m" | "30m" => 59 * day,
        "1h" => 729 * day,
        _ => 100 * 365 * day,
    }
}

#[async_trait]
impl Provider for Yahoo {
    fn id(&self) -> &'static str {
        "yahoo"
    }
    fn describe(&self) -> &'static str {
        "Yahoo Finance: indices, stocks, ETFs, futures, FX, yields (unofficial; may be delayed)"
    }
    fn notes(&self, symbol: &str) -> Vec<String> {
        if symbol.ends_with("=F") || symbol.starts_with('^') {
            vec!["may be delayed".into()]
        } else {
            vec![]
        }
    }

    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let (iv, resample_to) = interval(tf);
        let end = range.end_ms();
        // Ask for extra room: markets are closed nights/weekends, so N bars span more wall time.
        let want = range.start.unwrap_or_else(|| {
            let bars_per_day = (86400 / tf.secs.max(1)).max(1) as f64;
            let trading_days = (range.limit as f64 / bars_per_day.min(7.0)).ceil() as i64;
            end - tf.ms().max(3_600_000) * range.limit as i64 * 3 - trading_days * 86_400_000
        });
        let start = want.max(end - max_span_ms(iv));
        let r = chart(symbol, iv, start, end).await?;
        let ts = r["timestamp"].as_array().cloned().unwrap_or_default();
        if ts.is_empty() && r["meta"]["regularMarketPrice"].is_null() {
            bail!("yahoo: no data for '{symbol}' — unknown ticker? search with `autorekt search {} --kind instruments`", symbol.trim_start_matches('^'));
        }
        let q = &r["indicators"]["quote"][0];
        let reg_time = r["meta"]["regularMarketTime"].as_i64().unwrap_or(0) * 1000;
        let step = Timeframe::parse(if iv == "1wk" { "1w" } else if iv == "1mo" { "1M" } else { iv }).map(|t| t.ms()).unwrap_or(tf.ms());
        let mut bars = Vec::new();
        for (i, t) in ts.iter().enumerate() {
            let (Some(o), Some(h), Some(l), Some(c)) = (num(&q["open"][i]), num(&q["high"][i]), num(&q["low"][i]), num(&q["close"][i])) else {
                continue;
            };
            let t = t.as_i64().unwrap_or(0) * 1000;
            bars.push(Bar {
                timestamp: t,
                open: o,
                high: h,
                low: l,
                close: c,
                volume: num(&q["volume"][i]).unwrap_or(0.0),
                closed: t + step <= reg_time.max(now_ms() - 20 * 60_000),
            });
        }
        // Daily bars: Yahoo stamps them at exchange-local midnight (cash markets) or at the
        // session open the evening before (futures, 18:00 ET). Rounding to the nearest UTC
        // midnight labels both with their trading date and is immune to DST changes.
        if iv == "1d" || iv == "1wk" || iv == "1mo" {
            for b in &mut bars {
                let t = b.timestamp + 12 * 3_600_000;
                b.timestamp = t - t.rem_euclid(86_400_000);
            }
        }
        // Yahoo intraday futures/indices sometimes carry a stray open (settlement or
        // session-roll print) far from the previous close while the bar's close is normal.
        // Re-anchor such opens to the previous close so they do not read as real moves.
        if !(iv == "1d" || iv == "1wk" || iv == "1mo") {
            for i in 1..bars.len() {
                let pc = bars[i - 1].close;
                let b = &mut bars[i];
                let jump = (b.open / pc - 1.0).abs();
                let settle = (b.close / pc - 1.0).abs();
                if jump > 0.02 && settle < jump / 3.0 {
                    let stray = b.open;
                    b.open = pc;
                    if b.high == stray {
                        b.high = pc.max(b.close).max(b.low);
                    }
                    if b.low == stray {
                        b.low = pc.min(b.close).min(b.high);
                    }
                }
            }
        }
        let bars = match resample_to {
            Some(t) => resample(&bars, t),
            None => bars,
        };
        Ok(finish(bars, range))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let end = now_ms();
        let r = chart(symbol, "1d", end - 10 * 86_400_000, end).await?;
        let m = &r["meta"];
        let last = num(&m["regularMarketPrice"]).ok_or_else(|| anyhow::anyhow!("yahoo: no price for {symbol}"))?;
        let mut notes = Vec::new();
        let t = m["regularMarketTime"].as_i64().unwrap_or(0) * 1000;
        if now_ms() - t > 3 * 3_600_000 {
            notes.push("market closed".into());
        }
        // previousClose is absent for some symbols; derive it from daily closes.
        let prev = num(&m["previousClose"]).or_else(|| {
            let closes: Vec<f64> = r["indicators"]["quote"][0]["close"].as_array()?.iter().filter_map(num).collect();
            if closes.len() >= 2 {
                Some(closes[closes.len() - 2])
            } else {
                None
            }
        });
        Ok(Ticker {
            symbol: symbol.into(),
            timestamp: t,
            last,
            high: num(&m["regularMarketDayHigh"]),
            low: num(&m["regularMarketDayLow"]),
            open: prev,
            base_volume: num(&m["regularMarketVolume"]).filter(|v| *v > 0.0),
            notes,
            ..Default::default()
        })
    }

    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        let q = filter.ok_or_else(|| anyhow::anyhow!("yahoo: give a search term"))?;
        let v: Value = get_json(&format!("{}/v1/finance/search?q={}&quotesCount=15&newsCount=0", base(), urlencode(q))).await?;
        Ok(v["quotes"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|x| {
                Some(MarketInfo {
                    id: format!("yahoo:{}", x["symbol"].as_str()?),
                    kind: x["quoteType"].as_str().unwrap_or("").to_lowercase(),
                    note: x["shortname"].as_str().or(x["longname"].as_str()).map(|s| format!("{} ({})", s, x["exchDisp"].as_str().unwrap_or(""))),
                })
            })
            .collect())
    }
}
