//! News (RSS/Atom feeds + Google News search) and the macro calendar (ForexFactory weekly JSON).

use crate::http::{get_json, get_text, urlencode};
use crate::model::Event;
use anyhow::Result;
use serde_json::Value;

pub struct Feed {
    pub name: &'static str,
    pub url: &'static str,
    pub topic: &'static str,
}

pub const FEEDS: &[Feed] = &[
    Feed { name: "cnbc", url: "https://www.cnbc.com/id/100003114/device/rss/rss.html", topic: "markets" },
    Feed { name: "cnbc-econ", url: "https://www.cnbc.com/id/20910258/device/rss/rss.html", topic: "macro" },
    Feed { name: "ft", url: "https://www.ft.com/markets?format=rss", topic: "markets" },
    Feed { name: "bloomberg", url: "https://feeds.bloomberg.com/markets/news.rss", topic: "markets" },
    Feed { name: "marketwatch", url: "https://feeds.content.dowjones.io/public/rss/mw_topstories", topic: "markets" },
    Feed { name: "seekingalpha", url: "https://seekingalpha.com/market_currents.xml", topic: "markets" },
    Feed { name: "investing", url: "https://www.investing.com/rss/news_25.rss", topic: "markets" },
    Feed { name: "fed", url: "https://www.federalreserve.gov/feeds/press_all.xml", topic: "macro" },
    Feed { name: "coindesk", url: "https://www.coindesk.com/arc/outboundfeeds/rss/", topic: "crypto" },
    Feed { name: "cointelegraph", url: "https://cointelegraph.com/rss", topic: "crypto" },
    Feed { name: "theblock", url: "https://www.theblock.co/rss.xml", topic: "crypto" },
    Feed { name: "decrypt", url: "https://decrypt.co/feed", topic: "crypto" },
];

fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    chrono::DateTime::parse_from_rfc2822(s)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(s))
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%a, %d %b %Y %H:%M:%S %Z"))
        .map(|d| d.timestamp_millis())
        .ok()
        .or_else(|| {
            // "Mon, 02 Nov 2026 14:00:00 GMT" variants with named zones
            let trimmed = s.rsplit_once(' ').map(|(a, _)| a)?;
            chrono::NaiveDateTime::parse_from_str(trimmed, "%a, %d %b %Y %H:%M:%S").ok().map(|d| d.and_utc().timestamp_millis())
        })
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse_feed(xml: &str, source: &str) -> Vec<Event> {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return vec![] };
    let mut out = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("item") || n.has_tag_name("entry")) {
        let child = |name: &str| node.children().find(|c| c.tag_name().name() == name);
        let text = |name: &str| child(name).and_then(|c| c.text()).unwrap_or("").trim().to_string();
        let title = strip_html(&text("title"));
        if title.is_empty() {
            continue;
        }
        let link = child("link").map(|l| l.attribute("href").map(|s| s.to_string()).unwrap_or_else(|| l.text().unwrap_or("").trim().to_string()));
        let date = ["pubDate", "published", "updated", "date"].iter().find_map(|k| parse_date(&text(k)));
        let body: String = strip_html(&[text("description"), text("summary")].concat()).chars().take(300).collect();
        // Google News titles end with " - Publisher"; keep the publisher as the source.
        let (title, src) = if source == "google" {
            match title.rsplit_once(" - ") {
                Some((t, p)) => (t.to_string(), format!("google/{p}")),
                None => (title, source.to_string()),
            }
        } else {
            (title, source.to_string())
        };
        let mut e = Event { timestamp: date.unwrap_or(0), kind: "news".into(), source: src, title, body, url: link, severity: 0, symbols: vec![] };
        tag(&mut e);
        out.push(e);
    }
    out
}

/// Crude impact score (0-3) and symbol tagging from the headline.
pub fn tag(e: &mut Event) {
    let t = e.title.to_lowercase();
    let high = [
        "crash", "plunge", "plummet", "collapse", "hack", "exploit", "halt", "emergency", "default", "bankrupt", "contagion", "liquidity crisis",
        "margin call", "circuit breaker", "rout", "meltdown", "panic", "black monday", "flash crash", "insolven", "bank run", "intervene", "intervention",
    ];
    let mid = [
        "fed", "fomc", "powell", "warsh", "rate hike", "rate cut", "hike", "cpi", "inflation", "payroll", "jobs report", "unemployment", "gdp", "tariff",
        "treasury", "yield", "selloff", "sell-off", "slump", "tumble", "surge", "soar", "liquidat", "record", "boj", "ecb", "pboc", "opec", "recession",
        "downgrade", "earnings", "guidance", "volatil", "vix", "sanction", "war", "election",
    ];
    let mut score: u8 = 0;
    // Hacks/exploits matter for markets only with a financial context (exchange, crypto, $).
    let money = ["crypto", "exchange", "bitcoin", "ether", "defi", "token", "stablecoin", "bank", "million", "billion", "$", "fund", "wallet", "bridge", "protocol"];
    let cyber_only = ["hack", "exploit"].iter().any(|k| t.contains(k)) && !money.iter().any(|k| t.contains(k));
    let high_hit = high.iter().filter(|k| t.contains(**k)).any(|k| !(cyber_only && (*k == "hack" || *k == "exploit")));
    if high_hit {
        score = 3;
    } else if mid.iter().filter(|k| t.contains(*k)).count() >= 2 {
        score = 2;
    } else if mid.iter().any(|k| t.contains(k)) {
        score = 1;
    }
    e.severity = e.severity.max(score);
    let tags: &[(&str, &[&str])] = &[
        ("BTC", &["bitcoin", "btc"]),
        ("ETH", &["ether", "eth "]),
        ("SOL", &["solana"]),
        ("SPX", &["s&p", "stocks", "wall street", "dow", "nasdaq"]),
        ("GOLD", &["gold"]),
        ("WTI", &["oil", "crude", "opec"]),
        ("US10Y", &["treasur", "yield", "bond"]),
        ("DXY", &["dollar"]),
        ("USDJPY", &["yen", "boj"]),
        ("FED", &["fed ", "fomc", "powell", "warsh", "federal reserve"]),
    ];
    for (sym, words) in tags {
        if words.iter().any(|w| t.contains(w)) && !e.symbols.iter().any(|s| s == sym) {
            e.symbols.push(sym.to_string());
        }
    }
}

