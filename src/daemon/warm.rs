//! Keeps the series an agent is likely to read fresh in the local store: the briefing, the ids
//! rules watch and anything asked for in the last week. One queue per provider, one series at a
//! time with a pause after each fetch, so no source sees a burst; failing series back off.

use crate::ids::{self, InstrumentId};
use crate::model::{now_ms, Timeframe};
use crate::providers::{self, Range};
use crate::{config, expr, rules, store};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const WEEK: i64 = 7 * 86_400_000;
/// After a failed fetch, wait this long before trying the series again.
const BACKOFF: i64 = 10 * 60_000;

/// Ids behind a board item: "LABEL|expression", an expression, or an id.
fn ids_of(item: &str, out: &mut BTreeSet<InstrumentId>) {
    let item = item.split_once('|').map_or(item, |(_, e)| e).trim();
    if expr::is_expression(item) {
        let mut needs = BTreeSet::new();
        if let Ok(ast) = expr::parse(item) {
            if expr::needs(&ast, Timeframe::H1, &mut needs).is_ok() {
                out.extend(needs.into_iter().map(|n| n.id));
            }
        }
    } else if let Ok(id) = ids::parse(item) {
        out.insert(id);
    }
}

fn warm_set() -> BTreeSet<InstrumentId> {
    let mut out = BTreeSet::new();
    let s = config::brief_sections();
    for item in s.main.iter().chain(&s.fear).chain(&s.liq).chain(&s.world).chain(&s.crypto) {
        ids_of(item, &mut out);
    }
    for (r, _) in rules::load().unwrap_or_default() {
        if let Some(e) = &r.expr {
            ids_of(e, &mut out);
        }
    }
    if let Ok(c) = store::open() {
        for i in store::wanted_since(&c, now_ms() - WEEK).unwrap_or_default() {
            ids_of(&i, &mut out);
        }
    }
    out.retain(|i| i.provider != "local");
    out
}

/// Statistics and odds are read as daily series; markets also hourly.
fn daily(id: &InstrumentId) -> bool {
    ids::points(id) || matches!(id.provider.as_str(), "polymarket" | "kalshi")
}

/// The store keys a board row reads for `id`, with the timeframe that sets their freshness.
fn keys(id: &InstrumentId) -> Vec<(String, Timeframe)> {
    let name = id.to_string();
    let mut k = vec![(format!("{name}|86400"), Timeframe::D1), (format!("{name}|ticker"), Timeframe::H1)];
    if !daily(id) {
        k.push((format!("{name}|3600"), Timeframe::H1));
    }
    k
}

fn due(id: &InstrumentId) -> bool {
    let Ok(c) = store::open() else { return false };
    let now = now_ms();
    keys(id).iter().any(|(k, tf)| {
        let f = store::fetch_state(&c, k).unwrap_or_default();
        let fresh = f.ok_at.is_some_and(|t| now - t < providers::ttl_ms(id, *tf));
        let backing_off = f.err_at.is_some_and(|t| now - t < BACKOFF && f.ok_at.map_or(true, |ok| t > ok));
        !fresh && !backing_off
    })
}

/// Read what a board row reads, through the store.
async fn refresh(id: &InstrumentId) {
    if !daily(id) {
        let _ = providers::ohlcv(id, Timeframe::H1, Range::last(24 * 6)).await;
    }
    let _ = providers::ohlcv(id, Timeframe::D1, Range::last(crate::render::board::daily_points(&id.to_string()))).await;
    let _ = providers::ticker(id).await;
}

/// Pause after each fetch from one provider.
fn pace(provider: &str) -> Duration {
    Duration::from_millis(match provider {
        "okx" | "binance" | "hyperliquid" | "coinbase" | "deribit" => 300,
        "yahoo" => 1500,
        _ => 1000,
    })
}

pub(super) async fn warm_loop() {
    let mut workers: BTreeMap<String, tokio::task::JoinHandle<()>> = BTreeMap::new();
    let mut pruned = 0;
    loop {
        let mut by_provider: BTreeMap<String, Vec<InstrumentId>> = BTreeMap::new();
        for id in warm_set() {
            by_provider.entry(id.provider.clone()).or_default().push(id);
        }
        for (p, ids) in by_provider {
            if workers.get(&p).is_some_and(|h| !h.is_finished()) {
                continue;
            }
            workers.insert(
                p,
                tokio::spawn(async move {
                    for id in ids {
                        if due(&id) {
                            refresh(&id).await;
                            tokio::time::sleep(pace(&id.provider)).await;
                        }
                    }
                }),
            );
        }
        if now_ms() - pruned > 86_400_000 {
            pruned = now_ms();
            if let Ok(c) = store::open() {
                let _ = store::prune_bars(&c, now_ms() - 400 * 86_400_000);
            }
        }
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}
