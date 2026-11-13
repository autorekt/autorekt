//! `show FILE.md --share`: publish a markdown document whose ```show blocks hold one chart per
//! line (several lines become one tabbed component) as a page with live charts. A journal is
//! published with its data blocks as tables.

use super::*;
use crate::cli::{parse_chart_line, ShowArgs};
use crate::daemon::{chart_blocks, ChartReq};

struct Chart {
    line: String,
    series: view::Series,
    overlays: Vec<String>,
}

fn charts(block: &str) -> Result<Vec<Chart>> {
    let mut out = Vec::new();
    for line in block.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let a = parse_chart_line(line).with_context(|| format!("chart line `{line}`"))?;
        if a.what.is_empty() {
            bail!("chart line `{line}` names no id or expression");
        }
        let series = view::Series { id: a.what.join(" "), interval: a.interval.unwrap_or_else(|| "1h".into()), limit: a.limit, range: a.range, at: a.at, news: false, with: vec![], context: false };
        out.push(Chart { line: line.to_string(), series, overlays: a.overlay });
    }
    Ok(out)
}

fn label(c: &Chart) -> String {
    let mut l = format!("{} · {}", c.series.id, c.series.interval);
    if let Some(r) = &c.series.range {
        l.push_str(&format!(" · {r}"));
    }
    if let Some(at) = &c.series.at {
        l.push_str(&format!(" to {}", at.split('T').next().unwrap_or(at)));
    }
    l
}

pub async fn report(path: &str, a: &ShowArgs, g: &Global) -> Result<()> {
    let source = std::fs::canonicalize(path).with_context(|| format!("reading {path}"))?.display().to_string();
    if a.share && share::ttl_secs(&a.ttl)? == 0 {
        let v = daemon::api("report", json!({"markdown": "", "blocks": [], "source": source, "name": a.name, "ttl_secs": 0})).await?;
        return share::print_result(&v, g);
    }
    let md = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let md = if journal::is_journal(&md) { journal::share_markdown(&md, a.in_ccy.as_deref(), a.show_amounts).await? } else { md };
    let blocks: Vec<Vec<Chart>> = chart_blocks(&md).iter().map(|b| charts(b)).collect::<Result<_>>()?;
    // Every chart must load now, so a typo fails here instead of on the reader's screen.
    let all: Vec<&Chart> = blocks.iter().flatten().collect();
    let results = futures::future::join_all(all.iter().map(|c| async move {
        let mut sources = vec![c.series.id.clone()];
        sources.extend(c.overlays.iter().cloned());
        for id in sources {
            let s = view::Series { id, interval: c.series.interval.clone(), limit: c.series.limit, range: c.series.range.clone(), at: c.series.at.clone(), news: false, with: vec![], context: false };
            let bars = view::fetch(&s).await?;
            if bars.is_empty() {
                bail!("no data");
            }
        }
        Ok(())
    }))
    .await;
    let failures: Vec<String> = all.iter().zip(results).filter_map(|(c, r)| r.err().map(|e| format!("  `{}`: {e:#}", c.line))).collect();
    if !failures.is_empty() {
        bail!("{} chart(s) in {path} do not load:\n{}", failures.len(), failures.join("\n"));
    }
    let req: Vec<Vec<ChartReq>> = blocks
        .iter()
        .map(|b| {
            b.iter()
                .map(|c| {
                    Ok(ChartReq {
                        id: c.series.id.clone(),
                        tf: c.series.interval.clone(),
                        limit: c.series.limit,
                        range_secs: c.series.range.as_deref().map(parse_duration).transpose()?,
                        end: c.series.at.as_deref().map(parse_time).transpose()?,
                        overlays: c.overlays.clone(),
                        label: label(c),
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<_>>()?;
    let v = daemon::api("report", json!({"markdown": md, "blocks": req, "source": source, "name": a.name, "ttl_secs": share::ttl_secs(&a.ttl)?, "tunnel": if a.local { Some(false) } else { None }})).await?;
    share::print_result(&v, g)
}
