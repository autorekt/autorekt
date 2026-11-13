//! `show FILE.md` for a journal (see journal.rs): the document with its data blocks replaced by
//! live tables, after a list of what needs attention: orders the price has reached or is near,
//! and dates within a week. Holdings show the latest price, value, gain and weight; --in CCY
//! converts values to one currency; --at replays the trades up to a date.

use super::*;
use crate::cli::ShowArgs;
use crate::journal::{self, Holding, Journal, Row};

/// A holding with its price now (in its own currency).
pub struct Live {
    pub h: Holding,
    pub last: Option<f64>,
}

/// An order with the price now and whether the price has reached its limit since it was placed.
pub struct OrderState {
    pub row: Row,
    pub last: Option<f64>,
    pub reached: Option<(i64, f64)>,
}

/// Dollars per unit of `ccy` is 1 / this: units of `ccy` per US dollar (Yahoo's XXX=X quotes).
async fn per_usd(ccy: &str) -> Result<f64> {
    if ccy == "USD" {
        return Ok(1.0);
    }
    let t = providers::ticker(&ids::parse(&format!("yahoo:{ccy}=X"))?).await.with_context(|| format!("exchange rate for {ccy}"))?;
    Ok(t.last)
}

/// Amount in `from` converted to `to`.
pub async fn convert(amount: f64, from: &str, to: &str) -> Result<f64> {
    if from == to {
        return Ok(amount);
    }
    Ok(amount / per_usd(from).await? * per_usd(to).await?)
}

fn is_currency(id: &str) -> bool {
    id.len() == 3 && id.chars().all(|c| c.is_ascii_uppercase())
}

pub async fn holdings(j: &Journal, at: Option<i64>) -> Result<(Vec<Live>, std::collections::BTreeMap<(String, String), f64>)> {
    let (hs, cash) = journal::book(j, at.unwrap_or(i64::MAX))?;
    let mut out = Vec::new();
    for h in hs {
        let last = if is_currency(&h.id) {
            Some(1.0)
        } else {
            match at {
                Some(t) => providers::ohlcv(&ids::parse(&h.id)?, Timeframe::D1, Range { start: Some(t - 10 * 86_400_000), end: Some(t), limit: 20 }).await?.last().map(|b| b.close),
                None => Some(providers::ticker(&ids::parse(&h.id)?).await.with_context(|| format!("price of {}", h.id))?.last),
            }
        };
        out.push(Live { h, last });
    }
    Ok((out, cash))
}

pub async fn orders(j: &Journal) -> Result<Vec<OrderState>> {
    let mut out = Vec::new();
    for r in &j.orders {
        let id = r.get("id").context("an order has no id")?;
        let limit: f64 = r.get("limit").and_then(|v| v.parse().ok()).with_context(|| format!("order {}: no limit", r.get("ref").map_or("?", |s| s.as_str())))?;
        let placed = r.get("placed").map(|p| parse_time(p)).transpose()?.unwrap_or_else(|| now_ms() - 86_400_000);
        let iid = ids::parse(id)?;
        let bars = providers::ohlcv(&iid, Timeframe::D1, Range { start: Some(placed), end: None, limit: 400 }).await?;
        let sell = r.get("side").is_some_and(|s| s.eq_ignore_ascii_case("sell"));
        let reached = bars.iter().find(|b| if sell { b.high >= limit } else { b.low <= limit }).map(|b| (b.timestamp, if sell { b.high } else { b.low }));
        let last = providers::ticker(&iid).await.ok().map(|t| t.last);
        out.push(OrderState { row: r.clone(), last, reached });
    }
    Ok(out)
}

/// What needs a look: reached or near orders, and dates from 3 days ago to a week ahead.
pub fn attention(j: &Journal, os: &[OrderState]) -> Vec<String> {
    let mut out = Vec::new();
    for o in os {
        let name = o.row.get("ref").or(o.row.get("id")).cloned().unwrap_or_default();
        let limit: f64 = o.row.get("limit").and_then(|v| v.parse().ok()).unwrap_or(f64::NAN);
        if let Some((t, p)) = o.reached {
            out.push(format!("order {name}: the price reached {} on {} (limit {}); if it filled, add the trade and remove the order", render::fmt_auto(p), chrono::DateTime::from_timestamp_millis(t).unwrap_or_default().format("%Y-%m-%d"), render::fmt_auto(limit)));
        } else if let Some(l) = o.last.filter(|l| (l / limit - 1.0).abs() <= 0.01) {
            out.push(format!("order {name}: {} is within 1% of the limit {}", render::fmt_auto(l), render::fmt_auto(limit)));
        }
    }
    let now = now_ms();
    for d in &j.dates {
        if let Some(t) = d.get("date").and_then(|s| parse_time(s).ok()) {
            if t >= now - 3 * 86_400_000 && t <= now + 7 * 86_400_000 {
                out.push(format!("{}: {} {}", d.get("date").unwrap(), d.get("what").cloned().unwrap_or_default(), d.get("note").cloned().unwrap_or_default()).trim().to_string());
            }
        }
    }
    out
}

