//! Expression language for rules and derived series. Pine Script flavoured:
//!
//! ```text
//! close("BTC", "1h") > ta.sma(close("BTC", "1h"), 50)
//! ta.crossover(BTC, ta.ema(BTC, 20))
//! fred:SOFR - fred:IORB > 0.05
//! pct(BTC, "1h") < -3 and VIX > 25
//! abs(ta.change(prob("polymarket:<slug>"), "6h")) >= 0.05
//! stale(VIX, "2h")
//! ```
//!
//! Bare ids (`BTC`, `fred:SOFR`, `BTC#funding_rate`) are series of closes at the evaluation
//! timeframe. Ids containing operator characters can be quoted: `"kalshi:KXFED-26DEC-H25"`.
//! Everything evaluates to a series on a shared time grid; comparisons yield 1/0. Series of
//! different frequency are aligned as-of (last known value).

use crate::ids::{self, InstrumentId};
use crate::model::{now_ms, Bar, Timeframe};
use crate::providers::{self, Range};
use anyhow::{anyhow, bail, Result};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------- lexer

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(String),
    LParen,
    RParen,
    Comma,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '^'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '/' | '^' | '#' | '=' | '@')
}

fn lex(src: &str) -> Result<Vec<Tok>> {
    let cs: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit()) {
            let s = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.' || cs[i] == 'e' || ((cs[i] == '-' || cs[i] == '+') && i > s && cs[i - 1] == 'e')) {
                i += 1;
            }
            // A number immediately followed by letters is an identifier (e.g. 000001.SS) — rare; keep simple.
            let n: f64 = cs[s..i].iter().collect::<String>().parse().map_err(|_| anyhow!("bad number at {s}"))?;
            out.push(Tok::Num(n));
            continue;
        }
        if c == '"' || c == '\'' {
            let q = c;
            i += 1;
            let s = i;
            while i < cs.len() && cs[i] != q {
                i += 1;
            }
            if i >= cs.len() {
                bail!("unterminated string");
            }
            out.push(Tok::Str(cs[s..i].iter().collect()));
            i += 1;
            continue;
        }
        if is_ident_start(c) {
            let s = i;
            while i < cs.len() && is_ident_char(cs[i]) {
                i += 1;
            }
            // Allow '-' inside provider-qualified ids such as kalshi:KXFED-26DEC-H25 when not spaced.
            let mut id: String = cs[s..i].iter().collect();
            while id.contains(':') && i + 1 < cs.len() && cs[i] == '-' && cs[i + 1].is_ascii_alphanumeric() {
                let s2 = i;
                i += 1;
                while i < cs.len() && is_ident_char(cs[i]) {
                    i += 1;
                }
                id.push_str(&cs[s2..i].iter().collect::<String>());
            }
            out.push(Tok::Ident(id));
            continue;
        }
        let two: String = cs[i..(i + 2).min(cs.len())].iter().collect();
        if ["<=", ">=", "==", "!=", "&&", "||"].contains(&two.as_str()) {
            out.push(Tok::Op(two));
            i += 2;
            continue;
        }
        match c {
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            ',' => out.push(Tok::Comma),
            '+' | '-' | '*' | '/' | '<' | '>' | '!' | '%' => out.push(Tok::Op(c.to_string())),
            _ => bail!("unexpected character '{c}' in expression"),
        }
        i += 1;
    }
    Ok(out)
}

// ---------------------------------------------------------------- parser

