//! Market structure computed from bars alone, the same way for every series: swing points
//! (zigzag), price levels that swings keep returning to, trend state, range statistics, and
//! data-quality checks. Nothing here knows which instrument it is looking at.

use crate::model::Bar;
use serde::Serialize;

/// Average true range over the last `n` bars.
pub fn atr(bars: &[Bar], n: usize) -> Option<f64> {
    if bars.len() < n + 1 {
        return None;
    }
    let tr: Vec<f64> = bars
        .windows(2)
        .map(|w| (w[1].high - w[1].low).max((w[1].high - w[0].close).abs()).max((w[1].low - w[0].close).abs()))
        .collect();
    let tail = &tr[tr.len() - n..];
    Some(tail.iter().sum::<f64>() / n as f64)
}

/// A swing high or low.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pivot {
    pub timestamp: i64,
    pub price: f64,
    pub high: bool,
}

/// Swing points: a new pivot is confirmed once price reverses by `threshold` from the last
/// extreme. The final element is the running extreme, which is not yet confirmed.
pub fn zigzag(bars: &[Bar], threshold: f64) -> Vec<Pivot> {
    let Some(first) = bars.first() else { return vec![] };
    let mut pivots: Vec<Pivot> = Vec::new();
    let mut hi = Pivot { timestamp: first.timestamp, price: first.high, high: true };
    let mut lo = Pivot { timestamp: first.timestamp, price: first.low, high: false };
    let mut up: Option<bool> = None;
    for b in &bars[1..] {
        if b.high > hi.price {
            hi = Pivot { timestamp: b.timestamp, price: b.high, high: true };
        }
        if b.low < lo.price {
            lo = Pivot { timestamp: b.timestamp, price: b.low, high: false };
        }
        match up {
            None if hi.price - lo.price >= threshold => {
                up = Some(hi.timestamp > lo.timestamp);
                pivots.push(if hi.timestamp > lo.timestamp { lo.clone() } else { hi.clone() });
            }
            Some(true) if hi.price - b.low >= threshold => {
                pivots.push(hi.clone());
                up = Some(false);
                lo = Pivot { timestamp: b.timestamp, price: b.low, high: false };
            }
            Some(false) if b.high - lo.price >= threshold => {
                pivots.push(lo.clone());
                up = Some(true);
                hi = Pivot { timestamp: b.timestamp, price: b.high, high: true };
            }
            _ => {}
        }
    }
    match up {
        Some(true) => pivots.push(hi),
        Some(false) => pivots.push(lo),
        None => {}
    }
    pivots
}

/// A price zone that swing points returned to.
#[derive(Debug, Clone, Serialize)]
pub struct Level {
    pub price: f64,
    pub touches: usize,
    pub last: i64,
}

/// Cluster confirmed pivots whose prices lie within `width` of each other.
pub fn levels(pivots: &[Pivot], width: f64) -> Vec<Level> {
    let mut sorted: Vec<&Pivot> = pivots.iter().collect();
    sorted.sort_by(|a, b| a.price.total_cmp(&b.price));
    let mut out: Vec<(Vec<&Pivot>, f64)> = Vec::new();
    for p in sorted {
        match out.last_mut() {
            Some((group, anchor)) if p.price - *anchor <= width => group.push(p),
            _ => out.push((vec![p], p.price)),
        }
    }
    out.into_iter()
        .map(|(g, _)| Level { price: g.iter().map(|p| p.price).sum::<f64>() / g.len() as f64, touches: g.len(), last: g.iter().map(|p| p.timestamp).max().unwrap_or(0) })
        .collect()
}

/// Whether the series is above its 20 and 50 period averages and which way the 20 is sloping.
#[derive(Debug, Clone, Serialize)]
pub struct Trend {
    pub above_sma20: bool,
    pub above_sma50: bool,
    pub sma20_rising: bool,
}

impl Trend {
    pub fn label(&self) -> &'static str {
        match (self.above_sma20, self.above_sma50, self.sma20_rising) {
            (true, true, true) => "up",
            (false, false, false) => "down",
            _ => "mixed",
        }
    }
}

fn sma(closes: &[f64], n: usize) -> Option<f64> {
    (closes.len() >= n).then(|| closes[closes.len() - n..].iter().sum::<f64>() / n as f64)
}

pub fn trend(bars: &[Bar]) -> Option<Trend> {
    let c: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let last = *c.last()?;
    let (s20, s50) = (sma(&c, 20)?, sma(&c, 50)?);
    let s20_prev = sma(&c[..c.len() - 5], 20)?;
    Some(Trend { above_sma20: last > s20, above_sma50: last > s50, sma20_rising: s20 > s20_prev })
}

