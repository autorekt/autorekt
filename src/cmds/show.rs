//! `show`: read anything. The first word selects the view: nothing is the briefing, a board
//! name is a board, a few keywords select special views, and anything else is ids.

use super::*;
use crate::cli::ShowArgs;

const VIEWS: &[&str] = &["funding", "prelaunch", "odds", "news", "calendar", "alerts"];

pub async fn show(a: ShowArgs, g: &Global, o: &Opts) -> Result<()> {
    let first = a.what.first().map(|s| s.as_str());
    let rest: Vec<String> = a.what.iter().skip(1).cloned().collect();
    match first {
        None if a.changes => alerts::diff(&a.agent, &config::default_board(), "default", g, o).await,
        None => brief::brief(hours(&a, 12)?, g, o).await,
        Some(set) if config::board(set).is_some() && rest.is_empty() => {
            let items = config::board(set).unwrap();
            if a.changes {
                alerts::diff(&a.agent, &items, set, g, o).await
            } else {
                view::board(&items, set, g, o).await
            }
        }
        Some("funding") => crypto::funding(rest, g).await,
        Some("prelaunch") => crypto::prelaunch(g).await,
        Some("odds") => view::odds(a.limit.unwrap_or(10), g).await,
        Some("news") => view::news(rest.first().map(|s| s.as_str()), since(&a)?, a.min_severity, a.limit.unwrap_or(40), g, o).await,
        Some("calendar") => view::calendar(&rest, a.min_severity.max(2), g, o).await,
        Some("alerts") => alerts::events(&a.range.clone().unwrap_or_else(|| "24h".into()), a.limit.unwrap_or(50), a.min_severity, g, o),
        Some(w) if a.what.len() == 1 && w.chars().all(|c| c.is_ascii_lowercase()) && !ids::is_known_alias(w) => {
            bail!("'{w}' is not a view or a known alias. Views: {}. For anything else: autorekt search {w}", view_names().join(", "))
        }
        Some(f) if a.what.len() == 1 && f.ends_with(".md") => report::report(f, &a, g, o).await,
        Some(_) if a.what.len() == 1 => one(&a, g, o).await,
        Some(_) => {
            if a.changes {
                alerts::diff(&a.agent, &a.what, "ids", g, o).await
            } else {
                view::board(&a.what, "ids", g, o).await
            }
        }
    }
}

/// A single id or expression: a prediction market, a shared link, or a chart.
async fn one(a: &ShowArgs, g: &Global, o: &Opts) -> Result<()> {
    let target = &a.what[0];
    let s = view::Series { id: target.clone(), interval: a.interval.clone().unwrap_or_else(|| "1h".into()), limit: a.limit, range: a.range.clone(), at: a.at.clone(), news: a.news, with: a.with.clone(), context: true };
    if a.share {
        return share::share(&s, &a.overlay, &a.ttl, a.local, g).await;
    }
    if !expr::is_expression(target) {
        let id = ids::parse(target)?;
        if matches!(id.provider.as_str(), "polymarket" | "kalshi") && id.metric.is_none() {
            return view::market(&id, &s, g, o).await;
        }
    }
    view::chart(&s, g, o).await
}

fn hours(a: &ShowArgs, default: i64) -> Result<i64> {
    Ok(match &a.range {
        Some(r) => (parse_duration(r)? / 3600).max(1),
        None => default,
    })
}

fn since(a: &ShowArgs) -> Result<Option<i64>> {
    a.range.as_deref().map(|r| Ok(now_ms() - parse_duration(r)? * 1000)).transpose()
}

/// Names `show` treats as views rather than ids.
fn view_names() -> Vec<String> {
    let mut v = config::board_names();
    v.extend(VIEWS.iter().map(|s| s.to_string()));
    v
}
