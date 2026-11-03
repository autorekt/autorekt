//! Polymarket (Gamma API for discovery, CLOB for price history).
//! Ids: `polymarket:<market-slug>` for one binary market, or `polymarket:<event-slug>` for an
//! event with several markets (e.g. the Fed decision). `#No` selects another outcome.

use super::*;
use crate::http::{get_json, num, urlencode};
use serde_json::Value;

pub struct Polymarket;

const GAMMA: &str = "https://gamma-api.polymarket.com";
const CLOB: &str = "https://clob.polymarket.com";

fn parse_list(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => serde_json::from_str::<Vec<Value>>(s)
            .unwrap_or_default()
            .iter()
            .map(|x| x.as_str().map(|s| s.to_string()).unwrap_or_else(|| x.to_string()))
            .collect(),
        Value::Array(a) => a.iter().map(|x| x.as_str().map(|s| s.to_string()).unwrap_or_else(|| x.to_string())).collect(),
        _ => vec![],
    }
}

pub fn to_market(m: &Value, event: Option<&str>) -> ProbMarket {
    let names = parse_list(&m["outcomes"]);
    let prices = parse_list(&m["outcomePrices"]);
    let tokens = parse_list(&m["clobTokenIds"]);
    let outcomes = names
        .iter()
        .enumerate()
        .map(|(i, n)| Outcome {
            name: n.clone(),
            price: prices.get(i).and_then(|p| p.parse().ok()).unwrap_or(f64::NAN),
            bid: if i == 0 { num(&m["bestBid"]) } else { num(&m["bestAsk"]).map(|a| 1.0 - a) },
            ask: if i == 0 { num(&m["bestAsk"]) } else { num(&m["bestBid"]).map(|b| 1.0 - b) },
            token: tokens.get(i).cloned(),
        })
        .collect();
    let title = m["groupItemTitle"].as_str().filter(|s| !s.is_empty()).map(|g| g.to_string());
    ProbMarket {
        id: format!("polymarket:{}", m["slug"].as_str().unwrap_or("")),
        question: title.unwrap_or_else(|| m["question"].as_str().unwrap_or("").to_string()),
        outcomes,
        volume_24h: num(&m["volume24hr"]),
        volume: num(&m["volumeNum"]).or(num(&m["volume"])),
        change_24h: num(&m["oneDayPriceChange"]),
        end_date: m["endDate"].as_str().map(|s| s.to_string()),
        event: event.map(|s| s.to_string()),
    }
}

fn open(m: &Value) -> bool {
    m["closed"].as_bool() != Some(true) && m["active"].as_bool() != Some(false)
}

impl Polymarket {
    async fn market(&self, slug: &str) -> Result<Option<Value>> {
        let v: Vec<Value> = get_json(&format!("{GAMMA}/markets?slug={}", urlencode(slug))).await?;
        Ok(v.into_iter().next())
    }

    async fn history(&self, slug: &str, outcome: Option<&str>, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        let m = self.market(slug).await?.ok_or_else(|| {
            anyhow::anyhow!("polymarket: '{slug}' is not a single market; run `autorekt prob show polymarket:{slug}` to list its markets")
        })?;
        let pm = to_market(&m, None);
        let o = match outcome {
            Some(name) => pm
                .outcomes
                .iter()
                .find(|o| o.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| anyhow::anyhow!("polymarket: no outcome '{name}' (have: {})", pm.outcomes.iter().map(|o| o.name.as_str()).collect::<Vec<_>>().join(", ")))?,
            None => pm.outcomes.first().ok_or_else(|| anyhow::anyhow!("polymarket: market has no outcomes"))?,
        };
        let token = o.token.clone().ok_or_else(|| anyhow::anyhow!("polymarket: no token id"))?;
        let fidelity = (tf.secs / 60).clamp(1, 1440);
        let start = range.start_ms(tf) / 1000;
        let end = range.end_ms() / 1000;
        let v: Value = get_json(&format!("{CLOB}/prices-history?market={token}&startTs={start}&endTs={end}&fidelity={fidelity}")).await?;
        let pts = v["history"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|p| Some(MetricPoint { timestamp: p["t"].as_i64()? * 1000, value: num(&p["p"])? }))
            .collect();
        Ok(finish_metric(pts, range))
    }
}

