//! A journal is a markdown document whose typed code blocks hold an investor's records, read by
//! `show FILE.md` and `show @FILE.md`. Tables are pipe tables with a header row:
//!
//! - `trades`: date action id qty price ccy fee note, where action is buy, sell, div (price is
//!   the cash received), fx (id BASE.QUOTE, qty in BASE, price in QUOTE per BASE), deposit or
//!   withdraw (qty in ccy). Holdings, costs and cash follow from it.
//! - `positions`: id qty cost ccy, holdings from before the trades start.
//! - `orders`: ref side id qty limit placed, and optionally type stop target expires note.
//! - `dates`: date what note.
//!
//! A block's info line can set defaults for every row, e.g. ```` ```trades account=company ````.
//! Text outside the blocks is notes; a heading that names an id belongs to that id.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

pub type Row = BTreeMap<String, String>;

#[derive(Debug, Default, Clone)]
pub struct Journal {
    pub trades: Vec<Row>,
    pub positions: Vec<Row>,
    pub orders: Vec<Row>,
    pub dates: Vec<Row>,
    /// (heading, text) of each section, in order.
    pub notes: Vec<(String, String)>,
    /// Lines of the `show` blocks.
    pub views: Vec<String>,
}

const KINDS: &[&str] = &["trades", "positions", "orders", "dates"];

/// Rows of a pipe table; `defaults` fill empty or missing cells.
fn table(body: &str, defaults: &Row, where_: &str) -> Result<Vec<Row>> {
    let mut lines = body.lines().map(str::trim).filter(|l| l.starts_with('|'));
    let cells = |l: &str| l.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect::<Vec<_>>();
    let Some(head) = lines.next() else { return Ok(vec![]) };
    let head = cells(head);
    let mut out = Vec::new();
    for (n, l) in lines.enumerate() {
        let c = cells(l);
        if c.iter().all(|x| x.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) {
            continue;
        }
        if c.len() > head.len() {
            bail!("{where_} row {}: {} cells but {} columns", n + 1, c.len(), head.len());
        }
        let mut r = defaults.clone();
        for (k, v) in head.iter().zip(c) {
            if !v.is_empty() {
                r.insert(k.to_lowercase(), v);
            }
        }
        out.push(r);
    }
    Ok(out)
}

