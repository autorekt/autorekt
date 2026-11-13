//! `show IDS --with COLUMNS`: one row per id, one column per --with (see columns.rs). A column
//! starting with + is added to the board's usual columns instead of replacing them. --where
//! keeps rows whose columns pass conditions, --sort orders them, --top/--bottom keep the ends.

use super::*;
use crate::cli::ShowArgs;
use crate::columns::{self, Column, Func};

/// A time given as an end: a bare date means the end of that day.
pub fn end_time(s: &str) -> Result<i64> {
    let t = parse_time(s)?;
    Ok(if chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok() { t + 86_400_000 - 1 } else { t })
}

/// The command's --from/--to span, if given.
pub fn command_span(a: &ShowArgs) -> Result<Option<(i64, i64)>> {
    match (&a.from, &a.to) {
        (None, None) => Ok(None),
        (from, to) => {
            let end = to.as_deref().map(end_time).transpose()?.unwrap_or_else(now_ms);
            let start = from.as_deref().map(parse_time).transpose()?.ok_or_else(|| anyhow::anyhow!("--to needs --from"))?;
            Ok(Some((start, end)))
        }
    }
}

/// Columns of a table with no --with.
const DEFAULT: &[&str] = &["last", "chg(1d)", "chg(5d)", "chg(1M)", "chg(ytd)", "from_high(52w)"];

/// The columns asked for; with --from and no --with, the latest value and the change over the
/// span; with no --with, `default` (or DEFAULT).
fn wanted(a: &ShowArgs, default: &[&str]) -> Result<(Vec<Column>, bool)> {
    let specs: Vec<String> = a.with.iter().flat_map(|w| split_top(w)).collect();
    if specs.is_empty() && a.from.is_some() {
        let mut chg = columns::parse("chg()")?;
        chg.label = "chg".into();
        return Ok((vec![columns::parse("last")?, chg], false));
    }
    if specs.is_empty() {
        let d = if default.is_empty() { DEFAULT } else { default };
        return Ok((d.iter().map(|s| columns::parse(s)).collect::<Result<_>>()?, false));
    }
    let added = specs.iter().all(|s| s.starts_with('+'));
    Ok((specs.iter().map(|s| columns::parse(s.trim_start_matches('+'))).collect::<Result<_>>()?, added))
}

/// One --where condition: a column (or `id`) compared with a number (or, for `id`, a string).
#[derive(Debug)]
struct Cond {
    col: String,
    op: String,
    num: f64,
    text: String,
}

/// "dy > 6 and pb < 0.8 or id == \"0788.HK\"": alternatives of conjunctions.
fn parse_where(s: &str) -> Result<Vec<Vec<Cond>>> {
    s.split(" or ")
        .map(|alt| {
            alt.split(" and ")
                .map(|c| {
                    let c = c.trim();
                    let mut depth = 0;
                    let pos = c.char_indices().find_map(|(i, ch)| {
                        match ch {
                            '(' => depth += 1,
                            ')' => depth -= 1,
                            _ => {}
                        }
                        (depth == 0 && "<>=!".contains(ch)).then_some(i)
                    });
                    let i = pos.ok_or_else(|| anyhow::anyhow!("--where: no comparison in '{c}' (use > < >= <= == !=)"))?;
                    let op_len = if c[i + 1..].starts_with('=') { 2 } else { 1 };
                    let (col, op, rhs) = (c[..i].trim(), &c[i..i + op_len], c[i + op_len..].trim());
                    if !matches!(op, ">" | "<" | ">=" | "<=" | "==" | "!=") {
                        bail!("--where: unknown comparison '{op}' in '{c}'");
                    }
                    let text = rhs.trim_matches(['"', '\'']).to_string();
                    let num = if col == "id" { f64::NAN } else { rhs.parse().map_err(|_| anyhow::anyhow!("--where: '{rhs}' in '{c}' is not a number"))? };
                    Ok(Cond { col: col.to_string(), op: op.to_string(), num, text })
                })
                .collect()
        })
        .collect()
}

