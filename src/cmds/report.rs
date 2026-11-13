//! `show FILE.md`: a markdown report whose ```chart blocks hold `show`-style lines (one chart
//! per line; several lines become one tabbed component). Without --share it prints a text
//! preview with the charts rendered as text; with --share the daemon publishes it.

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

pub async fn report(path: &str, a: &ShowArgs, g: &Global, o: &Opts) -> Result<()> {
    let source = std::fs::canonicalize(path).with_context(|| format!("reading {path}"))?.display().to_string();
    if a.share && share::ttl_secs(&a.ttl)? == 0 {
        let v = daemon::api("report", json!({"markdown": "", "blocks": [], "source": source, "name": a.name, "ttl_secs": 0})).await?;
        return share::print_result(&v, g);
    }
    let md = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
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
    if !a.share {
        return preview(&md, &blocks, g, o).await;
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

/// The report as text: prose verbatim, each chart block as text charts.
async fn preview(md: &str, blocks: &[Vec<Chart>], g: &Global, o: &Opts) -> Result<()> {
    let mut n = 0;
    let mut in_chart = false;
    for line in md.lines() {
        let t = line.trim_start();
        if !in_chart && (t.starts_with("```chart") || t.starts_with("~~~chart")) {
            in_chart = true;
            for c in blocks.get(n).map(|b| b.as_slice()).unwrap_or(&[]) {
                println!("[chart] {}", c.line);
                view::chart(&c.series, g, o).await?;
            }
            n += 1;
            continue;
        }
        if in_chart {
            if t.starts_with("```") || t.starts_with("~~~") {
                in_chart = false;
            }
            continue;
        }
        println!("{line}");
    }
    Ok(())
}
