//! Shared HTTP client with retries and readable errors.

use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use std::sync::OnceLock;
use std::time::Duration;

pub const UA: &str = concat!("autorekt/", env!("CARGO_PKG_VERSION"), " (+https://github.com/autorekt/autorekt)");
/// Only for endpoints that insist on a browser (CNN).
pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

pub fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        let mut b = reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .gzip(true);
        if let Some(p) = &crate::config::get().proxy {
            let px = reqwest::Proxy::all(p).unwrap_or_else(|e| {
                eprintln!("error: invalid proxy '{p}' in config.yaml: {e}");
                std::process::exit(2)
            });
            b = b.proxy(px);
        }
        b.build().expect("http client")
    })
}

fn explain(status: u16, url: &str, body: &str) -> anyhow::Error {
    let host = url.split('/').nth(2).unwrap_or(url);
    let hint = match status {
        451 => " — this API is geo-restricted from your location (e.g. Binance from the US); try another provider such as okx: or hyperliquid:",
        403 => " — access denied (often a geo-block or bot filter)",
        429 => " — rate limited; slow down",
        _ => "",
    };
    let snippet: String = body.chars().take(200).collect();
    anyhow!("HTTP {status} from {host}{hint}\n  {snippet}")
}

/// Hosts that throttle hard get at most one request per this many ms from this process.
fn pace_ms(url: &str) -> Option<u64> {
    let host = url.split('/').nth(2).unwrap_or("");
    (host.ends_with("eastmoney.com") || host.ends_with("hkexnews.hk")).then_some(1000)
}

/// Wait for this host's turn.
async fn pace(url: &str) {
    static LAST: tokio::sync::Mutex<Option<tokio::time::Instant>> = tokio::sync::Mutex::const_new(None);
    let Some(ms) = pace_ms(url) else { return };
    let mut last = LAST.lock().await;
    if let Some(t) = *last {
        tokio::time::sleep_until(t + Duration::from_millis(ms)).await;
    }
    *last = Some(tokio::time::Instant::now());
}

async fn send(req: reqwest::RequestBuilder, url: &str) -> Result<String> {
    let mut last_err = None;
    for attempt in 0..3 {
        pace(url).await;
        let r = req.try_clone().ok_or_else(|| anyhow!("request not cloneable"))?;
        match r.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let text = resp.text().await.unwrap_or_default();
                if (200..300).contains(&status) {
                    return Ok(text);
                }
                let e = explain(status, url, &text);
                if status == 429 || status >= 500 {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(500 * (attempt + 1) as u64)).await;
                    continue;
                }
                return Err(e);
            }
            Err(e) => {
                let host = url.split('/').nth(2).unwrap_or(url).to_string();
                last_err = Some(anyhow!("cannot reach {host}: {e}"));
                tokio::time::sleep(Duration::from_millis(400 * (attempt + 1) as u64)).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("request failed")))
}

pub async fn get_text(url: &str) -> Result<String> {
    send(client().get(url), url).await
}

pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let text = get_text(url).await?;
    serde_json::from_str(&text).map_err(|e| {
        let snippet: String = text.chars().take(200).collect();
        anyhow!("unexpected response from {url}: {e}\n  {snippet}")
    })
}

pub async fn post_json<T: DeserializeOwned>(url: &str, body: &serde_json::Value) -> Result<T> {
    let text = send(client().post(url).json(body), url).await?;
    serde_json::from_str(&text).map_err(|e| {
        let snippet: String = text.chars().take(200).collect();
        anyhow!("unexpected response from {url}: {e}\n  {snippet}")
    })
}

/// Parse a JSON value that may be a number or a numeric string.
pub fn num(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}


pub fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