fn passes(c: &Cond, label: &str, v: Option<f64>) -> bool {
    if c.col == "id" {
        return match c.op.as_str() {
            "==" => label == c.text,
            "!=" => label != c.text,
            _ => false,
        };
    }
    let Some(v) = v.filter(|v| v.is_finite()) else { return false };
    match c.op.as_str() {
        ">" => v > c.num,
        "<" => v < c.num,
        ">=" => v >= c.num,
        "<=" => v <= c.num,
        "==" => v == c.num,
        _ => v != c.num,
    }
}

/// Split "chg(5d),pe" at commas outside parentheses.
fn split_top(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0);
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur).trim().to_string());
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Whether `show` prints a table of columns: --view table, --where, --sort, --top or --bottom;
/// for several ids also any --with or --from; for one id (otherwise a chart), a column that is
/// not an expression.
pub fn wanted_table(a: &ShowArgs, several: bool) -> bool {
    if a.view.as_deref() == Some("table") || a.filter.is_some() || a.sort.is_some() || a.top.is_some() || a.bottom.is_some() {
        return true;
    }
    if several {
        return !a.with.is_empty() || a.from.is_some();
    }
    a.with.iter().flat_map(|w| split_top(w)).any(|s| !matches!(columns::parse(s.trim_start_matches('+')).map(|c| c.func), Ok(Func::Expr(_))))
}

/// The id whose closes include dividends and splits (Yahoo ids only), for --adjust.
pub fn adjusted(id: &str) -> Result<String> {
    let parsed = (!expr::is_expression(id)).then(|| ids::parse(id)).transpose()?;
    match parsed {
        Some(p) if p.provider == "yahoo" && p.metric.is_none() => Ok(format!("{p}#adj_close")),
        _ => bail!("--adjust: dividend-adjusted prices come from Yahoo ids only, not '{id}'"),
    }
}

/// Daily bars of an id or expression from `start` to `end`.
async fn daily(id: &str, start: i64, end: Option<i64>) -> Result<Vec<Bar>> {
    let r = Range { start: Some(start), end, limit: 10000 };
    if expr::is_expression(id) {
        let (g, v) = expr::evaluate(id, Timeframe::D1, r).await?;
        return Ok(g.into_iter().zip(v).filter(|(_, x)| x.is_finite()).map(|(t, x)| Bar::point(t, x)).collect());
    }
    providers::ohlcv(&ids::parse(id)?, Timeframe::D1, r).await
}

/// The latest value of a fundamental of `id`.
async fn fundamental(id: &str, metric: &str) -> Result<Option<f64>> {
    if expr::is_expression(id) {
        bail!("{metric} is a company figure; '{id}' is an expression");
    }
    let mid = ids::parse(&format!("{}#{metric}", ids::parse(id)?))?;
    Ok(providers::ohlcv(&mid, Timeframe::D1, Range::last(5)).await?.last().map(|b| b.close))
}

/// The latest value of an expression column for `id`.
async fn expr_value(spec: &str, id: &str, tf: Timeframe, end: Option<i64>) -> Result<Option<f64>> {
    let me = if expr::is_expression(id) { format!("({id})") } else { format!("\"{id}\"") };
    let (_, v) = expr::evaluate(&spec.replace('$', &me), tf, Range { start: None, end, limit: 300 }).await?;
    Ok(v.into_iter().rev().find(|x| x.is_finite()))
}

/// What every row of a table shares: the end, the span, the period of statistics, how far back
/// statistics look, whether prices are adjusted, and the benchmarks' bars.
struct Ctx {
    tf: Timeframe,
    end: Option<i64>,
    span: Option<(i64, i64)>,
    stats_from: i64,
    adjust: bool,
    benches: std::collections::HashMap<String, Vec<Bar>>,
    /// Values a set's list already gives, per item and column label.
    known: std::collections::HashMap<String, std::collections::HashMap<String, f64>>,
}