#[derive(Debug, Clone)]
pub enum Ast {
    Num(f64),
    Str(String),
    Ref(String),
    Call(String, Vec<Ast>),
    Unary(String, Box<Ast>),
    Bin(String, Box<Ast>, Box<Ast>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

fn prec(op: &str) -> Option<u8> {
    Some(match op {
        "or" | "||" => 1,
        "and" | "&&" => 2,
        "==" | "!=" => 3,
        "<" | ">" | "<=" | ">=" => 4,
        "+" | "-" => 5,
        "*" | "/" | "%" => 6,
        _ => return None,
    })
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn op_at(&self) -> Option<String> {
        match self.peek()? {
            Tok::Op(o) => Some(o.clone()),
            Tok::Ident(w) if w == "and" || w == "or" => Some(w.clone()),
            _ => None,
        }
    }
    fn expr(&mut self, min: u8) -> Result<Ast> {
        let mut lhs = self.unary()?;
        while let Some(op) = self.op_at() {
            let Some(p) = prec(&op) else { break };
            if p < min {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(p + 1)?;
            lhs = Ast::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
    fn unary(&mut self) -> Result<Ast> {
        match self.peek() {
            Some(Tok::Op(o)) if o == "-" || o == "!" => {
                let o = o.clone();
                self.pos += 1;
                Ok(Ast::Unary(o, Box::new(self.unary()?)))
            }
            Some(Tok::Ident(w)) if w == "not" => {
                self.pos += 1;
                Ok(Ast::Unary("!".into(), Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }
    fn primary(&mut self) -> Result<Ast> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Ast::Num(n)),
            Some(Tok::Str(s)) => Ok(Ast::Str(s)),
            Some(Tok::LParen) => {
                let e = self.expr(0)?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    _ => bail!("expected ')'"),
                }
            }
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.expr(0)?);
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                _ => bail!("expected ',' or ')' in call to {name}"),
                            }
                        }
                    } else {
                        self.pos += 1;
                    }
                    Ok(Ast::Call(name, args))
                } else if name == "true" {
                    Ok(Ast::Num(1.0))
                } else if name == "false" {
                    Ok(Ast::Num(0.0))
                } else {
                    Ok(Ast::Ref(name))
                }
            }
            t => bail!("unexpected {:?}", t),
        }
    }
}

pub fn parse(src: &str) -> Result<Ast> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0 };
    let e = p.expr(0)?;
    if p.pos < p.toks.len() {
        bail!("unexpected trailing input near token {}", p.pos);
    }
    Ok(e)
}

// ---------------------------------------------------------------- data needs

/// One series the expression needs: (id, timeframe, field).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Need {
    pub id: InstrumentId,
    pub tf_secs: i64,
}

const SERIES_FNS: &[&str] = &["close", "open", "high", "low", "volume", "value", "prob", "series", "hl2", "ohlc4"];

fn fn_name(n: &str) -> &str {
    n.strip_prefix("ta.").or_else(|| n.strip_prefix("math.")).unwrap_or(n)
}

fn arg_str(a: &Ast) -> Option<String> {
    match a {
        Ast::Str(s) => Some(s.clone()),
        Ast::Ref(s) => Some(s.clone()),
        _ => None,
    }
}

fn tf_arg(a: Option<&Ast>, default: Timeframe) -> Result<Timeframe> {
    match a {
        None => Ok(default),
        Some(x) => match arg_str(x) {
            Some(s) => Timeframe::parse(&s),
            None => bail!("timeframe must be a string like \"1h\""),
        },
    }
}

/// Metric-function sugar: funding_rate("BTC") → BTC#funding_rate.
const METRIC_FNS: &[&str] = &["funding_rate", "open_interest", "long_short_ratio", "top_long_short_ratio", "taker_buy_sell_ratio", "basis", "premium"];

fn ref_of(call: &str, args: &[Ast], tf: Timeframe) -> Result<Option<(InstrumentId, Timeframe)>> {
    let f = fn_name(call);
    if SERIES_FNS.contains(&f) || METRIC_FNS.contains(&f) || f == "stale" || f == "pct" || f == "atr" {
        let Some(first) = args.first() else { bail!("{call}() needs an instrument id") };
        let Some(s) = arg_str(first) else { return Ok(None) };
        let mut id = ids::parse(&s)?;
        if METRIC_FNS.contains(&f) {
            id.metric = Some(f.to_string());
        }
        if f == "prob" {
            if let Some(o) = args.get(1).and_then(arg_str) {
                id.metric = Some(o);
            }
            return Ok(Some((id, tf_arg(args.get(2), tf)?)));
        }
        let tfa = if matches!(f, "stale" | "pct") { None } else { args.get(1) };
        let tfa = if f == "atr" { args.get(1).filter(|a| matches!(a, Ast::Str(_))) } else { tfa };
        return Ok(Some((id, tf_arg(tfa, tf)?)));
    }
    Ok(None)
}

