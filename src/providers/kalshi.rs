//! Kalshi (CFTC-regulated event contracts; public market data).
//! Ids: `kalshi:<MARKET_TICKER>` (e.g. KXFEDDECISION-26DEC-H25) or `kalshi:<EVENT_TICKER>`
//! (e.g. KXFEDDECISION-26DEC, listing all its markets). Prices are probabilities 0-1.

use super::*;
use crate::http::{get_json, num};
use serde_json::Value;

pub struct Kalshi;

fn base() -> String {
    crate::config::base_url("kalshi", "https://api.elections.kalshi.com/trade-api/v2")
}

fn dollars(m: &Value, k: &str) -> Option<f64> {
    num(&m[format!("{k}_dollars")]).or_else(|| num(&m[k]).map(|c| c / 100.0))
}

fn to_market(m: &Value) -> ProbMarket {
    let yes = dollars(m, "last_price").or_else(|| match (dollars(m, "yes_bid"), dollars(m, "yes_ask")) {
        (Some(b), Some(a)) => Some((a + b) / 2.0),
        _ => None,
    });
    let prev = dollars(m, "previous_price");
    let name = m["yes_sub_title"].as_str().filter(|s| !s.is_empty()).unwrap_or("Yes").to_string();
    ProbMarket {
        id: format!("kalshi:{}", m["ticker"].as_str().unwrap_or("")),
        question: m["title"].as_str().unwrap_or("").to_string(),
        outcomes: vec![Outcome {
            name,
            price: yes.unwrap_or(f64::NAN),
            bid: dollars(m, "yes_bid"),
            ask: dollars(m, "yes_ask"),
            token: m["ticker"].as_str().map(|s| s.to_string()),
        }],
        volume_24h: num(&m["volume_24h_fp"]).or(num(&m["volume_24h"])),
        volume: num(&m["volume_fp"]).or(num(&m["volume"])),
        change_24h: match (yes, prev) {
            (Some(y), Some(p)) => Some(y - p),
            _ => None,
        },
        end_date: m["close_time"].as_str().map(|s| s.to_string()),
        event: m["event_ticker"].as_str().map(|s| s.to_string()),
    }
}

impl Kalshi {
    async fn market(&self, ticker: &str) -> Result<Option<Value>> {
        match get_json::<Value>(&format!("{}/markets/{}", base(), ticker.to_uppercase())).await {
            Ok(v) if v["market"].is_object() => Ok(Some(v["market"].clone())),
            Ok(_) => Ok(None),
            Err(e) if e.to_string().contains("404") => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn event(&self, ticker: &str) -> Result<Option<(Value, Vec<Value>)>> {
        match get_json::<Value>(&format!("{}/events/{}?with_nested_markets=true", base(), ticker.to_uppercase())).await {
            Ok(v) if v["event"].is_object() => {
                let ms = v["event"]["markets"].as_array().cloned().or_else(|| v["markets"].as_array().cloned()).unwrap_or_default();
                Ok(Some((v["event"].clone(), ms)))
            }
            Ok(_) => Ok(None),
            Err(e) if e.to_string().contains("404") => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn history(&self, ticker: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let t = ticker.to_uppercase();
        let series = t.split('-').next().unwrap_or(&t);
        let iv = match tf.secs {
            s if s < 3600 => 1,
            s if s < 86400 => 60,
            _ => 1440,
        };
        let start = range.start_ms(tf) / 1000;
        let end = range.end_ms() / 1000;
        let v: Value = get_json(&format!("{}/series/{series}/markets/{t}/candlesticks?start_ts={start}&end_ts={end}&period_interval={iv}", base())).await?;
        let pts = v["candlesticks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                let p = num(&c["price"]["close_dollars"]).or_else(|| num(&c["price"]["close"]).map(|x| x / 100.0)).or_else(|| {
                    match (num(&c["yes_bid"]["close_dollars"]), num(&c["yes_ask"]["close_dollars"])) {
                        (Some(b), Some(a)) => Some((a + b) / 2.0),
                        _ => None,
                    }
                })?;
                Some(MetricPoint { timestamp: c["end_period_ts"].as_i64()? * 1000 - tf.ms(), value: p })
            })
            .collect();
        Ok(finish_metric(pts, range))
    }
}

#[async_trait]
impl ProbSource for Kalshi {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<ProbMarket>> {
        let words: Vec<String> = query.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
        let mut out = Vec::new();
        let mut cursor = String::new();
        for _ in 0..15 {
            let url = format!("{}/events?status=open&limit=200&with_nested_markets=true{}", base(), if cursor.is_empty() { String::new() } else { format!("&cursor={cursor}") });
            let v: Value = get_json(&url).await?;
            for e in v["events"].as_array().cloned().unwrap_or_default() {
                let hay = format!("{} {} {}", e["title"].as_str().unwrap_or(""), e["sub_title"].as_str().unwrap_or(""), e["event_ticker"].as_str().unwrap_or("")).to_lowercase();
                if !words.iter().all(|w| hay.contains(w)) {
                    continue;
                }
                let ms: Vec<ProbMarket> = e["markets"].as_array().cloned().unwrap_or_default().iter().map(to_market).collect();
                let outs = ms.iter().map(|m| Outcome { name: m.outcomes[0].name.clone(), price: m.outcomes[0].price, bid: None, ask: None, token: None }).collect();
                out.push(ProbMarket {
                    id: format!("kalshi:{}", e["event_ticker"].as_str().unwrap_or("")),
                    question: e["title"].as_str().unwrap_or("").to_string(),
                    outcomes: outs,
                    volume_24h: Some(ms.iter().filter_map(|m| m.volume_24h).sum()),
                    volume: None,
                    change_24h: None,
                    end_date: ms.first().and_then(|m| m.end_date.clone()),
                    event: None,
                });
                if out.len() >= limit {
                    return Ok(out);
                }
            }
            cursor = v["cursor"].as_str().unwrap_or("").to_string();
            if cursor.is_empty() {
                break;
            }
        }
        Ok(out)
    }

