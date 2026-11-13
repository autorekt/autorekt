//! A journal is a markdown document whose typed code blocks hold an investor's records, read by
//! `show FILE.md` and `show @FILE.md`. Tables are pipe tables with a header row:
//!
//! - `trades`: date action id qty price ccy fee note, where action is buy, sell, div (price is
//!   the cash received), fx (id BASE.QUOTE, qty in BASE, price in QUOTE per BASE), deposit or
//!   withdraw (qty in ccy). Holdings, costs and cash follow from it.
//! - `positions`: id qty cost ccy, holdings from before the trades start.
//! - `orders`: ref side id qty limit placed, and optionally type stop target expires note.
//! - `dates`: date what note.
//! - `instruments`: id multiplier, for futures and FX contracts (value = qty * price * multiplier).
//! - `watch`: rules in the same YAML as rule files (`rules: [...]`), installed by `watch FILE.md`.
//!
//! In `account=sim` orders (optionally with stop and target), fills and exits follow from daily
//! bars: a limit fills when the bar trades through it (at the open if it gaps past), then the
//! stop or the target closes the position; a bar that touches both counts the stop first.
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
    pub instruments: Vec<Row>,
    /// The YAML of the `watch` blocks.
    pub watch: String,
    /// (heading, text) of each section, in order.
    pub notes: Vec<(String, String)>,
    /// Lines of the `show` blocks.
    pub views: Vec<String>,
}

const KINDS: &[&str] = &["trades", "positions", "orders", "dates", "instruments", "watch"];

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
                    "watch" => j.watch.push_str(&body),
                    k => {
                        let rows = table(&body, &defaults, &format!("```{k} block"))?;
                        match k {
                            "trades" => j.trades.extend(rows),
                            "positions" => j.positions.extend(rows),
                            "orders" => j.orders.extend(rows),
                            "instruments" => j.instruments.extend(rows),
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

/// The contract multiplier of an id (1 unless the instruments block says otherwise).
pub fn multiplier(j: &Journal, id: &str) -> f64 {
    j.instruments.iter().find(|r| r.get("id").is_some_and(|i| i == id)).and_then(|r| r.get("multiplier")?.parse().ok()).unwrap_or(1.0)
}

/// The trades that `account=sim` orders make, given each id's daily bars (oldest first).
pub fn simulate(j: &Journal, bars: &BTreeMap<String, Vec<crate::model::Bar>>) -> Vec<Row> {
    let mut out = Vec::new();
    for o in j.orders.iter().filter(|o| o.get("account").is_some_and(|a| a == "sim")) {
        let (Some(id), Some(limit)) = (o.get("id"), o.get("limit").and_then(|v| v.parse::<f64>().ok())) else { continue };
        let Some(b) = bars.get(id) else { continue };
        let placed = o.get("placed").and_then(|p| crate::model::parse_time(p).ok()).unwrap_or(0);
        let expires = o.get("expires").and_then(|p| crate::model::parse_time(p).ok()).unwrap_or(i64::MAX);
        let sell = o.get("side").is_some_and(|s| s.eq_ignore_ascii_case("sell"));
        let qty: f64 = o.get("qty").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let stop = o.get("stop").and_then(|v| v.parse::<f64>().ok());
        let target = o.get("target").and_then(|v| v.parse::<f64>().ok());
        let name = o.get("ref").cloned().unwrap_or_default();
        let trade = |t: i64, action: &str, price: f64, note: String| {
            let mut r = o.clone();
            r.insert("date".into(), chrono::DateTime::from_timestamp_millis(t).unwrap_or_default().format("%Y-%m-%d").to_string());
            r.insert("action".into(), action.into());
            r.insert("price".into(), price.to_string());
            r.insert("qty".into(), qty.to_string());
            r.insert("note".into(), note);
            r
        };
        let mut i = b.iter().position(|x| x.timestamp >= placed).unwrap_or(b.len());
        let fill = loop {
            let Some(x) = b.get(i) else { break None };
            if x.timestamp > expires {
                break None;
            }
            if !sell && x.low <= limit {
                break Some((x.timestamp, x.open.min(limit)));
            }
            if sell && x.high >= limit {
                break Some((x.timestamp, x.open.max(limit)));
            }
            i += 1;
        };
        let Some((t, p)) = fill else { continue };
        out.push(trade(t, if sell { "sell" } else { "buy" }, p, format!("{name} filled (sim)")));
        // Exits from the bar after the fill on, or the fill bar's own close side.
        for x in &b[i..] {
            let stop_hit = stop.is_some_and(|s| if sell { x.high >= s } else { x.low <= s });
            let target_hit = target.is_some_and(|g| if sell { x.low <= g } else { x.high >= g });
            if x.timestamp == t && !stop_hit {
                continue;
            }
            let (price, why) = if stop_hit {
                let s = stop.unwrap();
                (if sell { x.open.max(s) } else { x.open.min(s) }, "stop")
            } else if target_hit {
                let g = target.unwrap();
                (if sell { x.open.min(g) } else { x.open.max(g) }, "target")
            } else {
                continue;
            };
            out.push(trade(x.timestamp, if sell { "buy" } else { "sell" }, price, format!("{name} {why} (sim{})", if stop_hit && target_hit { ", bar touched both: stop counted first" } else { "" })));
            break;
        }
    }
    out
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
                add_cash(&ccy, -q * price * multiplier(j, &id) - fee);
                match h.iter_mut().find(|x| x.account == account && x.id == id) {
                    Some(x) if q > 0.0 && x.qty >= 0.0 => {
                        x.cost = (x.cost * x.qty + q * price + fee) / (x.qty + q);
                        x.qty += q;
                    }
                    Some(x) => x.qty += q,
                    None if q > 0.0 || account == "sim" => h.push(Holding { account: account.clone(), id, qty: q, cost: (q.abs() * price + fee) / q.abs(), ccy: ccy.clone(), income: 0.0 }),
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

    #[test]
    fn sim_orders_fill_then_stop_or_target() {
        use crate::model::Bar;
        let day = 86_400_000;
        let t0 = crate::model::parse_time("2026-11-09").unwrap();
        let bar = |i: i64, o: f64, h: f64, l: f64| Bar { timestamp: t0 + i * day, open: o, high: h, low: l, close: o, volume: 0.0, closed: true };
        let md = "```orders account=sim\n| ref | side | id | qty | limit | stop | target | placed |\n| g | buy | GC | 4 | 3990 | 3870 | 4200 | 2026-11-09 |\n```\n```instruments\n| id | multiplier |\n| GC | 10 |\n```\n";
        let j = parse(md).unwrap();
        let mut bars = BTreeMap::new();
        bars.insert("GC".to_string(), vec![bar(0, 4100.0, 4150.0, 4060.0), bar(1, 4070.0, 4075.0, 3989.0), bar(2, 4000.0, 4005.0, 3856.0)]);
        let t = simulate(&j, &bars);
        assert_eq!(t.len(), 2);
        assert_eq!((t[0]["action"].as_str(), t[0]["price"].as_str()), ("buy", "3990"));
        assert_eq!((t[1]["action"].as_str(), t[1]["price"].as_str()), ("sell", "3870"));
        let mut j2 = j.clone();
        j2.trades = t;
        let (h, cash) = book(&j2, i64::MAX).unwrap();
        assert!(h.is_empty());
        assert!((cash[&("sim".to_string(), String::new())] - (3870.0 - 3990.0) * 4.0 * 10.0).abs() < 1e-6);
    }
}
