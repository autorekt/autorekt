//! Farside Investors' daily US spot crypto ETF flows, $mn: `farside:BTC` and `farside:ETH` are
//! the day's total net flow across funds; one fund with a metric, `farside:BTC#IBIT`. Daily since
//! each launch; a day's row fills in after the US close.

use super::*;
use crate::http::get_text;

pub struct Farside;

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").trim().to_string()
}

/// Cells of each `<tr>` in the first table whose header row ends with "Total".
fn table(html: &str) -> Vec<Vec<String>> {
    for t in html.split("<table").skip(1) {
        let t = t.split("</table>").next().unwrap_or("");
        let rows: Vec<Vec<String>> = t
            .split("<tr")
            .skip(1)
            .map(|r| r.split("</tr>").next().unwrap_or("").split("<t").skip(1).filter(|c| c.starts_with('d') || c.starts_with('h')).map(|c| strip_tags(&format!("<{c}"))).collect())
            .collect();
        if rows.first().and_then(|r| r.last()).is_some_and(|c| c == "Total") {
            return rows;
        }
    }
    vec![]
}

fn amount(s: &str) -> Option<f64> {
    let s = s.replace(',', "");
    match s.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        Some(neg) => neg.parse::<f64>().ok().map(|v| -v),
        None => s.parse().ok(),
    }
}

async fn series(symbol: &str, fund: Option<&str>) -> Result<Vec<MetricPoint>> {
    let page = match symbol.to_uppercase().as_str() {
        "BTC" => "bitcoin-etf-flow-all-data",
        "ETH" => "ethereum-etf-flow-all-data",
        _ => bail!("farside: unknown series '{symbol}' (BTC, ETH)"),
    };
    let rows = table(&get_text(&format!("https://farside.co.uk/{page}/")).await?);
    let Some(head) = rows.first() else { bail!("farside: no flow table on the page") };
    let col = match fund {
        None => head.len() - 1,
        Some(f) => head.iter().position(|h| h.eq_ignore_ascii_case(f)).ok_or_else(|| anyhow::anyhow!("farside: no fund '{f}' (have: {})", head[1..head.len() - 1].join(", ")))?,
    };
    Ok(rows
        .iter()
        .skip(1)
        .filter_map(|r| {
            let d = chrono::NaiveDate::parse_from_str(r.first()?, "%d %b %Y").ok()?;
            Some(MetricPoint { timestamp: day_ms(d), value: amount(r.get(col)?)? })
        })
        .collect())
}

#[async_trait]
impl Provider for Farside {
    fn id(&self) -> &'static str {
        "farside"
    }
    fn describe(&self) -> &'static str {
        "Farside Investors: daily net flows of US spot bitcoin and ether ETFs, $mn"
    }
    fn notes(&self, _s: &str) -> Vec<String> {
        vec!["daily net flow".into()]
    }
    async fn fetch_ohlcv(&self, symbol: &str, tf: Timeframe, range: Range) -> Result<Vec<Bar>> {
        Ok(point_bars(series(symbol, None).await?, tf, range))
    }
    async fn fetch_ticker(&self, symbol: &str) -> Result<Ticker> {
        point_ticker(symbol, series(symbol, None).await?, "")
    }
    async fn fetch_metric(&self, symbol: &str, metric: &str, _tf: Timeframe, range: Range) -> Result<Vec<MetricPoint>> {
        Ok(finish_metric(series(symbol, Some(metric)).await?, range))
    }
    async fn list_markets(&self, filter: Option<&str>) -> Result<Vec<MarketInfo>> {
        Ok(list_static("farside", &[("BTC", "US spot bitcoin ETF daily net flows, $mn; #IBIT #FBTC ..."), ("ETH", "US spot ether ETF daily net flows, $mn; #ETHA ...")], filter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_flow_table() {
        let html = "<table><tr><th>Date</th><th>IBIT</th><th>Total</th></tr><tr><td>04 Nov 2026</td><td>(1,233.7)</td><td>-</td></tr></table>";
        let rows = table(html);
        assert_eq!(rows[1], vec!["04 Nov 2026", "(1,233.7)", "-"]);
        assert_eq!(amount(&rows[1][1]), Some(-1233.7));
        assert_eq!(amount("-"), None);
    }
}
