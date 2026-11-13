//! Table columns for `show --with`: functions of a series' daily closes (the latest value, the
//! change over a window or between two dates, highs and lows, drawdown, volatility, volume
//! against its average), or the latest value of an expression where `$` is the series.

use crate::model::{parse_time, Bar};
use anyhow::{bail, Result};

const DAY: i64 = 86_400_000;

/// The span a function looks at.
#[derive(Debug, Clone, PartialEq)]
pub enum Window {
    /// This many days back from the last bar.
    Days(i64),
    /// Since the last close of the previous year.
    Ytd,
    /// From the close on (or before) the first date to the close on (or before) the second.
    Between(i64, i64),
    /// The --from/--to span of the command.
    Command,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Func {
    Last,
    Chg(Window),
    High(Window),
    Low(Window),
    FromHigh(Window),
    FromLow(Window),
    Dd(Window),
    Vol(Window),
    VolRatio(usize),
    /// An expression; its latest value is shown.
    Expr(String),
}

#[derive(Debug, Clone)]
pub struct Column {
    pub label: String,
    pub func: Func,
}

/// "5d", "4w", "6M", "1y", "ytd", "2024-09-13..2024-10-07", or "" for the command's --from/--to.
fn window(s: &str) -> Result<Window> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Window::Command);
    }
    if s == "ytd" {
        return Ok(Window::Ytd);
    }
    if let Some((a, b)) = s.split_once("..") {
        return Ok(Window::Between(parse_time(a.trim())?, parse_time(b.trim())?));
    }
    let (n, unit) = s.split_at(s.len() - 1);
    let n: i64 = n.parse().map_err(|_| anyhow::anyhow!("bad window '{s}' (use 5d, 4w, 6M, 1y, ytd or 2024-09-13..2024-10-07)"))?;
    let days = match unit {
        "d" => n,
        "w" => 7 * n,
        "M" => 30 * n,
        "y" => 365 * n,
        _ => bail!("bad window '{s}' (use 5d, 4w, 6M, 1y, ytd or 2024-09-13..2024-10-07)"),
    };
    Ok(Window::Days(days))
}

pub fn parse(spec: &str) -> Result<Column> {
    let s = spec.trim();
    let label = s.to_string();
    if s == "last" {
        return Ok(Column { label, func: Func::Last });
    }
    let call = s.split_once('(').filter(|_| s.ends_with(')')).map(|(f, rest)| (f.trim(), &rest[..rest.len() - 1]));
    let func = match call {
        Some(("chg", w)) => Func::Chg(window(w)?),
        Some(("high", w)) => Func::High(window(w)?),
        Some(("low", w)) => Func::Low(window(w)?),
        Some(("from_high", w)) => Func::FromHigh(window(w)?),
        Some(("from_low", w)) => Func::FromLow(window(w)?),
        Some(("dd", w)) => Func::Dd(window(w)?),
        Some(("vol", w)) => Func::Vol(window(w)?),
        Some(("vol_ratio", n)) => Func::VolRatio(n.trim().parse().map_err(|_| anyhow::anyhow!("vol_ratio(N): N is a number of days, e.g. vol_ratio(20)"))?),
        _ if s.contains('$') => Func::Expr(s.to_string()),
        _ => bail!("unknown column '{s}': use last, chg(5d), chg(ytd), chg(2024-09-13..2024-10-07), high(52w), low(52w), from_high(52w), from_low(52w), dd(5y), vol(20d), vol_ratio(20), or an expression with $ such as 'ta.rsi($,14)'"),
    };
    Ok(Column { label, func })
}

fn close_at(bars: &[Bar], t: i64) -> Option<f64> {
    bars.iter().rev().find(|b| b.timestamp <= t).map(|b| b.close)
}

/// The (start, end) times a window covers, given the last bar's time and the command's span.
fn span(w: &Window, last: i64, command: Option<(i64, i64)>) -> Option<(i64, i64)> {
    match w {
        Window::Days(n) => Some((last - n * DAY, last)),
        Window::Ytd => {
            use chrono::Datelike;
            let y = chrono::DateTime::from_timestamp_millis(last)?.year();
            Some((chrono::NaiveDate::from_ymd_opt(y, 1, 1)?.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis() - 1, last))
        }
        Window::Between(a, b) => Some((*a, *b)),
        Window::Command => command,
    }
}

impl Column {
    /// How far back the daily bars must reach (ms) for this column, before `end`.
    pub fn reach(&self, end: i64) -> i64 {
        let w = match &self.func {
            Func::Chg(w) | Func::High(w) | Func::Low(w) | Func::FromHigh(w) | Func::FromLow(w) | Func::Dd(w) | Func::Vol(w) => w,
            Func::VolRatio(n) => return end - (*n as i64 * 2 + 10) * DAY,
            _ => return end - 10 * DAY,
        };
        match span(w, end, None) {
            Some((a, _)) => a - 10 * DAY,
            None => end - 10 * DAY,
        }
    }