/// Tables for the data blocks: holdings (with cash), orders and dates. Without `amounts`,
/// quantities, costs and values are left out (for pages others will see).
pub async fn tables(j: &Journal, at: Option<i64>, in_ccy: Option<&str>, amounts: bool) -> Result<(Vec<Vec<String>>, Vec<Vec<String>>, Vec<Vec<String>>, Vec<String>)> {
    let (hs, cash) = holdings(j, at).await?;
    let os = orders(j).await?;
    let mut values = Vec::new();
    for l in &hs {
        let v = l.last.map(|p| p * l.h.qty).unwrap_or(f64::NAN);
        let shown = match in_ccy {
            Some(c) => convert(v, &l.h.ccy, c).await?,
            None => v,
        };
        values.push(shown);
    }
    let mut cash_rows = Vec::new();
    for ((acct, c), amt) in &cash {
        let shown = match in_ccy {
            Some(t) => convert(*amt, c, t).await?,
            None => *amt,
        };
        cash_rows.push((acct.clone(), c.clone(), *amt, shown));
    }
    let total: f64 = values.iter().filter(|v| v.is_finite()).sum::<f64>() + cash_rows.iter().map(|r| r.3).sum::<f64>();
    let weight = |v: f64| if in_ccy.is_some() && total > 0.0 { format!("{:.1}%", v / total * 100.0) } else { "-".into() };
    let ccy_of = |own: &str| in_ccy.unwrap_or(own).to_string();
    let mut h = vec![if amounts {
        ["account", "id", "qty", "cost", "last", "value", "ccy", "gain", "income", "weight"].map(String::from).to_vec()
    } else {
        ["account", "id", "last", "gain", "weight"].map(String::from).to_vec()
    }];
    for (l, v) in hs.iter().zip(&values) {
        let gain = l.last.filter(|_| l.h.cost > 0.0).map(|p| format!("{:+.1}%", (p / l.h.cost - 1.0) * 100.0)).unwrap_or("-".into());
        let last = l.last.map(render::fmt_auto).unwrap_or("-".into());
        h.push(if amounts {
            vec![l.h.account.clone(), l.h.id.clone(), render::fmt_auto(l.h.qty), if l.h.cost > 0.0 { render::fmt_auto(l.h.cost) } else { "-".into() }, last, render::fmt_auto(*v), ccy_of(&l.h.ccy), gain, if l.h.income != 0.0 { render::fmt_auto(l.h.income) } else { "-".into() }, weight(*v)]
        } else {
            vec![l.h.account.clone(), l.h.id.clone(), last, gain, weight(*v)]
        });
    }
    for (acct, c, amt, shown) in &cash_rows {
        h.push(if amounts {
            vec![acct.clone(), format!("cash {c}"), render::fmt_auto(*amt), "-".into(), "-".into(), render::fmt_auto(*shown), ccy_of(c), "-".into(), "-".into(), weight(*shown)]
        } else {
            vec![acct.clone(), format!("cash {c}"), "-".into(), "-".into(), weight(*shown)]
        });
    }
    if in_ccy.is_some() && amounts {
        h.push(vec!["".into(), "total".into(), "".into(), "".into(), "".into(), render::fmt_auto(total), in_ccy.unwrap().into(), "".into(), "".into(), "100%".into()]);
    }
    let mut o = vec![["ref", "side", "id", "qty", "limit", "last", "distance", "status"].map(String::from).to_vec()];
    for s in &os {
        let g = |k: &str| s.row.get(k).cloned().unwrap_or_default();
        let limit: f64 = g("limit").parse().unwrap_or(f64::NAN);
        let dist = s.last.map(|l| format!("{:+.1}%", (limit / l - 1.0) * 100.0)).unwrap_or("-".into());
        let status = if s.reached.is_some() { "reached" } else if s.last.is_some_and(|l| (l / limit - 1.0).abs() <= 0.01) { "near" } else { "open" };
        o.push(vec![g("ref"), g("side"), g("id"), if amounts { g("qty") } else { "-".into() }, g("limit"), s.last.map(render::fmt_auto).unwrap_or("-".into()), dist, status.into()]);
    }
    let mut d = vec![["date", "what", "note"].map(String::from).to_vec()];
    let mut rows: Vec<&Row> = j.dates.iter().collect();
    rows.sort_by_key(|r| r.get("date").cloned().unwrap_or_default());
    for r in rows {
        d.push(vec![r.get("date").cloned().unwrap_or_default(), r.get("what").cloned().unwrap_or_default(), r.get("note").cloned().unwrap_or_default()]);
    }
    Ok((h, o, d, attention(j, &os)))
}

