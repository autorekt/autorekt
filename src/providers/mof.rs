//! Japan Ministry of Finance JGB benchmark yields, percent, daily since 1974: `mof:JGB10Y`
//! (also 1Y 2Y 3Y 4Y 5Y 6Y 7Y 8Y 9Y 15Y 20Y 25Y 30Y 40Y). Published each Tokyo business day.

use super::*;
use crate::http::get_text;

pub struct Mof;

const BASE: &str = "https://www.mof.go.jp/english/policy/jgbs/reference/interest_rate";
const TENORS: &[&str] = &["1Y", "2Y", "3Y", "4Y", "5Y", "6Y", "7Y", "8Y", "9Y", "10Y", "15Y", "20Y", "25Y", "30Y", "40Y"];

fn parse(text: &str, col: usize) -> Vec<MetricPoint> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let d = chrono::NaiveDate::parse_from_str(f.first()?.trim(), "%Y/%m/%d").ok()?;
            Some(MetricPoint { timestamp: day_ms(d), value: f.get(col + 1)?.trim().parse().ok()? })
        })
        .collect()
}

/// The full history up to last month, cached on disk for the month.
async fn history() -> Result<String> {
    let dir = crate::config::home().join("cache").join("mof");
    let path = dir.join(format!("jgbcme_all_{}.csv", chrono::Utc::now().format("%Y%m")));
    if let Ok(t) = std::fs::read_to_string(&path) {
        return Ok(t);
    }
    let t = get_text(&format!("{BASE}/historical/jgbcme_all.csv")).await?;
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, &t);
    Ok(t)
}

async fn series(symbol: &str, start: i64) -> Result<Vec<MetricPoint>> {
    let s = symbol.to_uppercase();
    let col = s.strip_prefix("JGB").and_then(|t| TENORS.iter().position(|x| *x == t)).ok_or_else(|| anyhow::anyhow!("mof: unknown series '{symbol}' (JGB1Y ... JGB10Y, JGB15Y, JGB20Y, JGB25Y, JGB30Y, JGB40Y)"))?;
    let mut pts = parse(&get_text(&format!("{BASE}/jgbcme.csv")).await?, col);
    let month_start = pts.first().map(|p| p.timestamp).unwrap_or_else(now_ms);
    if start < month_start {
        pts.extend(parse(&history().await?, col).into_iter().filter(|p| p.timestamp >= start && p.timestamp < month_start));
    }
    pts.sort_by_key(|p| p.timestamp);
    pts.dedup_by_key(|p| p.timestamp);
    if pts.is_empty() {
        bail!("mof: no {s} yields");
    }
    Ok(pts)
}

#[async_trait]
impl Provider for Mof {
    fn id(&self) -> &'static str {
        "mof"
    }
    fn describe(&self) -> &'static str {
        "Japan Ministry of Finance: JGB benchmark yields 1y-40y, daily"
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["daily, Tokyo business days".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, lookback(tf, range)).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, now_ms() - 10 * DAY_MS).await?, "%")
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        let items: Vec<(String, String)> = TENORS.iter().map(|t| (format!("JGB{t}"), format!("Japan government bond yield {t}, %"))).collect();
        let refs: Vec<(&str, &str)> = items.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        Ok(list_static("mof", &refs, filter))
    }
}