    /// The value from daily bars (oldest first); `command` is the --from/--to span.
    pub fn eval(&self, bars: &[Bar], command: Option<(i64, i64)>) -> Option<f64> {
        let last = bars.last()?;
        let in_span = |w: &Window| -> Option<Vec<&Bar>> {
            let (a, b) = span(w, last.timestamp, command)?;
            let v: Vec<&Bar> = bars.iter().filter(|x| x.timestamp > a && x.timestamp <= b).collect();
            (!v.is_empty()).then_some(v)
        };
        match &self.func {
            Func::Last => Some(last.close),
            Func::Chg(w) => {
                let (a, b) = span(w, last.timestamp, command)?;
                let (p0, p1) = (close_at(bars, a)?, close_at(bars, b)?);
                (p0 != 0.0).then(|| (p1 / p0 - 1.0) * 100.0)
            }
            Func::High(w) => in_span(w).map(|v| v.iter().map(|x| x.high.max(x.close)).fold(f64::MIN, f64::max)),
            Func::Low(w) => in_span(w).map(|v| v.iter().map(|x| x.low.min(x.close)).fold(f64::MAX, f64::min)),
            Func::FromHigh(w) => {
                let h = in_span(w)?.iter().map(|x| x.high.max(x.close)).fold(f64::MIN, f64::max);
                Some((last.close / h - 1.0) * 100.0)
            }
            Func::FromLow(w) => {
                let l = in_span(w)?.iter().map(|x| x.low.min(x.close)).fold(f64::MAX, f64::min);
                (l > 0.0).then(|| (last.close / l - 1.0) * 100.0)
            }
            Func::Dd(w) => {
                let mut peak = f64::MIN;
                let mut worst: f64 = 0.0;
                for x in in_span(w)? {
                    peak = peak.max(x.close);
                    worst = worst.min(x.close / peak - 1.0);
                }
                Some(worst * 100.0)
            }
            Func::Vol(w) => {
                let v = in_span(w)?;
                let r: Vec<f64> = v.windows(2).filter(|p| p[0].close > 0.0 && p[1].close > 0.0).map(|p| (p[1].close / p[0].close).ln()).collect();
                (r.len() >= 2).then(|| crate::render::stdev(&r) * 252f64.sqrt() * 100.0)
            }
            Func::VolRatio(n) => {
                let k = bars.len();
                if k < n + 1 {
                    return None;
                }
                let avg = bars[k - 1 - n..k - 1].iter().map(|x| x.volume).sum::<f64>() / *n as f64;
                (avg > 0.0).then(|| last.volume / avg)
            }
            Func::Expr(_) => None,
        }
    }

    pub fn fmt(&self, v: Option<f64>) -> String {
        let Some(v) = v.filter(|v| v.is_finite()) else { return "-".into() };
        match &self.func {
            Func::Chg(_) | Func::FromHigh(_) | Func::FromLow(_) | Func::Dd(_) => format!("{v:+.2}%"),
            Func::Vol(_) => format!("{v:.1}%"),
            Func::VolRatio(_) => format!("{v:.2}x"),
            _ => crate::render::fmt_auto(v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars(closes: &[f64]) -> Vec<Bar> {
        closes.iter().enumerate().map(|(i, c)| Bar { timestamp: i as i64 * DAY, open: *c, high: *c, low: *c, close: *c, volume: 100.0 + i as f64, closed: true }).collect()
    }

    #[test]
    fn functions_read_daily_closes() {
        let b = bars(&[100.0, 110.0, 99.0, 121.0, 110.0]);
        let v = |s: &str| parse(s).unwrap().eval(&b, Some((DAY, 3 * DAY))).unwrap();
        assert_eq!(v("last"), 110.0);
        assert!((v("chg(2d)") - (110.0 / 99.0 - 1.0) * 100.0).abs() < 1e-9);
        assert!((v("chg()") - 10.0).abs() < 1e-9, "command span: 110 -> 121");
        assert_eq!(v("high(10d)"), 121.0);
        assert!((v("from_high(10d)") - (110.0 / 121.0 - 1.0) * 100.0).abs() < 1e-9);
        assert!((v("dd(10d)") - (99.0 / 110.0 - 1.0) * 100.0).abs() < 1e-9);
        assert!((v("vol_ratio(2)") - 104.0 / 102.5).abs() < 1e-9);
        assert!(matches!(parse("ta.rsi($,14)").unwrap().func, Func::Expr(_)));
        assert!(parse("chg(5x)").is_err());
        assert!(parse("pe").is_err());
    }
}
