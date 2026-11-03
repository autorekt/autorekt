//! Unified data types. Field names follow CCXT structures and pandas OHLCV columns.

use anyhow::{bail, Result};
use chrono::{Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One OHLCV bar. `timestamp` is the bar open time in Unix ms (CCXT convention).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bar {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    /// false for the bar that is still forming (CCXT Pro `watchOHLCV` semantics).
    pub closed: bool,
}

impl Bar {
    pub fn point(timestamp: i64, value: f64) -> Bar {
        Bar { timestamp, open: value, high: value, low: value, close: value, volume: 0.0, closed: true }
    }
}

/// A snapshot quote, CCXT `fetchTicker` shape (subset).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub symbol: String,
    pub timestamp: i64,
    pub last: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,
    /// Reference price for `change`/`percentage` (24h ago for crypto, previous close for TradFi).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
    /// Free-form notes such as "delayed 15m" or "market closed".
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub notes: Vec<String>,
}

impl Ticker {
    pub fn fill_change(&mut self) {
        if let Some(o) = self.open {
            if o != 0.0 && self.change.is_none() {
                self.change = Some(self.last - o);
                self.percentage = Some((self.last / o - 1.0) * 100.0);
            }
        }
    }
}

/// A named scalar over time: funding_rate, open_interest, long_short_ratio, ...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub timestamp: i64,
    pub value: f64,
}

/// One prediction-market outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub name: String,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<f64>,
    /// Provider-specific id used to fetch history (CLOB token id, Kalshi ticker, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// A prediction market (one question with outcomes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbMarket {
    pub id: String,
    pub question: String,
    pub outcomes: Vec<Outcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
}

/// News, macro releases, rule firings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: i64,
    pub kind: String,
    pub source: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub severity: u8,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub symbols: Vec<String>,
}

/// A CCXT timeframe: 1m 3m 5m 15m 30m 1h 2h 4h 6h 12h 1d 1w 1M.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timeframe {
    pub secs: i64,
}

impl Timeframe {
    pub const M1: Timeframe = Timeframe { secs: 60 };
    pub const H1: Timeframe = Timeframe { secs: 3600 };
    pub const D1: Timeframe = Timeframe { secs: 86400 };
    pub const MONTH: i64 = 30 * 86400;

    pub fn parse(s: &str) -> Result<Timeframe> {
        let s = s.trim();
        if s.is_empty() {
            bail!("empty timeframe");
        }
        let (num, unit) = s.split_at(s.len() - 1);
        let n: i64 = if num.is_empty() { 1 } else { num.parse().map_err(|_| anyhow::anyhow!("bad timeframe '{s}'"))? };
        let mult = match unit {
            "s" => 1,
            "m" => 60,
            "h" | "H" => 3600,
            "d" | "D" => 86400,
            "w" | "W" => 7 * 86400,
            "M" => Self::MONTH,
            _ => bail!("bad timeframe '{s}' (use 1m 5m 15m 1h 4h 1d 1w 1M)"),
        };
        Ok(Timeframe { secs: n * mult })
    }
    pub fn ms(&self) -> i64 {
        self.secs * 1000
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.secs;
        if s % Self::MONTH == 0 {
            write!(f, "{}M", s / Self::MONTH)
        } else if s % (7 * 86400) == 0 {
            write!(f, "{}w", s / (7 * 86400))
        } else if s % 86400 == 0 {
            write!(f, "{}d", s / 86400)
        } else if s % 3600 == 0 {
            write!(f, "{}h", s / 3600)
        } else if s % 60 == 0 {
            write!(f, "{}m", s / 60)
        } else {
            write!(f, "{}s", s)
        }
    }
}

/// Parse a duration like 30m, 6h, 7d, 90d into seconds.
pub fn parse_duration(s: &str) -> Result<i64> {
    Ok(Timeframe::parse(s)?.secs)
}

/// Parse a time given as Unix seconds or ms, RFC 3339, `YYYY-MM-DD HH:MM[:SS]` or `YYYY-MM-DD` (UTC).
pub fn parse_time(s: &str) -> Result<i64> {
    if let Ok(ms) = s.parse::<i64>() {
        return Ok(if ms < 10_000_000_000 { ms * 1000 } else { ms });
    }
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(d.timestamp_millis());
    }
    for f in ["%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(s, f) {
            return Ok(d.and_utc().timestamp_millis());
        }
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis());
    }
    bail!("cannot parse time '{s}' (use 2026-11-02, 2026-11-02T14:00:00Z or Unix ms)")
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Start of the bar of `tf` containing `ms` (UTC): weeks start on Monday, months on the 1st,
/// everything else on multiples of the timeframe.
pub fn bucket_start(ms: i64, tf: Timeframe) -> i64 {
    const DAY: i64 = 86_400_000;
    match tf.secs {
        s if s == 7 * 86400 => {
            let day = ms - ms.rem_euclid(DAY);
            // 1970-01-01 was a Thursday: (days + 3) mod 7 is the weekday with Monday = 0.
            day - (day / DAY + 3).rem_euclid(7) * DAY
        }
        Timeframe::MONTH => {
            let d = Utc.timestamp_millis_opt(ms).single().unwrap_or_default();
            Utc.with_ymd_and_hms(d.year(), d.month(), 1, 0, 0, 0).single().map(|m| m.timestamp_millis()).unwrap_or(ms)
        }
        _ => ms - ms.rem_euclid(tf.ms()),
    }
}

/// Resample bars into a coarser timeframe (OHLCV aggregation rules).
pub fn resample(bars: &[Bar], tf: Timeframe) -> Vec<Bar> {
    let mut out: Vec<Bar> = Vec::new();
    for b in bars {
        let bucket = bucket_start(b.timestamp, tf);
        match out.last_mut() {
            Some(last) if last.timestamp == bucket => {
                last.high = last.high.max(b.high);
                last.low = last.low.min(b.low);
                last.close = b.close;
                last.volume += b.volume;
                last.closed = b.closed;
            }
            _ => out.push(Bar { timestamp: bucket, ..b.clone() }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_start_on_mondays_and_month_firsts() {
        let t = |s: &str| chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis();
        let week = Timeframe { secs: 7 * 86400 };
        assert_eq!(bucket_start(t("2026-11-05T13:30:00Z"), week), t("2026-11-02T00:00:00Z"));
        assert_eq!(bucket_start(t("2026-11-02T00:00:00Z"), week), t("2026-11-02T00:00:00Z"));
        assert_eq!(bucket_start(t("2026-11-01T23:59:00Z"), week), t("2026-10-26T00:00:00Z"));
        assert_eq!(bucket_start(t("2026-11-05T13:30:00Z"), Timeframe { secs: Timeframe::MONTH }), t("2026-11-01T00:00:00Z"));
        assert_eq!(bucket_start(t("2026-11-05T13:30:00Z"), Timeframe { secs: 4 * 3600 }), t("2026-11-05T12:00:00Z"));
    }
}
