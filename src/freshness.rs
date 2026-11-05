//! How old a series' latest value is, and whether that is normal for it. Live markets should be
//! minutes old unless closed; statistics are judged against their own publication rhythm: the
//! typical spacing of their points plus room for weekends and publication lag.

use crate::ids::InstrumentId;
use crate::model::Bar;

const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;
/// Younger than this, a value counts as live and gets no age label.
const LIVE: i64 = 10 * MIN;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Freshness {
    /// Milliseconds since the latest value's timestamp.
    pub age_ms: i64,
    /// The age beyond which this series is late.
    pub limit_ms: i64,
    /// The market is closed, so no newer value is expected.
    pub closed: bool,
}

impl Freshness {
    pub fn stale(&self) -> bool {
        !self.closed && self.age_ms > self.limit_ms
    }

    /// A flag for the value's age; empty when it is live.
    pub fn label(&self) -> String {
        let age = age_text(self.age_ms);
        if self.stale() {
            format!("STALE {age} old (normally within {})", age_text(self.limit_ms))
        } else if self.age_ms > LIVE {
            format!("{age} old")
        } else {
            String::new()
        }
    }
}

/// "45m", "7h", "2d 4h", "12d".
pub fn age_text(ms: i64) -> String {
    let ms = ms.max(0);
    match ms {
        _ if ms < HOUR => format!("{}m", ms / MIN),
        _ if ms < 2 * DAY => format!("{}h", ms / HOUR),
        _ if ms < 10 * DAY => format!("{}d {}h", ms / DAY, ms % DAY / HOUR),
        _ => format!("{}d", ms / DAY),
    }
}

/// Median spacing of the latest points (0 for fewer than two).
fn spacing(bars: &[Bar]) -> i64 {
    let mut gaps: Vec<i64> = bars.windows(2).rev().take(12).map(|w| w[1].timestamp - w[0].timestamp).filter(|g| *g > 0).collect();
    gaps.sort();
    gaps.get(gaps.len() / 2).copied().unwrap_or(0)
}

/// Normal maximum age of a series published at the spacing of `bars`.
fn rhythm_limit(bars: &[Bar]) -> i64 {
    match spacing(bars) {
        0 => 3 * DAY,
        gap => gap + (4 * DAY).max(gap * 6 / 10),
    }
}

/// Freshness of a series whose latest value has timestamp `latest`.
pub fn assess(id: &InstrumentId, bars: &[Bar], latest: i64, closed: bool, now: i64) -> Freshness {
    let limit_ms = match id.provider.as_str() {
        // Metrics are stamped with the start of their bucket (an hour, for hourly bars).
        "okx" | "binance" | "hyperliquid" | "coinbase" | "deribit" | "polymarket" | "kalshi" => 15 * MIN + spacing(bars).min(DAY),
        "yahoo" => 45 * MIN,
        // Primary dealer positions come out about nine days after their as-of date.
        "nyfed" if id.symbol.starts_with("PD") => rhythm_limit(bars) + 9 * DAY,
        _ => rhythm_limit(bars),
    };
    Freshness { age_ms: now - latest, limit_ms, closed }
}

/// Freshness of an expression's daily points, by their own rhythm.
pub fn assess_points(bars: &[Bar], now: i64) -> Option<Freshness> {
    let last = bars.last()?;
    Some(Freshness { age_ms: now - last.timestamp, limit_ms: rhythm_limit(bars), closed: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daily(n: i64) -> Vec<Bar> {
        (0..n).map(|i| Bar::point(i * DAY, 1.0)).collect()
    }

    #[test]
    fn statistics_are_judged_by_their_own_rhythm() {
        let id = crate::ids::parse("fred:DGS10").unwrap();
        let bars = daily(20);
        let last = bars.last().unwrap().timestamp;
        assert!(!assess(&id, &bars, last, false, last + 3 * DAY).stale(), "a long weekend is normal");
        let late = assess(&id, &bars, last, false, last + 9 * DAY);
        assert!(late.stale());
        assert_eq!(late.label(), "STALE 9d 0h old (normally within 5d 0h)");
        let weekly: Vec<Bar> = (0..10).map(|i| Bar::point(i * 7 * DAY, 1.0)).collect();
        let wl = weekly.last().unwrap().timestamp;
        assert!(!assess(&id, &weekly, wl, false, wl + 10 * DAY).stale(), "a weekly series ten days on is normal");
    }

    #[test]
    fn markets_are_live_unless_closed() {
        let id = crate::ids::parse("SPX").unwrap();
        assert_eq!(assess(&id, &[], 0, false, 5 * MIN).label(), "");
        assert!(assess(&id, &[], 0, false, 2 * HOUR).stale());
        let closed = assess(&id, &[], 0, true, 14 * HOUR);
        assert!(!closed.stale());
        assert_eq!(closed.label(), "14h old");
    }
}