/// The ids a journal holds or has orders for (`@this` in its show blocks).
pub fn ids_of(j: &Journal) -> Result<Vec<String>> {
    let (hs, _) = journal::book(j, i64::MAX)?;
    let mut v: Vec<String> = hs.into_iter().map(|h| h.id).filter(|i| !is_currency(i)).collect();
    for r in &j.orders {
        if let Some(i) = r.get("id") {
            if !v.contains(i) {
                v.push(i.clone());
            }
        }
    }
    Ok(v)
}

/// Whether a markdown document is a journal (has a data block).
pub fn is_journal(md: &str) -> bool {
    md.lines().any(|l| ["```trades", "```positions", "```orders", "```dates"].iter().any(|k| l.trim_start().starts_with(k)))
}

/// The document to publish: data blocks become Markdown tables (amounts only with `amounts`),
/// `@this` in show blocks becomes the journal's ids, and show lines that are not one chart
/// (tables) become a note, since a page shows charts.
pub async fn share_markdown(md: &str, in_ccy: Option<&str>, amounts: bool) -> Result<String> {
    let j = journal::parse(md)?;
    let (h, ord, d, att) = tables(&j, None, in_ccy, amounts).await?;
    let mt = |rows: &[Vec<String>]| {
        render::set_markdown(true);
        let t = render::table(rows);
        render::set_markdown(false);
        t
    };
    let this = ids_of(&j)?.join(" ");
    let mut out = String::new();
    if !att.is_empty() {
        out.push_str(&format!("**Needs attention**\n\n{}\n", att.iter().map(|x| format!("- {x}\n")).collect::<String>()));
    }
    let mut block: Option<String> = None;
    let mut holdings_shown = false;
    for line in md.lines() {
        let t = line.trim_start();
        if let Some(kind) = &block {
            if t.starts_with("```") {
                if kind == "show" {
                    out.push_str("```\n");
                }
                block = None;
            } else if kind == "show" && !t.trim().is_empty() {
                let cmd = t.trim().replace("@this", &this);
                let one_chart = crate::cli::parse_chart_line(&cmd).is_ok_and(|a| a.what.len() == 1 && a.with.is_empty() && !a.what[0].starts_with('@'));
                if one_chart {
                    out.push_str(&cmd);
                    out.push('\n');
                }
            }
            continue;
        }
        if let Some(info) = t.strip_prefix("```") {
            let kind = info.split_whitespace().next().unwrap_or("");
            match kind {
                "trades" | "positions" if !holdings_shown => {
                    holdings_shown = true;
                    out.push_str(&mt(&h));
                }
                "trades" | "positions" => {}
                "orders" => out.push_str(&mt(&ord)),
                "dates" => out.push_str(&mt(&d)),
                "show" => out.push_str("```show\n"),
                _ => {
                    out.push_str(line);
                    out.push('\n');
                    continue;
                }
            }
            block = Some(kind.to_string());
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok(out.replace("```show\n```\n", ""))
}

/// The journal as text: what needs attention, then the document with live tables.
pub async fn show(path: &str, md: &str, a: &ShowArgs, g: &Global, o: &Opts) -> Result<()> {
    let j = journal::parse(md).with_context(|| format!("reading {path}"))?;
    let at = a.at.as_deref().map(parse_time).transpose()?;
    let (h, ord, d, att) = tables(&j, at, a.in_ccy.as_deref(), true).await?;
    if g.json {
        return print_json(&json!({"attention": att, "holdings": h, "orders": ord, "dates": d}));
    }
    println!("journal {path}  {} {}{}", o.full_time(at.unwrap_or_else(now_ms)), o.tz_name(), a.in_ccy.as_deref().map(|c| format!("  values in {c}")).unwrap_or_default());
    if att.is_empty() {
        println!("needs attention: nothing\n");
    } else {
        println!("needs attention:");
        for x in &att {
            println!("  - {x}");
        }
        println!();
    }
    let this = ids_of(&j)?.join(" ");
    let mut block: Option<String> = None;
    let mut holdings_shown = false;
    for line in md.lines() {
        let t = line.trim_start();
        if let Some(kind) = &block {
            if t.starts_with("```") {
                block = None;
            } else if kind == "show" && !t.trim().is_empty() {
                let cmd = t.trim().replace("@this", &this);
                println!("[show] {cmd}");
                let args = crate::cli::parse_chart_line(&cmd)?;
                Box::pin(super::show(args, g, o)).await?;
            }
            continue;
        }
        if let Some(info) = t.strip_prefix("```") {
            let kind = info.split_whitespace().next().unwrap_or("");
            match kind {
                "trades" | "positions" if !holdings_shown => {
                    holdings_shown = true;
                    print!("{}", render::table(&h));
                }
                "trades" | "positions" => {}
                "orders" => print!("{}", render::table(&ord)),
                "dates" => print!("{}", render::table(&d)),
                "show" => {}
                _ => {
                    println!("{line}");
                    continue;
                }
            }
            block = Some(kind.to_string());
            continue;
        }
        println!("{line}");
    }
    Ok(())
}