/// One row: the values of every column for `item` ("LABEL|id" or an id), or why it failed.
async fn row(item: &str, cols: &[Column], ctx: &Ctx) -> (String, Result<Vec<Option<f64>>>) {
    let (label, id) = match item.split_once('|') {
        Some((l, i)) => (l.trim().to_string(), i.trim().to_string()),
        None => (item.to_string(), item.to_string()),
    };
    if let Some(k) = ctx.known.get(item).filter(|k| ctx.end.is_none() && ctx.span.is_none() && cols.iter().all(|c| k.contains_key(&c.label))) {
        return (label, Ok(cols.iter().map(|c| k.get(&c.label).copied()).collect()));
    }
    let e = ctx.end.unwrap_or_else(now_ms);
    let stats = cols.iter().any(|c| c.bench().is_some());
    let start = cols.iter().map(|c| c.reach(e)).chain(ctx.span.map(|(a, _)| a - 10 * 86_400_000)).chain(stats.then_some(ctx.stats_from)).min().unwrap_or(e);
    let needs_bars = cols.iter().any(|c| !matches!(c.func, Func::Expr(_) | Func::Metric(_) | Func::Lists));
    let source = if ctx.adjust && needs_bars {
        match adjusted(&id) {
            Ok(a) => a,
            Err(err) => return (label, Err(err)),
        }
    } else {
        id.clone()
    };
    let bars = if needs_bars {
        match daily(&source, start, ctx.end).await {
            Ok(b) => b,
            Err(err) => return (label, Err(err)),
        }
    } else {
        vec![]
    };
    let flows = if cols.iter().any(|c| matches!(c.func, Func::Flow(_))) {
        match providers::ohlcv(&match ids::parse(&format!("{id}#main_flow")) {
            Ok(i) => i,
            Err(err) => return (label, Err(err)),
        }, Timeframe::D1, Range::last(150)).await {
            Ok(f) => f,
            Err(err) => return (label, Err(err.context("flow() needs an Eastmoney id, e.g. eastmoney:90.BK0475"))),
        }
    } else {
        vec![]
    };
    let (tf, end, span) = (ctx.tf, ctx.end, ctx.span);
    let mut vals = Vec::new();
    for c in cols {
        vals.push(match &c.func {
            Func::Expr(s) => match expr_value(s, &id, tf, end).await {
                Ok(v) => v,
                Err(err) => return (label, Err(err.context(c.label.clone()))),
            },
            _ if c.bench().is_some() => {
                let own: Vec<Bar> = bars.iter().filter(|b| b.timestamp >= ctx.stats_from).cloned().collect();
                ctx.benches.get(c.bench().unwrap()).and_then(|b| c.eval_vs(&own, b, tf.ms()))
            }
            Func::Flow(_) => bars.last().and_then(|b| c.eval_flow(&flows, b.timestamp, span)),
            Func::Lists => match crate::company::lists(&id).await {
                Ok(v) => Some(v),
                Err(err) => return (label, Err(err.context("lists"))),
            },
            Func::Metric(m) => match fundamental(&id, m).await {
                Ok(v) => v,
                Err(err) => return (label, Err(err.context(c.label.clone()))),
            },
            _ => c.eval(&bars, span),
        });
    }
    (label, Ok(vals))
}

