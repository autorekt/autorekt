//! Text candle chart: one line per bar, exact OHLCV numbers, then a bar picture on a shared scale.
//!
//! ```text
//! okx:BTC/USDT:USDT  1h  UTC  11-02 00:00..11:00  scale 67100..69500 (1 char = 80)
//! time   open   high   low    close  chg%   vol   |67100                69500|
//! 01:00  68005  68010  67150  67420  -0.86  3.4k  |-:::::::::-             |
//! ```
//! Glyphs: `=` up body, `:` down body, `-` wick, `|` open == close.

use super::*;
use crate::model::{Bar, Event};

pub fn atr(bars: &[Bar], n: usize) -> Option<f64> {
    if bars.len() < n + 1 {
        return None;
    }
    let trs: Vec<f64> = bars
        .windows(2)
        .map(|w| {
            let (p, b) = (&w[0], &w[1]);
            (b.high - b.low).max((b.high - p.close).abs()).max((b.low - p.close).abs())
        })
        .collect();
    let tail = &trs[trs.len() - n..];
    Some(mean(tail))
}

fn bar_picture(b: &Bar, lo: f64, hi: f64, w: usize, ascii: bool) -> String {
    let col = |p: f64| -> usize {
        if hi <= lo {
            return w / 2;
        }
        (((p - lo) / (hi - lo)) * (w - 1) as f64).round().clamp(0.0, (w - 1) as f64) as usize
    };
    let mut cells = vec![' '; w];
    let (l, h) = (col(b.low), col(b.high));
    for c in cells.iter_mut().take(h + 1).skip(l) {
        *c = '-';
    }
    let (o, c) = (col(b.open), col(b.close));
    let (a, z) = (o.min(c), o.max(c));
    let body = if b.close > b.open {
        if ascii { '=' } else { '█' }
    } else if b.close < b.open {
        if ascii { ':' } else { '░' }
    } else {
        '|'
    };
    for cell in cells.iter_mut().take(z + 1).skip(a) {
        *cell = body;
    }
    if a == z && b.close != b.open {
        cells[a] = body;
    }
    cells.into_iter().collect()
}

pub struct CandleView<'a> {
    pub title: String,
    pub tf_label: String,
    pub bars: &'a [Bar],
    pub step_ms: i64,
    pub notes: Vec<String>,
    pub events: &'a [Event],
}

