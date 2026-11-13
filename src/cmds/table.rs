//! `show IDS --with COLUMNS`: one row per id, one column per --with (see columns.rs). A column
//! starting with + is added to the board's usual columns instead of replacing them.

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

/// The columns asked for; with --from and no --with, the latest value and the change over the span.
fn wanted(a: &ShowArgs) -> Result<(Vec<Column>, bool)> {
    let specs: Vec<String> = a.with.iter().flat_map(|w| split_top(w)).collect();
    if specs.is_empty() && a.from.is_some() {
        let mut chg = columns::parse("chg()")?;
        chg.label = "chg".into();
        return Ok((vec![columns::parse("last")?, chg], false));
    }
    let added = !specs.is_empty() && specs.iter().all(|s| s.starts_with('+'));
    Ok((specs.iter().map(|s| columns::parse(s.trim_start_matches('+'))).collect::<Result<_>>()?, added))
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

/// Whether `show` prints a table of columns: for several ids, any --with or --from; for one id
/// (otherwise a chart), a column that is not an expression.
pub fn wanted_table(a: &ShowArgs, several: bool) -> bool {
    if several {
        return !a.with.is_empty() || a.from.is_some();
    }
    a.with.iter().flat_map(|w| split_top(w)).any(|s| !matches!(columns::parse(s.trim_start_matches('+')).map(|c| c.func), Ok(Func::Expr(_))))
}

/// Daily bars of an id or expression from `start` to `end`.
async fn daily(id: &str, start: i64, end: Option<i64>) -> Result<Vec<Bar>> {
    let r = Range { start: Some(start), end, limit: 5000 };
    if expr::is_expression(id) {
        let (g, v) = expr::evaluate(id, Timeframe::D1, r).await?;
        return Ok(g.into_iter().zip(v).filter(|(_, x)| x.is_finite()).map(|(t, x)| Bar::point(t, x)).collect());
    }
    providers::ohlcv(&ids::parse(id)?, Timeframe::D1, r).await
}

/// The latest value of an expression column for `id`.
async fn expr_value(spec: &str, id: &str, tf: Timeframe, end: Option<i64>) -> Result<Option<f64>> {
    let me = if expr::is_expression(id) { format!("({id})") } else { format!("\"{id}\"") };
    let (_, v) = expr::evaluate(&spec.replace('$', &me), tf, Range { start: None, end, limit: 300 }).await?;
    Ok(v.into_iter().rev().find(|x| x.is_finite()))
}

/// One row: the values of every column for `item` ("LABEL|id" or an id), or why it failed.
async fn row(item: &str, cols: &[Column], tf: Timeframe, end: Option<i64>, span: Option<(i64, i64)>) -> (String, Result<Vec<Option<f64>>>) {
    let (label, id) = match item.split_once('|') {
        Some((l, i)) => (l.trim().to_string(), i.trim().to_string()),
        None => (item.to_string(), item.to_string()),
    };
    let e = end.unwrap_or_else(now_ms);
    let start = cols.iter().map(|c| c.reach(e)).chain(span.map(|(a, _)| a - 10 * 86_400_000)).min().unwrap_or(e);
    let needs_bars = cols.iter().any(|c| !matches!(c.func, Func::Expr(_)));
    let bars = if needs_bars {
        match daily(&id, start, end).await {
            Ok(b) => b,
            Err(err) => return (label, Err(err)),
        }
    } else {
        vec![]
    };
    let mut vals = Vec::new();
    for c in cols {
        vals.push(match &c.func {
            Func::Expr(s) => match expr_value(s, &id, tf, end).await {
                Ok(v) => v,
                Err(err) => return (label, Err(err.context(c.label.clone()))),
            },
            _ => c.eval(&bars, span),
        });
    }
    (label, Ok(vals))
}

pub async fn table(items: &[String], name: &str, a: &ShowArgs, g: &Global, o: &Opts) -> Result<()> {
    let (cols, added) = wanted(a)?;
    let span = command_span(a)?;
    let end = match (&a.to, &a.at) {
        (Some(t), _) => Some(end_time(t)?),
        (None, Some(t)) => Some(parse_time(t)?),
        _ => None,
    };
    let tf = Timeframe::parse(a.interval.as_deref().unwrap_or("1d"))?;
    let rows = futures::future::join_all(items.iter().map(|i| row(i, &cols, tf, end, span))).await;
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
    let mut t = vec![std::iter::once("id".to_string()).chain(cols.iter().map(|c| c.label.clone())).collect::<Vec<_>>()];
    let mut missing = String::new();
    for ((label, r), c) in rows.iter().zip(cells) {
        match r {
            Ok(_) => t.push(std::iter::once(label.clone()).chain(c).collect()),
            Err(e) => missing.push_str(&format!("missing  {label}: {}\n", format!("{e:#}").lines().next().unwrap_or(""))),
        }
    }
    let scope = match (span, end) {
        (Some((s, e)), _) => format!("{} to {}", o.dt(s).format("%Y-%m-%d"), o.dt(e).format("%Y-%m-%d")),
        (None, Some(e)) => format!("as of {}", o.dt(e).format("%Y-%m-%d %H:%M")),
        (None, None) => o.full_time(now_ms()),
    };
    println!("table {name}  {scope} {}  (from daily closes; changes and distances in %)", o.tz_name());
    print!("{}{missing}", render::table(&t));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_split_outside_parentheses() {
        assert_eq!(split_top("chg(5d),last, ta.rsi($,14)"), vec!["chg(5d)", "last", "ta.rsi($,14)"]);
    }
}
