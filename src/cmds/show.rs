//! `show`: read anything. The first word selects the view: nothing is the briefing, a board
//! name is a board, a few keywords select special views, and anything else is ids.

use super::*;
use crate::cli::ShowArgs;

const VIEWS: &[&str] = &["funding", "prelaunch", "odds", "news", "calendar", "alerts", "shares"];

pub async fn show(mut a: ShowArgs, g: &Global, o: &Opts) -> Result<()> {
    a.what = expand(&a.what)?;
    match a.view.as_deref() {
        None | Some("chart") | Some("table") => {}
        Some("explain") => {
            let [id] = a.what.as_slice() else { bail!("--view explain takes one id or expression, e.g. autorekt show BTC --view explain") };
            return analysis::explain(id, &a, g, o).await;
        }
        Some("corr") => {
            let items = match a.what.as_slice() {
                [one] if config::board(one).is_some() => config::board(one).unwrap(),
                list => list.to_vec(),
            };
            return analysis::correlations(items, &a, g, o).await;
        }
        Some(v) => bail!("unknown --view '{v}' (chart, table, corr, explain)"),
    }
    let first = a.what.first().map(|s| s.as_str());
    let rest: Vec<String> = a.what.iter().skip(1).cloned().collect();
    match first {
        None if a.changes => alerts::diff(&a.agent, &config::default_board(), "default", g, o).await,
        None => brief::brief(hours(&a, 12)?, g, o).await,
        Some(set) if config::board(set).is_some() && rest.is_empty() => {
            let items = config::board(set).unwrap();
            if table::wanted_table(&a, true) {
                return table::table(&items, set, &a, g, o).await;
            }
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
        Some("shares") => share::list(g),
        Some("alerts") => alerts::events(&a.range.clone().unwrap_or_else(|| "24h".into()), a.limit.unwrap_or(50), a.min_severity, g, o),
        Some(w) if a.what.len() == 1 && w.chars().all(|c| c.is_ascii_lowercase()) && !ids::is_known_alias(w) => {
            bail!("'{w}' is not a view or a known alias. Views: {}. For anything else: autorekt search {w}", view_names().join(", "))
        }
        Some(f) if a.what.len() == 1 && f.ends_with(".md") => report::report(f, &a, g, o).await,
        Some(_) if table::wanted_table(&a, a.what.len() > 1) && !g.csv => table::table(&a.what, "ids", &a, g, o).await,
        Some(_) if a.what.len() == 1 || a.view.as_deref() == Some("chart") => {
            if a.what.len() > 1 {
                bail!("--view chart shows one id; several ids make a board or a table");
            }
            one(&a, g, o).await
        }
        Some(_) if g.csv => view::csv_many(&a.what.iter().map(|id| series(&a, id)).collect::<Vec<_>>(), o).await,
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
    let s = series(a, target);
    if a.share {
        return share::share(&s, a, g).await;
    }
    if !expr::is_expression(target) {
        let id = ids::parse(target)?;
        if matches!(id.provider.as_str(), "polymarket" | "kalshi") && id.metric.is_none() {
            return view::market(&id, &s, g, o).await;
        }
    }
    view::chart(&s, g, o).await
}

/// The words of `show` with each @FILE replaced by the ids in it (one per line; lines starting
/// with # are comments).
fn expand(what: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for w in what {
        match w.strip_prefix('@') {
            Some(path) => {
                let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
                out.extend(text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| l.to_string()));
            }
            None => out.push(w.clone()),
        }
    }
    Ok(out)
}

/// The series an id names, with --from/--to as the window when given.
fn series(a: &ShowArgs, id: &str) -> view::Series {
    let (mut range, mut at) = (a.range.clone(), a.at.clone());
    if let Ok(Some((start, end))) = table::command_span(a) {
        range = Some(format!("{}h", ((end - start + 3_599_999) / 3_600_000).max(1)));
        at = Some(end.to_string());
    }
    view::Series { id: id.to_string(), interval: a.interval.clone().unwrap_or_else(|| "1h".into()), limit: a.limit, range, at, news: a.news, with: a.with.clone(), context: true }
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