pub fn render(v: &CandleView, o: &Opts) -> String {
    let bars = v.bars;
    if bars.is_empty() {
        return format!("{}  {}  no data\n", v.title, v.tf_label);
    }
    let lo = bars.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
    let hi = bars.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
    let dec = decimals_for(hi.max(lo.abs()));
    let w = o.width.max(10);
    let span = o.span(bars[0].timestamp, bars.last().unwrap().timestamp);
    let mut out = String::new();
    let char_val = if w > 1 { (hi - lo) / (w - 1) as f64 } else { 0.0 };
    out.push_str(&format!(
        "{}  {}  {}  {}..{}  scale {}..{} (1 char = {})",
        v.title,
        v.tf_label,
        o.tz_name(),
        o.time_label(bars[0].timestamp, v.step_ms, 1),
        o.time_label(bars.last().unwrap().timestamp, v.step_ms, 1),
        fmt_price(lo, dec),
        fmt_price(hi, dec),
        fmt_price(char_val, decimals_for(char_val).max(dec)),
    ));
    if !v.notes.is_empty() {
        out.push_str(&format!("  [{}]", v.notes.join(", ")));
    }
    out.push('\n');
    let show_vol = bars.iter().any(|b| b.volume > 0.0);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut head = vec!["time".to_string(), "open".into(), "high".into(), "low".into(), "close".into(), "chg%".into()];
    if show_vol {
        head.push("vol".into());
    }
    let scale_hdr = {
        let l = fmt_price(lo, dec);
        let h = fmt_price(hi, dec);
        let gap = w.saturating_sub(l.len() + h.len());
        format!("|{l}{}{h}|", " ".repeat(gap))
    };
    head.push(scale_hdr);
    rows.push(head);
    // Events (news / rule firings) attached to the bar they fall in.
    let mut ev_by_bar: Vec<Vec<&Event>> = vec![Vec::new(); bars.len()];
    for e in v.events {
        if let Some(i) = bars.iter().rposition(|b| b.timestamp <= e.timestamp) {
            if e.timestamp < bars[i].timestamp + v.step_ms {
                ev_by_bar[i].push(e);
            }
        }
    }
    let mut prev_close: Option<f64> = None;
    for (i, b) in bars.iter().enumerate() {
        let base = prev_close.unwrap_or(b.open);
        let chg = pct(b.close, base);
        let mut r = vec![
            o.time_label(b.timestamp, v.step_ms, span) + if b.closed { "" } else { "*" },
            fmt_price(b.open, dec),
            fmt_price(b.high, dec),
            fmt_price(b.low, dec),
            fmt_price(b.close, dec),
            chg.map(|c| format!("{c:+.2}")).unwrap_or("-".into()),
        ];
        if show_vol {
            r.push(fmt_qty(b.volume));
        }
        let mut pic = format!("|{}|", bar_picture(b, lo, hi, w, o.ascii));
        if let Some(es) = ev_by_bar.get(i) {
            if !es.is_empty() {
                let t: Vec<String> = es.iter().take(2).map(|e| e.title.chars().take(60).collect()).collect();
                pic.push_str(&format!(" <- {}", t.join(" / ")));
            }
        }
        r.push(pic);
        rows.push(r);
        prev_close = Some(b.close);
    }
    out.push_str(&table(&rows));
    // Footer: summary stats.
    let first = &bars[0];
    let last = bars.last().unwrap();
    let total = pct(last.close, first.open);
    let mut summary = format!("summary  chg {}  range {}..{} ({:.2}%)", fmt_pct(total), fmt_price(lo, dec), fmt_price(hi, dec), (hi / lo - 1.0) * 100.0);
    if let Some(a) = atr(bars, 14.min(bars.len().saturating_sub(1)).max(1)) {
        summary.push_str(&format!("  ATR({}) {} ({:.2}%)", 14.min(bars.len() - 1), fmt_price(a, dec), a / last.close * 100.0));
    }
    let vols: Vec<f64> = bars.iter().map(|b| b.volume).collect();
    if show_vol {
        let avg = mean(&vols[..vols.len().saturating_sub(1).max(1)]);
        if avg > 0.0 {
            summary.push_str(&format!("  last vol {:.1}x avg", last.volume / avg));
        }
    }
    let hi_bar = bars.iter().max_by(|a, b| a.high.partial_cmp(&b.high).unwrap()).unwrap();
    let lo_bar = bars.iter().min_by(|a, b| a.low.partial_cmp(&b.low).unwrap()).unwrap();
    summary.push_str(&format!("  high@{} low@{}", o.time_label(hi_bar.timestamp, v.step_ms, span), o.time_label(lo_bar.timestamp, v.step_ms, span)));
    out.push_str(&summary);
    out.push('\n');
    // Notable bars: biggest up, biggest down, biggest volume.
    if bars.len() >= 5 {
        let mut chgs: Vec<(usize, f64)> = Vec::new();
        for i in 0..bars.len() {
            let base = if i == 0 { bars[0].open } else { bars[i - 1].close };
            if let Some(c) = pct(bars[i].close, base) {
                chgs.push((i, c));
            }
        }
        let mut notes = Vec::new();
        if let Some((i, c)) = chgs.iter().cloned().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) {
            if c > 0.0 {
                notes.push(format!("{} largest gain {c:+.2}%", o.time_label(bars[i].timestamp, v.step_ms, span)));
            }
        }
        if let Some((i, c)) = chgs.iter().cloned().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) {
            if c < 0.0 {
                notes.push(format!("{} largest drop {c:+.2}%", o.time_label(bars[i].timestamp, v.step_ms, span)));
            }
        }
        if show_vol {
            let avg = mean(&vols);
            if let Some((i, b)) = bars.iter().enumerate().max_by(|a, b| a.1.volume.partial_cmp(&b.1.volume).unwrap()) {
                if avg > 0.0 && b.volume / avg >= 1.5 {
                    notes.push(format!("{} heaviest volume {:.1}x avg", o.time_label(b.timestamp, v.step_ms, span), b.volume / avg));
                    let _ = i;
                }
            }
        }
        if !notes.is_empty() {
            out.push_str(&format!("notable  {}\n", notes.join("  |  ")));
        }
    }
    if bars.iter().any(|b| !b.closed) {
        out.push_str("(* = bar still forming)\n");
    }
    out
}
