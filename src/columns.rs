//! Table columns for `show --with`: functions of a series' daily closes (the latest value, the
//! change over a window or between two dates, highs and lows, drawdown, volatility, volume
//! against its average), statistics against a benchmark (beta, correlation, upside and downside
//! capture), or the latest value of an expression where `$` is the series.

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
    /// Net buying by large orders summed over the window (Eastmoney ids, CNY bn).
    Flow(Window),
    /// Against a benchmark id ("" = the command's --vs), from returns per -i period.
    Beta(String),
    Corr(String),
    UpCap(String),
    DownCap(String),
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

/// A length such as 30d, 52w, 6M or 5y, in ms.
pub fn length_ms(s: &str) -> Result<i64> {
    match window(s)? {
        Window::Days(n) => Ok(n * DAY),
        _ => bail!("'{s}' is not a length (use 30d, 52w, 6M or 5y)"),
    }
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
        Some(("beta", b)) => Func::Beta(b.trim().to_string()),
        Some(("corr", b)) => Func::Corr(b.trim().to_string()),
        Some(("upcap", b)) => Func::UpCap(b.trim().to_string()),
        Some(("downcap", b)) => Func::DownCap(b.trim().to_string()),
        Some(("flow", w)) => Func::Flow(window(w)?),
        Some(("vol_ratio", n)) => Func::VolRatio(n.trim().parse().map_err(|_| anyhow::anyhow!("vol_ratio(N): N is a number of days, e.g. vol_ratio(20)"))?),
        _ if s.contains('$') => Func::Expr(s.to_string()),
        _ => bail!("unknown column '{s}': use last, chg(5d), chg(ytd), chg(2024-09-13..2024-10-07), high(52w), low(52w), from_high(52w), from_low(52w), dd(5y), vol(20d), vol_ratio(20), flow(5d), beta(SPY), corr(SPY), upcap(SPY), downcap(SPY), or an expression with $ such as 'ta.rsi($,14)'"),
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

/// The last close in each period of `period_ms` (1d, 1w or 1M), keyed by the period.
fn per_period(bars: &[Bar], period_ms: i64) -> std::collections::BTreeMap<i64, f64> {
    use chrono::Datelike;
    let key = |t: i64| {
        let d = chrono::DateTime::from_timestamp_millis(t).unwrap_or_default();
        match period_ms {
            p if p >= 28 * DAY => (d.year() * 100 + d.month() as i32) as i64,
            p if p >= 7 * DAY => (d.iso_week().year() * 100 + d.iso_week().week() as i32) as i64,
            _ => t / DAY,
        }
    };
    bars.iter().map(|b| (key(b.timestamp), b.close)).collect()
}

/// (benchmark, series) returns over the periods both have.
fn paired_returns(bars: &[Bar], bench: &[Bar], period_ms: i64) -> Vec<(f64, f64)> {
    let (a, b) = (per_period(bars, period_ms), per_period(bench, period_ms));
    let common: Vec<(f64, f64)> = a.iter().filter_map(|(k, v)| Some((*b.get(k)?, *v))).collect();
    common.windows(2).filter(|w| w[0].0 > 0.0 && w[0].1 > 0.0).map(|w| (w[1].0 / w[0].0 - 1.0, w[1].1 / w[0].1 - 1.0)).collect()
}

impl Column {
    /// The benchmark of a statistics column ("" for the command's --vs), if it is one.
    pub fn bench(&self) -> Option<&str> {
        match &self.func {
            Func::Beta(b) | Func::Corr(b) | Func::UpCap(b) | Func::DownCap(b) => Some(b.as_str()),
            _ => None,
        }
    }

    /// A statistics column from the series' and the benchmark's daily bars, per period.
    pub fn eval_vs(&self, bars: &[Bar], bench: &[Bar], period_ms: i64) -> Option<f64> {
        let p = paired_returns(bars, bench, period_ms);
        if p.len() < 3 {
            return None;
        }
        let (xs, ys): (Vec<f64>, Vec<f64>) = p.iter().cloned().unzip();
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let capture = |up: bool| {
            let side: Vec<&(f64, f64)> = p.iter().filter(|(x, _)| if up { *x > 0.0 } else { *x < 0.0 }).collect();
            let mx = side.iter().map(|(x, _)| x).sum::<f64>();
            (!side.is_empty() && mx != 0.0).then(|| side.iter().map(|(_, y)| y).sum::<f64>() / mx * 100.0)
        };
        match &self.func {
            Func::Beta(_) => {
                let (mx, my) = (mean(&xs), mean(&ys));
                let cov: f64 = p.iter().map(|(x, y)| (x - mx) * (y - my)).sum();
                let var: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
                (var > 0.0).then(|| cov / var)
            }
            Func::Corr(_) => Some(crate::render::correlation(&xs, &ys)),
            Func::UpCap(_) => capture(true),
            Func::DownCap(_) => capture(false),
            _ => None,
        }
    }

    /// How far back the daily bars must reach (ms) for this column, before `end`.
    pub fn reach(&self, end: i64) -> i64 {
        let w = match &self.func {
            Func::Chg(w) | Func::High(w) | Func::Low(w) | Func::FromHigh(w) | Func::FromLow(w) | Func::Dd(w) | Func::Vol(w) | Func::Flow(w) => w,
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
            Func::Flow(_) | Func::Expr(_) | Func::Beta(_) | Func::Corr(_) | Func::UpCap(_) | Func::DownCap(_) => None,
        }
    }

    /// A flow column: the sum of the daily flows (oldest first) in its window, ending at the
    /// last price bar.
    pub fn eval_flow(&self, flows: &[Bar], last: i64, command: Option<(i64, i64)>) -> Option<f64> {
        let Func::Flow(w) = &self.func else { return None };
        let (a, b) = span(w, last, command)?;
        let v: Vec<f64> = flows.iter().filter(|x| x.timestamp > a && x.timestamp <= b).map(|x| x.close).collect();
        (!v.is_empty()).then(|| v.iter().sum())
    }

    pub fn fmt(&self, v: Option<f64>) -> String {
        let Some(v) = v.filter(|v| v.is_finite()) else { return "-".into() };
        match &self.func {
            Func::Chg(_) | Func::FromHigh(_) | Func::FromLow(_) | Func::Dd(_) => format!("{v:+.2}%"),
            Func::Vol(_) | Func::UpCap(_) | Func::DownCap(_) => format!("{v:.0}%"),
            Func::Beta(_) | Func::Corr(_) => format!("{v:+.2}"),
            Func::Flow(_) => format!("{v:+.2}bn"),
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

    #[test]
    fn statistics_against_a_benchmark() {
        let bench = bars(&[100.0, 110.0, 99.0, 108.9, 98.01]);
        let half = bars(&[100.0, 105.0, 99.75, 104.7375, 99.500625]);
        let v = |s: &str| parse(s).unwrap().eval_vs(&half, &bench, DAY).unwrap();
        assert!((v("beta(B)") - 0.5).abs() < 0.02);
        assert!(v("corr(B)") > 0.99);
        assert!((v("upcap(B)") - 50.0).abs() < 1e-6);
        assert!((v("downcap(B)") - 50.0).abs() < 1e-6);
    }
}