#[async_trait]
impl ProbSource for Polymarket {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<ProbMarket>> {
        let v: Value = get_json(&format!("{GAMMA}/public-search?q={}&limit_per_type={}&events_status=active", urlencode(query), limit.max(5))).await?;
        let mut out = Vec::new();
        for e in v["events"].as_array().cloned().unwrap_or_default() {
            let slug = e["slug"].as_str().unwrap_or("");
            let markets: Vec<Value> = e["markets"].as_array().cloned().unwrap_or_default().into_iter().filter(open).collect();
            if markets.is_empty() {
                continue;
            }
            if markets.len() == 1 {
                let mut pm = to_market(&markets[0], Some(slug));
                pm.question = e["title"].as_str().unwrap_or(&pm.question).to_string();
                out.push(pm);
            } else {
                // Summarize an event as one row: its markets become outcomes (each market's Yes price).
                let outs: Vec<Outcome> = markets
                    .iter()
                    .map(|m| {
                        let pm = to_market(m, Some(slug));
                        Outcome { name: pm.question.clone(), price: pm.outcomes.first().map(|o| o.price).unwrap_or(f64::NAN), bid: None, ask: None, token: None }
                    })
                    .collect();
                out.push(ProbMarket {
                    id: format!("polymarket:{slug}"),
                    question: e["title"].as_str().unwrap_or("").to_string(),
                    outcomes: outs,
                    volume_24h: num(&e["volume24hr"]),
                    volume: num(&e["volume"]),
                    change_24h: None,
                    end_date: e["endDate"].as_str().map(|s| s.to_string()),
                    event: None,
                });
            }
        }
        out.truncate(limit);
        Ok(out)
    }

    async fn show(&self, id: &str) -> Result<Vec<ProbMarket>> {
        if let Some(m) = self.market(id).await? {
            return Ok(vec![to_market(&m, None)]);
        }
        let v: Vec<Value> = get_json(&format!("{GAMMA}/events?slug={}", urlencode(id))).await?;
        let e = v.into_iter().next().ok_or_else(|| anyhow::anyhow!("polymarket: no market or event '{id}' (try `autorekt prob search`)"))?;
        let slug = e["slug"].as_str().unwrap_or(id).to_string();
        let mut ms: Vec<ProbMarket> = e["markets"].as_array().cloned().unwrap_or_default().iter().filter(|m| open(m)).map(|m| to_market(m, Some(&slug))).collect();
        if ms.is_empty() {
            ms = e["markets"].as_array().cloned().unwrap_or_default().iter().map(|m| to_market(m, Some(&slug))).collect();
        }
        Ok(ms)
    }

    async fn trending(&self, limit: usize) -> Result<Vec<ProbMarket>> {
        let v: Vec<Value> =
            get_json(&format!("{GAMMA}/events?limit={}&active=true&closed=false&order=volume24hr&ascending=false", limit.max(1))).await?;
        let mut out = Vec::new();
        for e in v {
            let slug = e["slug"].as_str().unwrap_or("").to_string();
            let markets: Vec<Value> = e["markets"].as_array().cloned().unwrap_or_default().into_iter().filter(open).collect();
            let mut outs: Vec<Outcome> = markets
                .iter()
                .map(|m| {
                    let pm = to_market(m, Some(&slug));
                    let name = if markets.len() == 1 { "Yes".to_string() } else { pm.question.clone() };
                    Outcome { name, price: pm.outcomes.first().map(|o| o.price).unwrap_or(f64::NAN), bid: None, ask: None, token: None }
                })
                .collect();
            outs.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal));
            out.push(ProbMarket {
                id: format!("polymarket:{slug}"),
                question: e["title"].as_str().unwrap_or("").to_string(),
                outcomes: outs,
                volume_24h: num(&e["volume24hr"]),
                volume: num(&e["volume"]),
                change_24h: None,
                end_date: e["endDate"].as_str().map(|s| s.to_string()),
                event: None,
            });
        }
        Ok(out)
    }
}

#[async_trait]
impl Provider for Polymarket {
    fn id(&self) -> &'static str {
        "polymarket"
    }
    fn describe(&self) -> &'static str {
        "Polymarket prediction markets (probabilities 0-1; history from the CLOB)"
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let pts = self.history(symbol, None, tf, range).await?;
        Ok(resample(&pts.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect::<Vec<_>>(), tf))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let m = self.market(symbol).await?.ok_or_else(|| anyhow::anyhow!("polymarket: '{symbol}' is not a single market (use `prob show`)"))?;
        let pm = to_market(&m, None);
        let o = pm.outcomes.first().ok_or_else(|| anyhow::anyhow!("no outcomes"))?;
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
    async fn fetch_metric(&self, symbol: &str, metric: &str, tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        // The "metric" of a prediction market is the outcome name: polymarket:<slug>#No
        self.history(symbol, Some(metric), tf, range).await
    }
}