pub fn needs(ast: &Ast, tf: Timeframe, out: &mut BTreeSet<Need>) -> Result<()> {
    match ast {
        Ast::Num(_) => {}
        // A quoted id used as a value ("kalshi:KXFED-26DEC-H25" > 0.5). Quoted durations and
        // the extra arguments of series functions are skipped by the Call branch below.
        Ast::Str(name) | Ast::Ref(name) => {
            let id = ids::parse(name)?;
            out.insert(Need { id, tf_secs: tf.secs });
        }
        Ast::Call(name, args) => {
            if let Some((id, t)) = ref_of(name, args, tf)? {
                out.insert(Need { id, tf_secs: t.secs });
                // Other args may contain nested series.
                for a in args.iter().skip(1) {
                    if !matches!(a, Ast::Str(_) | Ast::Ref(_)) {
                        needs(a, tf, out)?;
                    }
                }
            } else {
                for a in args {
                    if !matches!(a, Ast::Str(s) if Timeframe::parse(s).is_ok()) {
                        needs(a, tf, out)?;
                    }
                }
            }
        }
        Ast::Unary(_, a) => needs(a, tf, out)?,
        Ast::Bin(_, a, b) => {
            needs(a, tf, out)?;
            needs(b, tf, out)?;
        }
    }
    Ok(())
}

/// Largest look-back (in bars of `tf`) the expression asks for, to size fetches.
pub fn lookback(ast: &Ast, tf: Timeframe) -> usize {
    match ast {
        Ast::Call(_, args) => {
            let mut m = 0;
            for a in args {
                match a {
                    Ast::Num(n) => m = m.max(*n as usize),
                    Ast::Str(s) | Ast::Ref(s) => {
                        if let Ok(d) = Timeframe::parse(s) {
                            m = m.max((d.secs / tf.secs.max(1)) as usize);
                        }
                    }
                    _ => {}
                }
                m = m.max(lookback(a, tf));
            }
            m * 2 + 5
        }
        Ast::Unary(_, a) => lookback(a, tf),
        Ast::Bin(_, a, b) => lookback(a, tf).max(lookback(b, tf)),
        _ => 0,
    }
}

// ---------------------------------------------------------------- evaluation

pub type Data = BTreeMap<Need, Vec<Bar>>;

/// Fetch everything an expression needs.
pub async fn fetch(ast: &Ast, tf: Timeframe, range: Range) -> Result<Data> {
    let mut ns = BTreeSet::new();
    needs(ast, tf, &mut ns)?;
    let lb = lookback(ast, tf);
    let futs = ns.iter().map(|n| {
        let n = n.clone();
        async move {
            let t = Timeframe { secs: n.tf_secs };
            // Extend the window to cover indicator look-back.
            let r = Range {
                start: range.start.map(|s| s - (lb as i64 + 2) * t.ms().max(tf.ms())),
                end: range.end,
                limit: (range.limit + lb + 5).min(5000),
            };
            let bars = providers::ohlcv(&n.id, t, r).await.map_err(|e| anyhow!("{}: {e}", n.id))?;
            Ok::<_, anyhow::Error>((n, bars))
        }
    });
    let mut out = BTreeMap::new();
    for r in futures::future::join_all(futs).await {
        let (n, b) = r?;
        out.insert(n, b);
    }
    Ok(out)
}

/// Evaluation context: a time grid plus the fetched bars.
pub struct Ctx<'a> {
    pub grid: Vec<i64>,
    pub tf: Timeframe,
    pub data: &'a Data,
}

type S = Vec<f64>;

