//! Text formats designed for agents: exact numbers first, a small picture for shape,
//! a header that states scope and a footer with the stats an agent would otherwise compute.

pub mod board;
pub mod candles;
pub mod series;

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether tables print as Markdown (`--md`).
static MARKDOWN: AtomicBool = AtomicBool::new(false);

pub fn set_markdown(on: bool) {
    MARKDOWN.store(on, Ordering::Relaxed);
}

#[derive(Clone, Copy)]
pub struct Opts {
    pub tz: Tz,
    pub width: usize,
    pub ascii: bool,
}

impl Opts {
    pub fn tz_name(&self) -> String {
        self.tz.name().to_string()
    }
    pub fn dt(&self, ms: i64) -> DateTime<Tz> {
        Utc.timestamp_millis_opt(ms).single().unwrap_or_default().with_timezone(&self.tz)
    }
    /// Timestamp label. `span_ms` is the span of the whole view: views within one calendar
    /// day show HH:MM, longer intraday views MM-DD HH:MM, daily+ bars a date. Callers pass
    /// the same span for every row so the format is consistent within a view.
    pub fn time_label(&self, ms: i64, step_ms: i64, span_ms: i64) -> String {
        let d = self.dt(ms);
        if step_ms >= 86_400_000 {
            d.format("%Y-%m-%d").to_string()
        } else if span_ms <= 0 {
            d.format("%H:%M").to_string()
        } else {
            d.format("%m-%d %H:%M").to_string()
        }
    }
    /// Span argument for `time_label`: 0 when first and last fall on the same local date.
    pub fn span(&self, first: i64, last: i64) -> i64 {
        if self.dt(first).date_naive() == self.dt(last).date_naive() {
            0
        } else {
            last - first
        }
    }
    pub fn full_time(&self, ms: i64) -> String {
        self.dt(ms).format("%Y-%m-%d %H:%M").to_string()
    }
}

pub fn parse_tz(s: &str) -> anyhow::Result<Tz> {
    let s = match s {
        "utc" | "UTC" | "Z" => "UTC",
        "et" | "ET" | "NY" | "ny" => "America/New_York",
        "ct" | "CT" => "America/Chicago",
        "london" => "Europe/London",
        "hk" | "HK" => "Asia/Hong_Kong",
        "cn" | "CN" | "beijing" | "shanghai" => "Asia/Shanghai",
        "tokyo" | "jp" | "JP" => "Asia/Tokyo",
        other => other,
    };
    s.parse::<Tz>().map_err(|_| anyhow::anyhow!("unknown time zone '{s}' (use an IANA name like America/New_York, or UTC)"))
}

/// Decimals that keep ~5 significant digits for a price of this magnitude.
pub fn decimals_for(x: f64) -> usize {
    let a = x.abs();
    if a == 0.0 || !a.is_finite() {
        return 2;
    }
    let mag = a.log10().floor() as i32;
    match mag {
        m if m >= 4 => 0,
        3 => 1,
        1 | 2 => 2,
        0 => 4,
        m => (4 - m).min(10) as usize,
    }
}

pub fn fmt_price(x: f64, dec: usize) -> String {
    if !x.is_finite() {
        return "-".into();
    }
    format!("{:.*}", dec, x)
}

pub fn fmt_auto(x: f64) -> String {
    fmt_price(x, decimals_for(x))
}

/// 1234567 → 1.23M
pub fn fmt_qty(x: f64) -> String {
    if !x.is_finite() {
        return "-".into();
    }
    let a = x.abs();
    let (v, s) = if a >= 1e12 {
        (x / 1e12, "T")
    } else if a >= 1e9 {
        (x / 1e9, "B")
    } else if a >= 1e6 {
        (x / 1e6, "M")
    } else if a >= 1e3 {
        (x / 1e3, "k")
    } else {
        return if a >= 100.0 { format!("{x:.0}") } else if a >= 1.0 { format!("{x:.1}") } else { format!("{x:.3}") };
    };
    if v.abs() >= 100.0 {
        format!("{v:.0}{s}")
    } else {
        format!("{v:.1}{s}")
    }
}

pub fn fmt_pct(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => format!("{v:+.2}%"),
        _ => "-".into(),
    }
}

pub fn pct(a: f64, b: f64) -> Option<f64> {
    (b != 0.0 && a.is_finite() && b.is_finite()).then(|| (a / b - 1.0) * 100.0)
}

