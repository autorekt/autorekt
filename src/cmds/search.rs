//! `search`: find anything by name — how an alias resolves, matching instruments across
//! providers, prediction markets, and dated news. Without a query, list the sources.

use super::*;
use crate::cli::SearchArgs;
use crate::providers::{news as feeds, MarketInfo};
use crate::render::series;

const KINDS: &[&str] = &["instruments", "odds", "news"];

pub async fn search(a: SearchArgs, g: &Global, o: &Opts) -> Result<()> {
    if a.query.is_empty() {
        return sources(a.check, g).await;
    }
    if let Some(k) = &a.kind {
        if !KINDS.contains(&k.as_str()) {
            bail!("unknown --kind '{k}' (instruments, odds, news)");
        }
    }
    let q = a.query.join(" ");
    let want = |k: &str| a.kind.as_deref().map_or(true, |x| x == k);
    let n = a.limit;
    let instruments = async { if want("instruments") { Some(instruments(&q, n).await) } else { None } };
    let odds = async {
        if want("odds") {
            let (pm, ks) = (providers::prob_source("polymarket").unwrap(), providers::prob_source("kalshi").unwrap());
            let (p, k) = tokio::join!(pm.search(&q, n), ks.search(&q, n));
            Some((p, k))
        } else {
            None
        }
    };
    let news = async {
        if want("news") {
            Some(feeds::google(&q).await)
        } else {
            None
        }
    };
    let (inst, odds, news) = tokio::join!(instruments, odds, news);
    let cut = a.range.as_deref().map(|r| parse_duration(r).map(|s| now_ms() - s * 1000)).transpose()?;
    let news = news.map(|r| {
        r.map(|mut v| {
            if let Some(c) = cut {
                v.retain(|e| e.timestamp >= c);
            }
            let mut v = feeds::dedupe(v);
            v.truncate(n);
            v
        })
    });
    // Only real resolutions: an alias, or an explicit provider id (normalized).
    let resolved = ids::parse(&q).ok().filter(|_| a.query.len() == 1 && (q.contains(':') || ids::is_known_alias(&q)));
    if g.json {
        let err = |e: &anyhow::Error| json!({"error": e.to_string()});
        return print_json(&json!({
            "resolves_to": resolved.as_ref().map(|i| i.to_string()),
            "instruments": inst.map(|v| v.into_iter().map(|(p, r)| json!({"provider": p, "results": r.as_ref().map(|x| json!(x)).unwrap_or_else(err)})).collect::<Vec<_>>()),
            "odds": odds.as_ref().map(|(p, k)| json!({"polymarket": p.as_ref().map(|x| json!(x)).unwrap_or_else(err), "kalshi": k.as_ref().map(|x| json!(x)).unwrap_or_else(err)})),
            "news": news.as_ref().map(|r| r.as_ref().map(|x| json!(x)).unwrap_or_else(err)),
        }));
    }
    if let Some(id) = resolved {
        println!("{q} resolves to {id}\n");
    }
    if let Some(list) = inst {
        println!("## instruments");
        for (p, r) in list {
            match r {
                Ok(v) if v.is_empty() => {}
                Ok(v) => {
                    for m in v.iter().take(n) {
                        println!("{}  {}{}", m.id, m.kind, m.note.as_ref().map(|x| format!("  {x}")).unwrap_or_default());
                    }
                }
                Err(e) => eprintln!("warning: {p}: {}", e.to_string().lines().next().unwrap_or("")),
            }
        }
        println!();
    }
    if let Some((p, k)) = odds {
        println!("## prediction markets");
        for (name, r) in [("polymarket", p), ("kalshi", k)] {
            match r {
                Ok(v) => print!("{}", series::prob_markets(&v, false)),
                Err(e) => eprintln!("warning: {name}: {e}"),
            }
        }
        println!();
    }
    if let Some(r) = news {
        println!("## news");
        print!("{}", series::events(&r?, o, false));
    }
    Ok(())
}

/// Matching markets from providers with a listing or search endpoint.
async fn instruments(q: &str, n: usize) -> Vec<(&'static str, Result<Vec<MarketInfo>>)> {
    let names = ["okx", "hyperliquid", "yahoo", "cftc"];
    let futs = names.iter().map(|p| async move {
        let r = match providers::get(p) {
            Ok(prov) => prov.list_markets(Some(q)).await.map(|mut v| {
                v.truncate(n);
                v
            }),
            Err(e) => Err(e),
        };
        (*p, r)
    });
    futures::future::join_all(futs).await
}

/// The sources, what they offer, and (with --check) whether they answer right now.
async fn sources(check: bool, g: &Global) -> Result<()> {
    let probe = |p: &str| -> &'static str {
        match p {
            "okx" => "okx:BTC/USDT:USDT",
            "binance" => "binance:BTC/USDT",
            "hyperliquid" => "hyperliquid:BTC",
            "deribit" => "deribit:BTC-DVOL",
            "coinbase" => "coinbase:BTC/USD",
            "yahoo" => "yahoo:^GSPC",
            "fred" => "fred:DGS10",
            "cftc" => "cftc:13874A",
            "finra" => "finra:SPY",
            "llama" => "llama:stablecoins",
            "alt" => "alt:fng",
            "kalshi" => "kalshi:KXFEDDECISION-26DEC-H0",
            _ => "",
        }
    };
    let all = providers::all();
    let statuses = futures::future::join_all(all.iter().map(|p| async move {
        if !check {
            return String::new();
        }
        let r = match (p.id(), probe(p.id())) {
            ("polymarket", _) => providers::prob_source("polymarket").unwrap().trending(1).await.map(|_| ()),
            (_, "") => return "-".into(),
            (_, id) => providers::ticker(&ids::parse(id).unwrap()).await.map(|_| ()),
        };
        match r {
            Ok(()) => "ok".into(),
            Err(e) => format!("FAIL {}", e.to_string().lines().next().unwrap_or("")),
        }
    }))
    .await;
    if g.json {
        return print_json(&all.iter().zip(&statuses).map(|(p, s)| json!({"id": p.id(), "describe": p.describe(), "metrics": p.metrics(), "status": s})).collect::<Vec<_>>());
    }
    for (p, s) in all.iter().zip(statuses) {
        println!("{:<12} {}{}", p.id(), p.describe(), if s.is_empty() { String::new() } else { format!("  [{s}]") });
        if !p.metrics().is_empty() {
            println!("{:<12}   metrics: {}", "", p.metrics().join(", "));
        }
    }
    println!("{:<12} RSS feeds (cnbc ft bloomberg marketwatch seekingalpha investing fed coindesk cointelegraph theblock decrypt) and Google News search", "news");
    println!("{:<12} ForexFactory weekly calendar", "calendar");
    Ok(())
}
