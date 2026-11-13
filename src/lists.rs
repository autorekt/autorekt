//! US restriction lists a stock may be on: OFAC's Non-SDN Chinese Military-Industrial Complex
//! list (NS-CMIC, matched by the equity tickers OFAC records), and the Department of Defense's
//! Section 1260H list of Chinese military companies (from the Federal Register, matched by
//! company name, less companies removed since). Both are kept a day in the cache.

use crate::http::get_text;
use anyhow::{Context, Result};
use serde_json::Value;

pub const NS_CMIC: f64 = 1.0;
pub const L1260H: f64 = 2.0;

async fn cached(name: &str, url: &str) -> Result<String> {
    let path = crate::config::home().join("cache").join(name);
    let fresh = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).is_some_and(|t| t.elapsed().map_or(false, |e| e.as_secs() < 86400));
    if fresh {
        return Ok(std::fs::read_to_string(&path)?);
    }
    let t = get_text(url).await?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, &t)?;
    Ok(t)
}

/// "00981 HK" and "981 HK" name the same listing.
fn norm(ticker: &str) -> String {
    match ticker.split_once(' ') {
        Some((code, market)) => format!("{} {market}", code.trim_start_matches('0')),
        None => ticker.to_string(),
    }
}

/// The OFAC tickers ("941 HK", "CHL US") of NS-CMIC entries, without leading zeros.
async fn cmic_tickers() -> Result<Vec<String>> {
    let csv = cached("ofac_cons_prim.csv", "https://sanctionslistservice.ofac.treas.gov/api/PublicationPreview/exports/CONS_PRIM.CSV").await.context("OFAC consolidated list")?;
    let mut out = Vec::new();
    for line in csv.lines().filter(|l| l.contains("CMIC")) {
        for part in line.split("Equity Ticker ").skip(1) {
            let t: String = part.split(';').next().unwrap_or("").trim_matches(['"', ' ']).to_uppercase();
            out.push(norm(&t));
        }
    }
    Ok(out)
}

/// Text between the tags of each `<p class="flush-paragraph...">` in a Federal Register page.
fn flush_paragraphs(html: &str) -> Vec<String> {
    html.split("<p class=\"flush-paragraph").skip(1).filter_map(|p| Some(p.split_once('>')?.1.split("</p>").next()?.trim().to_string())).collect()
}

/// Company names on the latest 1260H list, lower case, without the companies removed since.
async fn names_1260h() -> Result<Vec<String>> {
    let api = "https://www.federalregister.gov/api/v1/documents.json?conditions%5Bterm%5D=1260H&conditions%5Btype%5D%5B%5D=NOTICE&order=newest&per_page=40&fields%5B%5D=title&fields%5B%5D=body_html_url";
    let v: Value = serde_json::from_str(&cached("fr_1260h.json", api).await.context("Federal Register search")?)?;
    let docs = v["results"].as_array().cloned().unwrap_or_default();
    let mut removed = Vec::new();
    let mut names = Vec::new();
    for d in &docs {
        let title = d["title"].as_str().unwrap_or("");
        let url = d["body_html_url"].as_str().unwrap_or("");
        if title.contains("Removal") {
            let html = cached(&format!("fr_{}.html", url.rsplit('/').next().unwrap_or("x")), url).await?;
            let text = html.replace(['\n', '\r'], " ");
            for part in text.split("determined that ").skip(1) {
                if let Some((who, _)) = part.split_once(" should be removed") {
                    removed.push(who.trim().to_lowercase());
                }
            }
        } else if title.contains("Chinese Military Compan") {
            let html = cached(&format!("fr_{}.html", url.rsplit('/').next().unwrap_or("x")), url).await?;
            names = flush_paragraphs(&html).into_iter().map(|n| n.to_lowercase()).collect();
            break;
        }
    }
    if names.is_empty() {
        let _ = std::fs::remove_file(crate::config::home().join("cache").join("fr_1260h.json"));
        anyhow::bail!("no 1260H designation notice found in the Federal Register");
    }
    names.retain(|n| !removed.iter().any(|r| n.starts_with(r.as_str())));
    Ok(names)
}

/// Which lists a stock is on: NS_CMIC + L1260H flags summed (0 = none). `name` is its English
/// name (Yahoo's long name), for the 1260H list, which gives names only.
pub async fn check(ticker: &str, name: &str) -> Result<f64> {
    let mut v = 0.0;
    if cmic_tickers().await?.iter().any(|t| *t == norm(ticker)) {
        v += NS_CMIC;
    }
    // The first two words of the name ("alibaba group", "china mobile") start a listed name.
    let words = |s: &str| s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect::<Vec<_>>();
    let key: Vec<String> = words(name).into_iter().take(2).collect();
    if !key.is_empty() && names_1260h().await?.iter().any(|n| words(n).starts_with(&key)) {
        v += L1260H;
    }
    Ok(v)
}

pub fn label(v: f64) -> String {
    match v as i64 {
        0 => "-".into(),
        1 => "NS-CMIC".into(),
        2 => "1260H".into(),
        _ => "NS-CMIC,1260H".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn federal_register_names_come_from_flush_paragraphs() {
        let html = r#"<p class="flush-paragraph flush-paragraph-1" id="p-13">Alibaba Group Holding Limited (Alibaba)</p><li>x</li><p class="flush-paragraph">BYD Company Limited (BYD)</p>"#;
        assert_eq!(flush_paragraphs(html), vec!["Alibaba Group Holding Limited (Alibaba)", "BYD Company Limited (BYD)"]);
        assert_eq!(label(L1260H + NS_CMIC), "NS-CMIC,1260H");
    }
}