pub fn spark(values: &[f64], ascii: bool) -> String {
    let glyphs: Vec<char> = if ascii { "_.-~=*#@".chars().collect() } else { "▁▂▃▄▅▆▇█".chars().collect() };
    let vals: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if vals.is_empty() {
        return String::new();
    }
    let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    values
        .iter()
        .map(|v| {
            if !v.is_finite() {
                ' '
            } else if hi == lo {
                glyphs[3]
            } else {
                let i = ((v - lo) / (hi - lo) * (glyphs.len() - 1) as f64).round() as usize;
                glyphs[i.min(glyphs.len() - 1)]
            }
        })
        .collect()
}

/// Down-sample a series to at most `n` points by taking the last value of each bucket.
pub fn squeeze(values: &[f64], n: usize) -> Vec<f64> {
    if values.len() <= n || n == 0 {
        return values.to_vec();
    }
    (0..n)
        .map(|i| {
            let end = ((i + 1) * values.len()) / n;
            values[end.saturating_sub(1)]
        })
        .collect()
}

pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.iter().sum::<f64>() / v.len() as f64
}

pub fn stdev(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// Pearson correlation of two equally long series.
pub fn correlation(xs: &[f64], ys: &[f64]) -> f64 {
    let (mx, my) = (mean(xs), mean(ys));
    let cov: f64 = xs.iter().zip(ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let vx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
    let vy: f64 = ys.iter().map(|y| (y - my).powi(2)).sum();
    cov / (vx * vy).sqrt()
}

/// z-score of the last value against the preceding ones.
pub fn zscore_last(v: &[f64]) -> Option<f64> {
    if v.len() < 8 {
        return None;
    }
    let (hist, last) = v.split_at(v.len() - 1);
    let s = stdev(hist);
    (s > 0.0 && s.is_finite()).then(|| (last[0] - mean(hist)) / s)
}

/// Pad/truncate to display width (chars).
pub fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.chars().take(w).collect()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

pub fn lpad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{s}", " ".repeat(w - n))
    }
}

/// Align a table: each row is a list of cells; numeric-looking columns are right-aligned.
/// With `--md` the first row is the header of a Markdown table.
pub fn table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    if MARKDOWN.load(Ordering::Relaxed) {
        return markdown_table(rows);
    }
    let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut widths = vec![0; ncol];
    let mut numeric = vec![true; ncol];
    for (ri, r) in rows.iter().enumerate() {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
            if ri > 0 && !c.is_empty() && c != "-" {
                let t = c.trim_start_matches(['+', '-']).trim_end_matches(['%', 'k', 'M', 'B', 'T', 'x']);
                if t.parse::<f64>().is_err() {
                    numeric[i] = false;
                }
            }
        }
    }
    let mut out = String::new();
    for r in rows {
        let mut line = String::new();
        for (i, c) in r.iter().enumerate() {
            let last = i + 1 == r.len();
            let cell = if numeric[i] && i > 0 { lpad(c, widths[i]) } else if last { c.clone() } else { pad(c, widths[i]) };
            line.push_str(&cell);
            if !last {
                line.push_str("  ");
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn markdown_table(rows: &[Vec<String>]) -> String {
    let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let line = |r: &[String]| {
        let cells: Vec<String> = (0..ncol).map(|i| r.get(i).map_or(String::new(), |c| c.replace('|', "\\|"))).collect();
        format!("| {} |\n", cells.join(" | "))
    };
    let mut out = line(&rows[0]);
    out.push_str(&format!("|{}\n", " --- |".repeat(ncol)));
    for r in &rows[1..] {
        out.push_str(&line(r));
    }
    out
}

/// A named column of values aligned with the bars of a view (NaN = no value).
pub struct Column {
    pub label: String,
    pub values: Vec<f64>,
}

impl Column {
    pub fn cell(&self, i: usize) -> String {
        match self.values.get(i) {
            Some(v) if v.is_finite() => fmt_auto(*v),
            _ => "-".into(),
        }
    }
}

/// Compact number for prose (messages): up to 6 significant digits, no trailing zeros.
pub fn fmt_sig(x: f64) -> String {
    if !x.is_finite() {
        return "-".into();
    }
    let s = fmt_auto(x);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}