pub async fn table(items: &[String], name: &str, a: &ShowArgs, g: &Global, o: &Opts) -> Result<()> {
    let mut known = std::collections::HashMap::new();
    let mut items = items.to_vec();
    let mut default = vec![];
    if items.len() == 1 && crate::sets::is_set(&items[0]) {
        let members = crate::sets::members(&items[0]).await?;
        items = members.iter().map(|m| m.item.clone()).collect();
        known = members.into_iter().map(|m| (m.item, m.values)).collect();
        default = crate::sets::default_columns();
    }
    let items = &items[..];
    let (mut cols, added) = wanted(a, &default)?;
    if added && (a.filter.is_some() || a.sort.is_some() || a.top.is_some() || a.bottom.is_some()) {
        bail!("--where, --sort, --top and --bottom work on a table of your columns: drop the + from --with");
    }
    let shown = cols.len();
    let filter = a.filter.as_deref().map(parse_where).transpose()?.unwrap_or_default();
    let sort = a.sort.as_deref().or(if a.top.is_some() || a.bottom.is_some() { Some(cols[0].label.as_str()) } else { None }).map(|s| (s.trim_start_matches('-').to_string(), s.starts_with('-')));
    // Columns used only by --where and --sort are computed but not shown.
    for name in filter.iter().flatten().map(|c| c.col.clone()).filter(|c| c != "id").chain(sort.iter().map(|(c, _)| c.clone())) {
        if !cols.iter().any(|c| c.label == name) {
            cols.push(columns::parse(&name)?);
        }
    }
    let span = command_span(a)?;
    let end = match (&a.to, &a.at) {
        (Some(t), _) => Some(end_time(t)?),
        (None, Some(t)) => Some(parse_time(t)?),
        _ => None,
    };
    let tf = Timeframe::parse(a.interval.as_deref().unwrap_or("1d"))?;
    // Statistics columns name their benchmark, or use --vs.
    for c in cols.iter_mut() {
        if c.bench() == Some("") {
            let b = a.vs.clone().ok_or_else(|| anyhow::anyhow!("{} needs a benchmark: write it inside, e.g. beta(SPY), or give --vs SPY", c.label))?;
            c.func = columns::parse(&format!("{}({b})", c.label.trim_end_matches("()")))?.func;
        }
    }
    let stats_from = end.unwrap_or_else(now_ms) - columns::length_ms(a.range.as_deref().unwrap_or("1y"))?;
    let mut benches = std::collections::HashMap::new();
    for b in cols.iter().filter_map(|c| c.bench()).map(|b| b.to_string()).collect::<std::collections::BTreeSet<_>>() {
        let source = if a.adjust { adjusted(&b)? } else { b.clone() };
        let bars = daily(&source, stats_from - 40 * 86_400_000, end).await.with_context(|| format!("benchmark {b}"))?;
        benches.insert(b, bars.into_iter().filter(|x| x.timestamp >= stats_from).collect::<Vec<_>>());
    }
    let ctx = Ctx { tf, end, span, stats_from, adjust: a.adjust, benches, known };
    let mut rows = futures::future::join_all(items.iter().map(|i| row(i, &cols, &ctx))).await;
    let col_of = |name: &str| cols.iter().position(|c| c.label == name);
    if !filter.is_empty() {
        rows.retain(|(label, r)| match r {
            Ok(vals) => filter.iter().any(|alt| alt.iter().all(|c| passes(c, label, col_of(&c.col).and_then(|i| vals[i])))),
            Err(_) => false,
        });
    }
    if let Some((name, asc)) = &sort {
        let i = col_of(name).unwrap_or(0);
        let key = |r: &Result<Vec<Option<f64>>>| r.as_ref().ok().and_then(|v| v[i]).filter(|x| x.is_finite());
        rows.sort_by(|x, y| match (key(&x.1), key(&y.1)) {
            (Some(p), Some(q)) => if *asc { p.total_cmp(&q) } else { q.total_cmp(&p) },
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
    }
    let n = rows.len();
    let (top, bottom) = (a.top.unwrap_or(0), a.bottom.unwrap_or(0));
    let cut = (top > 0 || bottom > 0) && top + bottom < n;
    if cut {
        rows = rows.into_iter().enumerate().filter(|(i, _)| *i < top || *i >= n - bottom).map(|(_, r)| r).collect();
    }
    cols.truncate(shown);
    for (_, r) in rows.iter_mut() {
        if let Ok(v) = r {
            v.truncate(shown);
        }
    }
    if g.json {
        let v: Vec<_> = rows
            .iter()
            .map(|(label, r)| match r {
                Ok(vals) => json!({"id": label, "values": cols.iter().zip(vals).map(|(c, v)| (c.label.clone(), json!(v))).collect::<serde_json::Map<_, _>>()}),
                Err(e) => json!({"id": label, "error": format!("{e:#}")}),
            })
            .collect();
        return print_json(&v);
    }
    let cells: Vec<Vec<String>> = rows.iter().map(|(_, r)| match r {
        Ok(vals) => cols.iter().zip(vals).map(|(c, v)| c.fmt(*v)).collect(),
        Err(_) => cols.iter().map(|_| "-".to_string()).collect(),
    }).collect();
    if added {
        let mut board_rows = board::build(items).await;
        for (r, (_, res)) in board_rows.iter_mut().zip(&rows) {
            if let (None, Err(e)) = (&r.error, res) {
                r.error = Some(format!("{e:#}"));
            }
        }
        let headers: Vec<String> = cols.iter().map(|c| c.label.clone()).collect();
        print!("{}", board::render_with(name, &board_rows, &headers, &cells, o));
        return Ok(());
    }
    if g.csv {
        println!("id,{}", cols.iter().map(|c| format!("\"{}\"", c.label.replace('"', "\"\""))).collect::<Vec<_>>().join(","));
        for (label, r) in &rows {
            if let Ok(vals) = r {
                println!("{label},{}", vals.iter().map(|v| v.filter(|x| x.is_finite()).map(|x| x.to_string()).unwrap_or_default()).collect::<Vec<_>>().join(","));
            }
        }
        return Ok(());
    }
    let mut t = vec![std::iter::once("id".to_string()).chain(cols.iter().map(|c| c.label.clone())).collect::<Vec<_>>()];
    let mut missing = String::new();
    for (i, ((label, r), c)) in rows.iter().zip(cells).enumerate() {
        if cut && top > 0 && bottom > 0 && i == top {
            t.push(std::iter::once("...".to_string()).chain(cols.iter().map(|_| String::new())).collect());
        }
        match r {
            Ok(_) => t.push(std::iter::once(label.clone()).chain(c).collect()),
            Err(e) => missing.push_str(&format!("missing  {label}: {}\n", format!("{e:#}").lines().next().unwrap_or(""))),
        }
    }
    let shown_of = if cut || !filter.is_empty() { format!("  {} of {} rows", rows.len(), items.len()) } else { String::new() };
    let scope = match (span, end) {
        (Some((s, e)), _) => format!("{} to {}", o.dt(s).format("%Y-%m-%d"), o.dt(e).format("%Y-%m-%d")),
        (None, Some(e)) => format!("as of {}", o.dt(e).format("%Y-%m-%d %H:%M")),
        (None, None) => o.full_time(now_ms()),
    };
    let source = if crate::sets::is_set(name) { "Eastmoney list: changes in %, flows = net buying by large orders, CNY bn" } else { "from daily closes; changes and distances in %" };
    println!("table {name}  {scope} {}  ({source}){shown_of}", o.tz_name());
    print!("{}{missing}", render::table(&t));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn where_reads_alternatives_of_conditions() {
        let w = parse_where("chg(5d) > 2 and from_high(52w) >= -10 or id == \"SPY\"").unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!((w[0][0].col.as_str(), w[0][0].op.as_str(), w[0][0].num), ("chg(5d)", ">", 2.0));
        assert_eq!((w[0][1].col.as_str(), w[0][1].op.as_str()), ("from_high(52w)", ">="));
        assert!(passes(&w[1][0], "SPY", None) && !passes(&w[1][0], "QQQ", None));
        assert!(parse_where("dy 6").is_err());
    }

    #[test]
    fn columns_split_outside_parentheses() {
        assert_eq!(split_top("chg(5d),last, ta.rsi($,14)"), vec!["chg(5d)", "last", "ta.rsi($,14)"]);
    }
}