pub async fn fetch_feed(f: &Feed) -> Result<Vec<Event>> {
    let xml = get_text(f.url).await?;
    Ok(parse_feed(&xml, f.name))
}

pub async fn google(query: &str) -> Result<Vec<Event>> {
    let url = format!("https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en", urlencode(query));
    Ok(parse_feed(&get_text(&url).await?, "google"))
}

/// Fetch all feeds (filtered by topic), concurrently; failures are reported, not fatal.
pub async fn fetch_all(topic: Option<&str>) -> (Vec<Event>, Vec<String>) {
    let feeds: Vec<&Feed> = FEEDS.iter().filter(|f| topic.map_or(true, |t| f.topic == t)).collect();
    let results = futures::future::join_all(feeds.iter().map(|f| fetch_feed(f))).await;
    let mut events = Vec::new();
    let mut errors = Vec::new();
    for (f, r) in feeds.iter().zip(results) {
        match r {
            Ok(mut v) => events.append(&mut v),
            Err(e) => errors.push(format!("{}: {}", f.name, e.to_string().lines().next().unwrap_or(""))),
        }
    }
    (dedupe(events), errors)
}

pub fn dedupe(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    let mut seen = std::collections::HashSet::new();
    events.retain(|e| {
        let key: String = e.title.to_lowercase().chars().filter(|c| c.is_alphanumeric()).take(60).collect();
        seen.insert(key)
    });
    events
}

/// One ForexFactory calendar file, cached on disk for an hour: the host rate-limits hard,
/// and the CLI and the daemon share the cache.
async fn calendar_file(url: &str) -> Result<Vec<Value>> {
    let name = url.rsplit('/').next().unwrap_or("calendar.json");
    let path = crate::config::home().join("cache").join(name);
    let fresh = std::fs::metadata(&path).and_then(|m| m.modified()).map(|t| t.elapsed().map_or(false, |d| d.as_secs() < 3600)).unwrap_or(false);
    if fresh {
        return Ok(serde_json::from_str(&std::fs::read_to_string(&path)?)?);
    }
    let v: Vec<Value> = get_json(url).await?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_string(&v)?)?;
    Ok(v)
}

/// Macro calendar for this week (and next week when available).
pub async fn calendar() -> Result<Vec<Event>> {
    let mut out = Vec::new();
    for (url, required) in [
        ("https://nfs.faireconomy.media/ff_calendar_thisweek.json", true),
        ("https://nfs.faireconomy.media/ff_calendar_nextweek.json", false),
    ] {
        let v: Vec<Value> = match calendar_file(url).await {
            Ok(v) => v,
            Err(e) if required => return Err(e),
            Err(_) => continue,
        };
        for x in v {
            let ts = x["date"].as_str().and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok()).map(|d| d.timestamp_millis()).unwrap_or(0);
            let impact = x["impact"].as_str().unwrap_or("");
            let severity = match impact {
                "High" => 3,
                "Medium" => 2,
                "Low" => 1,
                _ => 0,
            };
            let f = x["forecast"].as_str().unwrap_or("");
            let p = x["previous"].as_str().unwrap_or("");
            let a = x["actual"].as_str().unwrap_or("");
            let mut body = Vec::new();
            if !a.is_empty() {
                body.push(format!("actual {a}"));
            }
            if !f.is_empty() {
                body.push(format!("forecast {f}"));
            }
            if !p.is_empty() {
                body.push(format!("previous {p}"));
            }
            out.push(Event {
                timestamp: ts,
                kind: "calendar".into(),
                source: x["country"].as_str().unwrap_or("").to_string(),
                title: x["title"].as_str().unwrap_or("").to_string(),
                body: body.join(", "),
                url: None,
                severity,
                symbols: vec![],
            });
        }
    }
    out.sort_by_key(|e| e.timestamp);
    out.dedup_by(|a, b| a.timestamp == b.timestamp && a.title == b.title && a.source == b.source);
    Ok(out)
}