impl<'a> Ctx<'a> {
    pub fn new(data: &'a Data, tf: Timeframe) -> Ctx<'a> {
        // Grid: union of bar times at the evaluation timeframe (finer series define it).
        let mut set = BTreeSet::new();
        for (n, bars) in data {
            if n.tf_secs == tf.secs {
                for b in bars {
                    set.insert(b.timestamp);
                }
            }
        }
        if set.is_empty() {
            for bars in data.values() {
                for b in bars {
                    set.insert(b.timestamp);
                }
            }
        }
        Ctx { grid: set.into_iter().collect(), tf, data }
    }

    fn field(&self, need: &Need, f: &str) -> Result<S> {
        let bars = self.data.get(need).ok_or_else(|| anyhow!("internal: missing data for {}", need.id))?;
        let ms = need.tf_secs * 1000;
        let mut out = Vec::with_capacity(self.grid.len());
        let mut j = 0usize;
        let mut cur: Option<&Bar> = None;
        for &t in &self.grid {
            // as-of: last bar whose period has started by the end of this grid bar
            while j < bars.len() && bars[j].timestamp <= t + self.tf.ms() - 1 {
                cur = Some(&bars[j]);
                j += 1;
            }
            let _ = ms;
            out.push(match cur {
                None => f64::NAN,
                Some(b) => match f {
                    "open" => b.open,
                    "high" => b.high,
                    "low" => b.low,
                    "volume" => b.volume,
                    "hl2" => (b.high + b.low) / 2.0,
                    "ohlc4" => (b.open + b.high + b.low + b.close) / 4.0,
                    _ => b.close,
                },
            });
        }
        Ok(out)
    }

    fn last_ts(&self, need: &Need) -> Option<i64> {
        self.data.get(need)?.last().map(|b| b.timestamp)
    }

    fn bars_arg(&self, a: Option<&Ast>, what: &str) -> Result<usize> {
        match a {
            Some(Ast::Num(n)) if *n >= 1.0 => Ok(*n as usize),
            Some(x) => match arg_str(x) {
                Some(s) => {
                    let d = Timeframe::parse(&s)?;
                    Ok(((d.secs / self.tf.secs.max(1)) as usize).max(1))
                }
                None => bail!("{what}: length must be a number of bars or a duration like \"6h\""),
            },
            None => bail!("{what}: missing length"),
        }
    }

    pub fn eval(&self, ast: &Ast) -> Result<S> {
        let n = self.grid.len();
        Ok(match ast {
            Ast::Num(x) => vec![*x; n],
            Ast::Str(s) | Ast::Ref(s) => {
                let id = ids::parse(s)?;
                self.field(&Need { id, tf_secs: self.tf.secs }, "close")?
            }
            Ast::Unary(op, a) => {
                let v = self.eval(a)?;
                match op.as_str() {
                    "-" => v.iter().map(|x| -x).collect(),
                    _ => v.iter().map(|x| b2f(!truthy(*x))).collect(),
                }
            }
            Ast::Bin(op, a, b) => {
                let (x, y) = (self.eval(a)?, self.eval(b)?);
                x.iter()
                    .zip(y.iter())
                    .map(|(&p, &q)| match op.as_str() {
                        "+" => p + q,
                        "-" => p - q,
                        "*" => p * q,
                        "/" => p / q,
                        "%" => p % q,
                        ">" => cmp(p, q, |a, b| a > b),
                        "<" => cmp(p, q, |a, b| a < b),
                        ">=" => cmp(p, q, |a, b| a >= b),
                        "<=" => cmp(p, q, |a, b| a <= b),
                        "==" => cmp(p, q, |a, b| a == b),
                        "!=" => cmp(p, q, |a, b| a != b),
                        "and" | "&&" => b2f(truthy(p) && truthy(q)),
                        "or" | "||" => b2f(truthy(p) || truthy(q)),
                        _ => f64::NAN,
                    })
                    .collect()
            }
            Ast::Call(name, args) => self.call(name, args)?,
        })
    }

    fn call(&self, name: &str, args: &[Ast]) -> Result<S> {
        let f = fn_name(name);
        let n = self.grid.len();
        if let Some((id, t)) = ref_of(name, args, self.tf)? {
            let need = Need { id: id.clone(), tf_secs: t.secs };
            return Ok(match f {
                "stale" => {
                    let d = args.get(1).and_then(arg_str).ok_or_else(|| anyhow!("stale(id, \"2h\")"))?;
                    let d = Timeframe::parse(&d)?.ms();
                    let last = self.last_ts(&need).unwrap_or(0);
                    let stale = now_ms() - (last + t.ms()) > d;
                    // Historical evaluation: stale where the as-of bar is older than d.
                    let bars = &self.data[&need];
                    let mut j = 0;
                    let mut cur: Option<i64> = None;
                    let mut out = Vec::with_capacity(n);
                    for (i, &g) in self.grid.iter().enumerate() {
                        while j < bars.len() && bars[j].timestamp <= g {
                            cur = Some(bars[j].timestamp);
                            j += 1;
                        }
                        let v = if i == n - 1 { stale } else { cur.map_or(true, |c| g - c > d) };
                        out.push(b2f(v));
                    }
                    out
                }
                "pct" => {
                    let d = args.get(1).and_then(arg_str).unwrap_or_else(|| "24h".into());
                    let k = ((Timeframe::parse(&d)?.secs / self.tf.secs.max(1)) as usize).max(1);
                    let c = self.field(&need, "close")?;
                    (0..n).map(|i| if i >= k { (c[i] / c[i - k] - 1.0) * 100.0 } else { f64::NAN }).collect()
                }
                "atr" => {
                    let len = self.bars_arg(args.iter().skip(1).find(|a| matches!(a, Ast::Num(_))).or(Some(&Ast::Num(14.0))), "atr")?;
                    let (h, l, c) = (self.field(&need, "high")?, self.field(&need, "low")?, self.field(&need, "close")?);
                    let tr: S = (0..n).map(|i| if i == 0 { h[i] - l[i] } else { (h[i] - l[i]).max((h[i] - c[i - 1]).abs()).max((l[i] - c[i - 1]).abs()) }).collect();
                    rma(&tr, len)
                }
                _ => self.field(&need, f)?,
            });
        }
        let a = |i: usize| -> Result<S> {
            let x = args.get(i).ok_or_else(|| anyhow!("{name}: missing argument {}", i + 1))?;
            self.eval(x)
        };
        Ok(match f {
            "abs" => a(0)?.iter().map(|x| x.abs()).collect(),
            "sqrt" => a(0)?.iter().map(|x| x.sqrt()).collect(),
            "log" => a(0)?.iter().map(|x| x.ln()).collect(),
            "min" => a(0)?.iter().zip(a(1)?).map(|(x, y)| x.min(y)).collect(),
            "max" => a(0)?.iter().zip(a(1)?).map(|(x, y)| x.max(y)).collect(),
            "nz" => {
                let d = if args.len() > 1 { a(1)? } else { vec![0.0; n] };
                a(0)?.iter().zip(d).map(|(x, y)| if x.is_finite() { *x } else { y }).collect()
            }
            "sma" => sma(&a(0)?, self.bars_arg(args.get(1), f)?),
            "ema" => ema(&a(0)?, self.bars_arg(args.get(1), f)?),
            "rma" => rma(&a(0)?, self.bars_arg(args.get(1), f)?),
            "wma" => wma(&a(0)?, self.bars_arg(args.get(1), f)?),
            "stdev" => roll(&a(0)?, self.bars_arg(args.get(1), f)?, |w| crate::render::stdev(w)),
            "highest" => roll(&a(0)?, self.bars_arg(args.get(1), f)?, |w| w.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
            "lowest" => roll(&a(0)?, self.bars_arg(args.get(1), f)?, |w| w.iter().cloned().fold(f64::INFINITY, f64::min)),
            "sum" => roll(&a(0)?, self.bars_arg(args.get(1), f)?, |w| w.iter().sum()),
            "zscore" => {
                let v = a(0)?;
                let k = self.bars_arg(args.get(1), f)?;
                let m = sma(&v, k);
                let s = roll(&v, k, |w| crate::render::stdev(w));
                (0..n).map(|i| (v[i] - m[i]) / s[i]).collect()
            }
            "change" => {
                let v = a(0)?;
                let k = if args.len() > 1 { self.bars_arg(args.get(1), f)? } else { 1 };
                (0..n).map(|i| if i >= k { v[i] - v[i - k] } else { f64::NAN }).collect()
            }
            "roc" => {
                let v = a(0)?;
                let k = if args.len() > 1 { self.bars_arg(args.get(1), f)? } else { 1 };
                (0..n).map(|i| if i >= k { (v[i] / v[i - k] - 1.0) * 100.0 } else { f64::NAN }).collect()
            }
            "rsi" => rsi(&a(0)?, self.bars_arg(args.get(1).or(Some(&Ast::Num(14.0))), f)?),
            "crossover" | "crossunder" => {
                let (x, y) = (a(0)?, a(1)?);
                (0..n)
                    .map(|i| {
                        if i == 0 {
                            return 0.0;
                        }
                        let up = x[i] > y[i] && x[i - 1] <= y[i - 1];
                        let dn = x[i] < y[i] && x[i - 1] >= y[i - 1];
                        b2f(if f == "crossover" { up } else { dn })
                    })
                    .collect()
            }
            "cross" => {
                let (x, y) = (a(0)?, a(1)?);
                (0..n).map(|i| b2f(i > 0 && ((x[i] > y[i]) != (x[i - 1] > y[i - 1])))).collect()
            }
            "rising" | "falling" => {
                let v = a(0)?;
                let k = self.bars_arg(args.get(1).or(Some(&Ast::Num(1.0))), f)?;
                (0..n)
                    .map(|i| {
                        if i < k {
                            return 0.0;
                        }
                        let ok = (i + 1 - k..=i).all(|j| if f == "rising" { v[j] > v[j - 1] } else { v[j] < v[j - 1] });
                        b2f(ok)
                    })
                    .collect()
            }
            "hour" => self.grid.iter().map(|t| ((t / 3_600_000) % 24) as f64).collect(),
            "dayofweek" => self.grid.iter().map(|t| (((t / 86_400_000) + 4) % 7) as f64).collect(),
            _ => bail!("unknown function '{name}'. Available: series close/open/high/low/volume/value/prob, metrics funding_rate/open_interest/long_short_ratio/..., pct, stale, atr, ta.sma ema rma wma rsi stdev highest lowest sum zscore change roc crossover crossunder cross rising falling, abs min max sqrt log nz, hour dayofweek"),
        })
    }
}

fn truthy(x: f64) -> bool {
    x.is_finite() && x != 0.0
}
fn b2f(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}
fn cmp(p: f64, q: f64, f: impl Fn(f64, f64) -> bool) -> f64 {
    if p.is_nan() || q.is_nan() {
        f64::NAN
    } else {
        b2f(f(p, q))
    }
}

fn roll(v: &[f64], k: usize, f: impl Fn(&[f64]) -> f64) -> S {
    (0..v.len())
        .map(|i| {
            if i + 1 < k {
                return f64::NAN;
            }
            let w = &v[i + 1 - k..=i];
            if w.iter().any(|x| x.is_nan()) {
                f64::NAN
            } else {
                f(w)
            }
        })
        .collect()
}

fn sma(v: &[f64], k: usize) -> S {
    roll(v, k, |w| w.iter().sum::<f64>() / w.len() as f64)
}

fn wma(v: &[f64], k: usize) -> S {
    let denom = (k * (k + 1) / 2) as f64;
    roll(v, k, |w| w.iter().enumerate().map(|(i, x)| x * (i + 1) as f64).sum::<f64>() / denom)
}

fn smooth(v: &[f64], alpha: f64, k: usize) -> S {
    let mut out = vec![f64::NAN; v.len()];
    let mut prev: Option<f64> = None;
    let mut seed: Vec<f64> = Vec::new();
    for i in 0..v.len() {
        let x = v[i];
        if x.is_nan() {
            out[i] = prev.unwrap_or(f64::NAN);
            continue;
        }
        match prev {
            None => {
                seed.push(x);
                if seed.len() == k {
                    let m = seed.iter().sum::<f64>() / k as f64;
                    prev = Some(m);
                    out[i] = m;
                }
            }
            Some(p) => {
                let m = alpha * x + (1.0 - alpha) * p;
                prev = Some(m);
                out[i] = m;
            }
        }
    }
    out
}

fn ema(v: &[f64], k: usize) -> S {
    smooth(v, 2.0 / (k as f64 + 1.0), k)
}
fn rma(v: &[f64], k: usize) -> S {
    smooth(v, 1.0 / k as f64, k)
}

fn rsi(v: &[f64], k: usize) -> S {
    let up: S = (0..v.len()).map(|i| if i == 0 { f64::NAN } else { (v[i] - v[i - 1]).max(0.0) }).collect();
    let dn: S = (0..v.len()).map(|i| if i == 0 { f64::NAN } else { (v[i - 1] - v[i]).max(0.0) }).collect();
    let (u, d) = (rma(&up, k), rma(&dn, k));
    (0..v.len()).map(|i| if d[i] == 0.0 { 100.0 } else { 100.0 - 100.0 / (1.0 + u[i] / d[i]) }).collect()
}

/// Parse, fetch and evaluate in one go; returns (grid, values).
pub async fn evaluate(src: &str, tf: Timeframe, range: Range) -> Result<(Vec<i64>, Vec<f64>)> {
    let ast = parse(src)?;
    let data = fetch(&ast, tf, range).await?;
    let ctx = Ctx::new(&data, tf);
    let v = ctx.eval(&ast)?;
    let mut grid = ctx.grid.clone();
    let mut vals = v;
    if let Some(s) = range.start {
        let keep: Vec<bool> = grid.iter().map(|t| *t >= s).collect();
        grid = grid.iter().zip(&keep).filter(|(_, k)| **k).map(|(t, _)| *t).collect();
        vals = vals.iter().zip(&keep).filter(|(_, k)| **k).map(|(v, _)| *v).collect();
    } else if grid.len() > range.limit {
        let cut = grid.len() - range.limit;
        grid.drain(..cut);
        vals.drain(..cut);
    }
    Ok((grid, vals))
}

/// True when a string looks like an expression rather than a single id.
pub fn is_expression(s: &str) -> bool {
    s.contains('(') || s.contains(" - ") || s.contains(" + ") || s.contains(" / ") || s.contains(" * ") || s.contains('>') || s.contains('<')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        assert!(parse("ta.crossover(close(\"BTC\", \"1h\"), ta.sma(close(\"BTC\",\"1h\"), 50))").is_ok());
        assert!(parse("fred:SOFR - fred:IORB > 0.05 and not stale(VIX, \"2h\")").is_ok());
        assert!(parse("abs(ta.change(prob(\"polymarket:x\"), \"6h\")) >= 0.05").is_ok());
        // Quoted ids as values are data needs; quoted durations inside calls are not.
        let mut ns = std::collections::BTreeSet::new();
        needs(&parse("\"BTC\" > 1 and ta.change(VIX, \"6h\") > 1 and ta.rsi(\"GOLD\", 14) > 50").unwrap(), Timeframe::H1, &mut ns).unwrap();
        let got: Vec<String> = ns.iter().map(|n| n.id.to_string()).collect();
        assert_eq!(got, vec!["okx:BTC/USDT:USDT".to_string(), "yahoo:GC=F".to_string(), "yahoo:^VIX".to_string()]);
        match parse("kalshi:KXFED-26DEC-H25 > 0.5").unwrap() {
            Ast::Bin(_, a, _) => assert!(matches!(*a, Ast::Ref(ref s) if s == "kalshi:KXFED-26DEC-H25")),
            _ => panic!(),
        }
    }
    #[test]
    fn indicators() {
        let v: Vec<f64> = (1..=20).map(|x| x as f64).collect();
        let s = sma(&v, 5);
        assert_eq!(s[4], 3.0);
        let r = rsi(&v, 14);
        assert_eq!(r[19], 100.0);
    }
}