pub fn parse(md: &str) -> Result<Journal> {
    let mut j = Journal::default();
    let mut block: Option<(String, Row, String)> = None;
    let mut heading = String::new();
    let mut text = String::new();
    for line in md.lines() {
        let t = line.trim_start();
        if let Some((kind, defaults, body)) = &mut block {
            if t.starts_with("```") {
                let (kind, defaults, body) = (kind.clone(), defaults.clone(), std::mem::take(body));
                block = None;
                match kind.as_str() {
                    "show" => j.views.extend(body.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from)),
                    k => {
                        let rows = table(&body, &defaults, &format!("```{k} block"))?;
                        match k {
                            "trades" => j.trades.extend(rows),
                            "positions" => j.positions.extend(rows),
                            "orders" => j.orders.extend(rows),
                            _ => j.dates.extend(rows),
                        }
                    }
                }
            } else {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        }
        if let Some(info) = t.strip_prefix("```") {
            let mut words = info.split_whitespace();
            let kind = words.next().unwrap_or("").to_string();
            if kind == "show" || KINDS.contains(&kind.as_str()) {
                let defaults: Row = words.filter_map(|w| w.split_once('=')).map(|(k, v)| (k.to_lowercase(), v.to_string())).collect();
                block = Some((kind, defaults, String::new()));
                continue;
            }
        }
        if t.starts_with('#') {
            if !heading.is_empty() || !text.trim().is_empty() {
                j.notes.push((std::mem::take(&mut heading), std::mem::take(&mut text)));
            }
            heading = t.trim_start_matches('#').trim().to_string();
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }
    if block.is_some() {
        bail!("a ``` block is not closed");
    }
    j.notes.push((heading, text));
    Ok(j)
}

fn num(r: &Row, k: &str, where_: &str) -> Result<f64> {
    let v = r.get(k).with_context(|| format!("{where_}: no {k}"))?;
    v.replace(',', "").parse().map_err(|_| anyhow::anyhow!("{where_}: {k} '{v}' is not a number"))
}

fn opt(r: &Row, k: &str) -> f64 {
    r.get(k).and_then(|v| v.replace(',', "").parse().ok()).unwrap_or(0.0)
}

/// A holding: quantity and average cost per unit, in `ccy`.
#[derive(Debug, Clone, PartialEq)]
pub struct Holding {
    pub account: String,
    pub id: String,
    pub qty: f64,
    pub cost: f64,
    pub ccy: String,
    pub income: f64,
}

/// Holdings and cash (account, ccy) -> amount, as of `until` (ms; trades after it are ignored).
pub fn book(j: &Journal, until: i64) -> Result<(Vec<Holding>, BTreeMap<(String, String), f64>)> {
    let mut h: Vec<Holding> = Vec::new();
    let mut cash: BTreeMap<(String, String), f64> = BTreeMap::new();
    for (i, r) in j.positions.iter().enumerate() {
        let w = format!("positions row {}", i + 1);
        let id = r.get("id").with_context(|| format!("{w}: no id"))?.clone();
        h.push(Holding { account: r.get("account").cloned().unwrap_or_default(), id, qty: num(r, "qty", &w)?, cost: opt(r, "cost"), ccy: r.get("ccy").cloned().unwrap_or_default(), income: 0.0 });
    }
    for (i, r) in j.trades.iter().enumerate() {
        let w = format!("trades row {}", i + 1);
        let date = r.get("date").with_context(|| format!("{w}: no date"))?;
        if crate::model::parse_time(date)? > until {
            continue;
        }
        let account = r.get("account").cloned().unwrap_or_default();
        let action = r.get("action").map(|s| s.to_lowercase()).with_context(|| format!("{w}: no action"))?;
        let ccy = r.get("ccy").cloned().unwrap_or_default();
        let fee = opt(r, "fee");
        let mut add_cash = |c: &str, x: f64| *cash.entry((account.clone(), c.to_string())).or_default() += x;
        match action.as_str() {
            "buy" | "sell" => {
                let id = r.get("id").with_context(|| format!("{w}: no id"))?.clone();
                let (qty, price) = (num(r, "qty", &w)?, num(r, "price", &w)?);
                let q = if action == "sell" { -qty.abs() } else { qty.abs() };
                add_cash(&ccy, -q * price - fee);
                match h.iter_mut().find(|x| x.account == account && x.id == id) {
                    Some(x) if q > 0.0 => {
                        x.cost = (x.cost * x.qty + q * price + fee) / (x.qty + q);
                        x.qty += q;
                    }
                    Some(x) => x.qty += q,
                    None if q > 0.0 => h.push(Holding { account: account.clone(), id, qty: q, cost: (q * price + fee) / q, ccy: ccy.clone(), income: 0.0 }),
                    None => bail!("{w}: selling {id}, which is not held"),
                }
            }
            "div" => {
                let id = r.get("id").with_context(|| format!("{w}: no id"))?;
                let amount = num(r, "price", &w)? - fee;
                add_cash(&ccy, amount);
                if let Some(x) = h.iter_mut().find(|x| x.account == account && x.id == *id) {
                    x.income += amount;
                }
            }
            "fx" => {
                let pair = r.get("id").with_context(|| format!("{w}: no id (BASE.QUOTE, e.g. USD.CNH)"))?;
                let (base, quote) = pair.split_once('.').with_context(|| format!("{w}: fx id '{pair}' is not BASE.QUOTE"))?;
                let (qty, price) = (num(r, "qty", &w)?, num(r, "price", &w)?);
                add_cash(base, qty);
                add_cash(quote, -qty * price - fee);
            }
            "deposit" => add_cash(&ccy, num(r, "qty", &w)?),
            "withdraw" => add_cash(&ccy, -num(r, "qty", &w)?.abs()),
            a => bail!("{w}: unknown action '{a}' (buy, sell, div, fx, deposit, withdraw)"),
        }
    }
    h.retain(|x| x.qty.abs() > 1e-9);
    Ok((h, cash))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Plan\n\nKeep it cheap.\n\n```positions account=co\n| id | qty | cost | ccy |\n| 900911.SS | 4000 | 0.6 | USD |\n```\n\n## China Tower 0788.HK\n\nDividends.\n\n```trades account=co\n| date | action | id | qty | price | ccy | fee |\n|---|---|---|---|---|---|---|\n| 2026-11-10 | fx | USD.HKD | -1000 | 7.8 | | |\n| 2026-11-12 | buy | 0788.HK | 500 | 9.05 | HKD | 25 |\n| 2026-11-20 | buy | 0788.HK | 500 | 8.60 | HKD | 25 |\n| 2026-12-01 | div | 0788.HK | | 110 | HKD | |\n```\n\n```orders\n| ref | side | id | qty | limit | placed |\n| t3 | buy | 0788.HK | 500 | 7.50 | 2026-11-11 |\n```\n\n```show\n@this --with value\n```\n";

    #[test]
    fn journal_blocks_and_book() {
        let j = parse(DOC).unwrap();
        assert_eq!((j.trades.len(), j.positions.len(), j.orders.len(), j.views.len()), (4, 1, 1, 1));
        assert_eq!(j.trades[1]["account"], "co");
        assert!(j.notes.iter().any(|(h, t)| h == "China Tower 0788.HK" && t.contains("Dividends")));
        let (h, cash) = book(&j, i64::MAX).unwrap();
        let tower = h.iter().find(|x| x.id == "0788.HK").unwrap();
        assert_eq!(tower.qty, 1000.0);
        assert!((tower.cost - (500.0 * 9.05 + 25.0 + 500.0 * 8.60 + 25.0) / 1000.0).abs() < 1e-9);
        assert_eq!(tower.income, 110.0);
        assert!((cash[&("co".to_string(), "HKD".to_string())] - (7800.0 - 4550.0 - 4325.0 + 110.0)).abs() < 1e-9);
        let (h, _) = book(&j, crate::model::parse_time("2026-11-15").unwrap()).unwrap();
        assert_eq!(h.iter().find(|x| x.id == "0788.HK").unwrap().qty, 500.0, "--at replays trades up to a date");
        assert!(parse("```trades\n| a |\n| 1 | 2 |\n```\n").is_err());
    }
}
