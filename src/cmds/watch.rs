//! Receiving what the daemon saw: `wait` blocks until an event, `events` lists the log,
//! `diff` shows what changed since an agent last looked (events, board moves, headlines).

use super::*;

pub async fn wait(rule: Option<String>, timeout: Option<String>, min_sev: u8, g: &Global, o: &Opts) -> Result<()> {
    let info = daemon::ensure().await?;
    let names: Option<Vec<String>> = rule.map(|r| r.split(',').map(|s| s.trim().to_string()).collect());
    if let Some(ns) = &names {
        let have: Vec<String> = rules::load()?.into_iter().map(|(r, _)| r.name).collect();
        for n in ns {
            if !have.contains(n) {
                bail!("no rule named '{n}'");
            }
        }
    }
    let deadline = timeout.as_deref().map(parse_duration).transpose()?.map(|s| now_ms() + s * 1000);
    let mut after = store::max_event_id(&store::open()?)?;
    eprintln!("waiting for events (daemon pid {}){}...", info.pid, deadline.map(|_| " with timeout".to_string()).unwrap_or_default());
    loop {
        let c = store::open()?;
        let evs = store::events_after(&c, after, 100)?;
        if let Some((id, _, _)) = evs.last() {
            after = *id;
        }
        let hits: Vec<_> = evs
            .into_iter()
            .filter(|(_, e, r)| e.severity >= min_sev && names.as_ref().map_or(true, |ns| r.as_ref().map_or(false, |r| ns.contains(r))))
            .collect();
        if !hits.is_empty() {
            if g.json {
                let v: Vec<_> = hits.iter().map(|(id, e, r)| json!({"id": id, "rule": r, "event": e})).collect();
                return print_json(&v);
            }
            for (_, e, _) in &hits {
                println!("{} [{}] sev{} {}", o.full_time(e.timestamp), e.source, e.severity, e.title);
                if !e.body.is_empty() && e.kind != "rule" {
                    println!("    {}", e.body);
                }
            }
            return Ok(());
        }
        if deadline.map_or(false, |d| now_ms() > d) {
            eprintln!("timeout: nothing fired");
            std::process::exit(2);
        }
        if daemon::running().is_none() {
            bail!("daemon stopped while waiting");
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

pub fn events(since: &str, limit: usize, g: &Global, o: &Opts) -> Result<()> {
    let c = store::open()?;
    let evs = store::events_since(&c, now_ms() - parse_duration(since)? * 1000, limit)?;
    if g.json {
        let v: Vec<_> = evs.iter().map(|(id, e, r)| json!({"id": id, "rule": r, "event": e})).collect();
        return print_json(&v);
    }
    if evs.is_empty() {
        println!("no events in the last {since}");
    }
    let list: Vec<Event> = evs.into_iter().map(|(_, e, _)| e).collect();
    print!("{}", render::series::events(&list, o, true));
    Ok(())
}


pub async fn diff(agent: &str, set: &str, peek: bool, g: &Global, o: &Opts) -> Result<()> {
    let c = store::open()?;
    let last_ts: i64 = store::get_cursor(&c, agent, "ts")?.and_then(|s| s.parse().ok()).unwrap_or(0);
    let last_ev: i64 = store::get_cursor(&c, agent, "event_id")?.and_then(|s| s.parse().ok()).unwrap_or(0);
    let snap: std::collections::HashMap<String, f64> = match store::get_cursor(&c, agent, &format!("board:{set}"))? {
        Some(s) => serde_json::from_str(&s).context("corrupt board snapshot in the cursor store")?,
        None => Default::default(),
    };
    let items = config::board(set).unwrap_or_else(config::default_board);
    let (rows, news_res) = tokio::join!(board::build(&items), crate::providers::news::fetch_all(None));
    let since = if last_ts > 0 { last_ts } else { now_ms() - 6 * 3_600_000 };
    let evs = store::events_after(&c, last_ev, 200)?;
    for e in &news_res.1 {
        eprintln!("warning: feed {e}");
    }
    let mut news: Vec<Event> = news_res.0.into_iter().filter(|e| e.timestamp > since && e.severity >= 2).collect();
    news.truncate(20);
    // Board moves since the snapshot.
    let mut moves: Vec<(String, f64, f64, f64)> = Vec::new();
    let mut new_snap = std::collections::HashMap::new();
    for r in &rows {
        if r.error.is_some() || !r.last.is_finite() {
            continue;
        }
        new_snap.insert(r.label.clone(), r.last);
        if let Some(prev) = snap.get(&r.label) {
            if let Some(p) = render::pct(r.last, *prev) {
                moves.push((r.label.clone(), *prev, r.last, p));
            }
        }
    }
    moves.sort_by(|a, b| b.3.abs().partial_cmp(&a.3.abs()).unwrap());
    if g.json {
        print_json(&json!({
            "agent": agent, "since": if last_ts > 0 { Some(last_ts) } else { None },
            "events": evs.iter().map(|(id, e, r)| json!({"id": id, "rule": r, "event": e})).collect::<Vec<_>>(),
            "moves": moves.iter().map(|m| json!({"id": m.0, "from": m.1, "to": m.2, "pct": m.3})).collect::<Vec<_>>(),
            "news": news,
        }))?;
    } else {
        if last_ts == 0 {
            println!("diff for '{agent}': first look — showing the board and the last 6h of major news; next time you will see only changes.");
        } else {
            println!("diff for '{agent}' since {} ({})", o.full_time(last_ts), ago(last_ts));
        }
        if !evs.is_empty() {
            println!("\nevents ({}):", evs.len());
            let list: Vec<Event> = evs.iter().map(|(_, e, _)| e.clone()).collect();
            print!("{}", render::series::events(&list, o, false));
        } else if last_ts > 0 {
            println!("\nevents: none");
        }
        if snap.is_empty() {
            println!();
            print!("{}", board::render(set, &rows, o));
        } else {
            println!("\nmoves since last look (board {set}):");
            let mut t = vec![vec!["id".to_string(), "then".into(), "now".into(), "chg".into()]];
            for (l, a, b, p) in moves.iter().take(25) {
                if p.abs() < 0.05 {
                    continue;
                }
                t.push(vec![l.clone(), render::fmt_auto(*a), render::fmt_auto(*b), format!("{p:+.2}%")]);
            }
            if t.len() == 1 {
                println!("  nothing moved more than 0.05%");
            } else {
                print!("{}", render::table(&t));
            }
            let flagged: Vec<String> = rows.iter().filter(|r| !r.flags.is_empty() && r.flags.iter().any(|f| f.contains("z") || f.contains("20d"))).map(|r| format!("{}: {}", r.label, r.flags.join(", "))).collect();
            if !flagged.is_empty() {
                println!("flags: {}", flagged.join(" | "));
            }
        }
        println!("\nmajor headlines{}:", if last_ts > 0 { " since last look" } else { " (6h)" });
        if news.is_empty() {
            println!("  none");
        } else {
            print!("{}", render::series::events(&news, o, false));
        }
    }
    if !peek {
        store::set_cursor(&c, agent, "ts", &now_ms().to_string())?;
        store::set_cursor(&c, agent, "event_id", &store::max_event_id(&c)?.to_string())?;
        store::set_cursor(&c, agent, &format!("board:{set}"), &serde_json::to_string(&new_snap)?)?;
    }
    Ok(())
}