/// Relative strength index (Wilder) of the closes.
pub fn rsi(bars: &[Bar], n: usize) -> Option<f64> {
    if bars.len() <= n {
        return None;
    }
    let (mut up, mut down) = (0.0, 0.0);
    for (i, w) in bars.windows(2).enumerate() {
        let d = w[1].close - w[0].close;
        let (u, dn) = (d.max(0.0), (-d).max(0.0));
        if i < n {
            up += u / n as f64;
            down += dn / n as f64;
        } else {
            up = (up * (n - 1) as f64 + u) / n as f64;
            down = (down * (n - 1) as f64 + dn) / n as f64;
        }
    }
    Some(if down == 0.0 { 100.0 } else { 100.0 - 100.0 / (1.0 + up / down) })
}

/// Percent change of the last close against the close `ms` earlier (daily bars).
pub fn change_since(bars: &[Bar], ms: i64) -> Option<f64> {
    let last = bars.last()?;
    let base = bars.iter().rev().find(|b| b.timestamp <= last.timestamp - ms)?;
    (base.close != 0.0).then(|| (last.close / base.close - 1.0) * 100.0)
}

/// Where the last close sits in the high-low range of `bars`, 0 (low) to 100 (high).
pub fn range_position(bars: &[Bar]) -> Option<f64> {
    let hi = bars.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
    let lo = bars.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
    let last = bars.last()?.close;
    (hi > lo).then(|| (last - lo) / (hi - lo) * 100.0)
}

/// Percentile (0-100) of the latest ATR(14) among all ATR(14) values in `bars`.
pub fn atr_percentile(bars: &[Bar]) -> Option<f64> {
    let values: Vec<f64> = (15..=bars.len()).filter_map(|i| atr(&bars[..i], 14)).collect();
    let last = *values.last()?;
    Some(values.iter().filter(|v| **v <= last).count() as f64 / values.len() as f64 * 100.0)
}

/// Things that make a series untrustworthy, described in words.
pub fn quality(bars: &[Bar]) -> Vec<String> {
    let mut out = Vec::new();
    let ohlc: Vec<&Bar> = bars.iter().filter(|b| b.high > b.low).collect();
    if ohlc.len() >= 10 {
        let flat = ohlc.iter().filter(|b| b.open == b.close).count();
        if flat * 2 > ohlc.len() {
            out.push(format!("suspect data: open equals close on {flat} of {} bars", ohlc.len()));
        }
    }
    let mut ranges: Vec<f64> = bars.iter().map(|b| b.high - b.low).filter(|r| *r > 0.0).collect();
    ranges.sort_by(f64::total_cmp);
    if ranges.len() >= 10 {
        let median = ranges[ranges.len() / 2];
        let outliers = bars.iter().filter(|b| b.high - b.low > 8.0 * median).count();
        if outliers > 0 {
            out.push(format!("{outliers} bar(s) span more than 8x the median range"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(t: i64, h: f64, l: f64) -> Bar {
        Bar { timestamp: t, open: (h + l) / 2.0, high: h, low: l, close: (h + l) / 2.0, volume: 0.0, closed: true }
    }

    #[test]
    fn zigzag_keeps_only_reversals_beyond_the_threshold() {
        // Up to 110, small dip to 107 (ignored), down to 95, up to 108 (unconfirmed).
        let bars: Vec<Bar> = [(101.0, 99.0), (105.0, 103.0), (110.0, 107.0), (108.0, 107.0), (104.0, 100.0), (97.0, 95.0), (103.0, 99.0), (108.0, 104.0)]
            .iter()
            .enumerate()
            .map(|(i, (h, l))| bar(i as i64, *h, *l))
            .collect();
        let p = zigzag(&bars, 6.0);
        let prices: Vec<f64> = p.iter().map(|x| x.price).collect();
        assert_eq!(prices, vec![99.0, 110.0, 95.0, 108.0]);
        assert_eq!(p.iter().map(|x| x.high).collect::<Vec<_>>(), vec![false, true, false, true]);
    }

    #[test]
    fn levels_cluster_nearby_pivots() {
        let p = |t, price| Pivot { timestamp: t, price, high: true };
        let l = levels(&[p(1, 100.0), p(5, 100.4), p(3, 120.0), p(9, 99.8)], 1.0);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].touches, 3);
        assert_eq!(l[0].last, 9);
        assert!((l[0].price - 100.066).abs() < 0.01);
    }

    #[test]
    fn quality_flags_flat_bodies() {
        let day = 86_400_000;
        let bars: Vec<Bar> = (0..20).map(|i| Bar { timestamp: i * day, open: 1.0, high: 1.2, low: 0.9, close: 1.0, volume: 0.0, closed: true }).collect();
        let q = quality(&bars);
        assert!(q.iter().any(|s| s.contains("open equals close on 20 of 20")), "{q:?}");
        assert!(quality(&bars[..5]).is_empty());
    }

    #[test]
    fn rsi_and_position_on_a_rising_series() {
        let bars: Vec<Bar> = (0..30).map(|i| bar(i, 100.0 + i as f64 + 0.5, 100.0 + i as f64 - 0.5)).collect();
        assert_eq!(rsi(&bars, 14), Some(100.0));
        assert!(range_position(&bars).unwrap() > 95.0);
        assert_eq!(trend(&bars[..20]).map(|t| t.label()), None, "needs 50 bars");
    }
}