    async fn show(&self, id: &str) -> Result<Vec<ProbMarket>> {
        if let Some((_, ms)) = self.event(id).await? {
            return Ok(ms.iter().map(to_market).collect());
        }
        if let Some(m) = self.market(id).await? {
            return Ok(vec![to_market(&m)]);
        }
        bail!("kalshi: no event or market '{id}'")
    }

    async fn trending(&self, limit: usize) -> Result<Vec<ProbMarket>> {
        let v: Value = get_json(&format!("{}/markets?status=open&limit=1000", base())).await?;
        let mut ms: Vec<ProbMarket> = v["markets"].as_array().cloned().unwrap_or_default().iter().map(to_market).collect();
        ms.sort_by(|a, b| b.volume_24h.unwrap_or(0.0).partial_cmp(&a.volume_24h.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
        ms.truncate(limit);
        Ok(ms)
    }
}

#[async_trait]
impl Provider for Kalshi {
    fn id(&self) -> &'static str {
        "kalshi"
    }
    fn describe(&self) -> &'static str {
        "Kalshi event contracts (probabilities 0-1)"
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let pts = self.history(symbol, tf, range).await?;
        Ok(resample(&pts.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect::<Vec<_>>(), tf))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let m = self.market(symbol).await?.ok_or_else(|| anyhow::anyhow!("kalshi: '{symbol}' is not a market ticker (use `prob show`)"))?;
        let pm = to_market(&m);
        let o = &pm.outcomes[0];
        Ok(Ticker {
            symbol: symbol.into(),
            timestamp: now_ms(),
            last: o.price,
            bid: o.bid,
            ask: o.ask,
            open: pm.change_24h.map(|c| o.price - c),
            quote_volume: pm.volume_24h,
            notes: vec![format!("P({})", o.name)],
            ..Default::default()
        })
    }
}

/// The soonest-closing open event of a Kalshi series (e.g. KXFEDDECISION → next FOMC).
pub async fn next_event(series: &str) -> Result<Vec<ProbMarket>> {
    let v: Value = get_json(&format!("{}/events?series_ticker={series}&status=open&with_nested_markets=true", base())).await?;
    let mut evs = v["events"].as_array().cloned().unwrap_or_default();
    let close = |e: &Value| -> String {
        e["markets"].as_array().and_then(|m| m.first()).and_then(|m| m["close_time"].as_str()).unwrap_or("9999").to_string()
    };
    evs.sort_by_key(|e| close(e));
    let e = evs.into_iter().next().ok_or_else(|| anyhow::anyhow!("kalshi: no open event in series {series}"))?;
    let title = e["title"].as_str().unwrap_or("").to_string();
    let mut ms: Vec<ProbMarket> = e["markets"].as_array().cloned().unwrap_or_default().iter().map(to_market).collect();
    for m in ms.iter_mut() {
        m.question = title.clone();
    }
    Ok(ms)
}
