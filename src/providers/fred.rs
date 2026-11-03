//! FRED (St. Louis Fed) economic data via the keyless CSV endpoint. Ids are FRED series ids:
//! `fred:DGS10`, `fred:BAMLH0A0HYM2` (HY OAS), `fred:SOFR`, `fred:WALCL`, `fred:RRPONTSYD`...
//! Values are daily or lower frequency, published with a lag.

use super::*;
use crate::http::get_text;

pub struct Fred;

async fn csv(series: &str, start_ms: i64) -> Result<Vec<MetricPoint>> {
    let cosd = chrono::DateTime::from_timestamp_millis(start_ms).unwrap_or_default().format("%Y-%m-%d");
    let url = format!("https://fred.stlouisfed.org/graph/fredgraph.csv?id={series}&cosd={cosd}");
    let text = get_text(&url).await?;
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let mut it = line.split(',');
        let (Some(d), Some(v)) = (it.next(), it.next()) else { continue };
        let Ok(v) = v.trim().parse::<f64>() else { continue };
        let Ok(date) = chrono::NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d") else { continue };
        let ts = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis();
        out.push(MetricPoint { timestamp: ts, value: v });
    }
    if out.is_empty() && text.contains("<html") {
        bail!("fred: series '{series}' not found");
    }
    Ok(out)
}

#[async_trait]
impl Provider for Fred {
    fn id(&self) -> &'static str {
        "fred"
    }
    fn describe(&self) -> &'static str {
        "FRED economic data (rates, spreads, Fed balance sheet, liquidity; daily or slower)"
    }
    fn notes(&self, _symbol: &str) -> Vec<String> {
        vec!["daily, published with lag".into()]
    }

    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        let tf = if tf.secs < 86400 { Timeframe::D1 } else { tf };
        // Weekly/monthly series are sparse: look back far enough to get `limit` points.
        let start = range.start.unwrap_or_else(|| range.end_ms() - tf.ms() * range.limit as i64 * 8);
        let pts = csv(symbol, start).await?;
        let mut bars: Vec<Bar> = pts.into_iter().map(|p| Bar::point(p.timestamp, p.value)).collect();
        if tf.secs > 86400 {
            bars = resample(&bars, tf);
        }
        Ok(finish(bars, range))
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        let pts = csv(symbol, now_ms() - 120 * 86_400_000).await?;
        let last = pts.last().ok_or_else(|| anyhow::anyhow!("fred: no recent data for {symbol}"))?;
        let prev = pts.len().checked_sub(2).map(|i| pts[i].value);
        let date = chrono::DateTime::from_timestamp_millis(last.timestamp).unwrap_or_default().format("%Y-%m-%d");
        Ok(Ticker { symbol: symbol.into(), timestamp: last.timestamp, last: last.value, open: prev, notes: vec![format!("as of {date}")], ..Default::default() })
    }
}
