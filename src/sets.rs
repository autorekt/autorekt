//! Sets that come from a source rather than the config: Eastmoney's lists of A-share industry and
//! concept boards and of the Hong Kong stocks in Stock Connect. One list call gives every member
//! with its latest changes and fund flows, so a table of a set needs no request per member for
//! those columns. Lists are kept ten minutes in the cache, since Eastmoney throttles them hard.

use crate::http::{get_json, num};
use crate::model::now_ms;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// (name, Eastmoney list filter, what it is)
pub const SETS: &[(&str, &str, &str)] = &[
    ("cn.industry", "m:90+t:2", "A-share industry boards"),
    ("cn.concept", "m:90+t:3", "A-share concept boards"),
    ("hk.southbound", "b:MK0144,b:MK0146", "Hong Kong stocks in Stock Connect (southbound)"),
];

/// The columns a set's list carries, as (column label, Eastmoney field, scale).
const FIELDS: &[(&str, &str, f64)] = &[
    ("last", "f2", 1.0),
    ("chg(1d)", "f3", 1.0),
    ("chg(5d)", "f109", 1.0),
    ("chg(10d)", "f160", 1.0),
    ("flow(1d)", "f62", 1e-9),
    ("flow(5d)", "f164", 1e-9),
    ("flow(10d)", "f174", 1e-9),
];

/// The columns a table of a set shows by default: the ones its list carries.
pub fn default_columns() -> Vec<&'static str> {
    FIELDS.iter().map(|(l, _, _)| *l).collect()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Member {
    /// "NAME|id", as a board item.
    pub item: String,
    pub values: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    at: i64,
    members: Vec<Member>,
}

pub fn is_set(name: &str) -> bool {
    SETS.iter().any(|(n, _, _)| *n == name)
}

/// A member's id: Hong Kong stocks as Yahoo ids (0700.HK), the rest as Eastmoney market.code.
fn member_id(market: i64, code: &str) -> String {
    if market == 116 || market == 128 {
        let c = code.trim_start_matches('0');
        return format!("yahoo:{:0>4}.HK", c);
    }
    format!("eastmoney:{market}.{code}")
}

pub async fn members(name: &str) -> Result<Vec<Member>> {
    let (_, filter, _) = SETS.iter().find(|(n, _, _)| *n == name).with_context(|| format!("unknown set '{name}'"))?;
    let path = crate::config::home().join("cache").join(format!("set-{name}.json"));
    if let Some(c) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Cached>(&t).ok()) {
        if now_ms() - c.at < 10 * 60_000 {
            return Ok(c.members);
        }
    }
    let fields: Vec<&str> = FIELDS.iter().map(|(_, f, _)| *f).collect();
    let mut out: Vec<Member> = Vec::new();
    for page in 1..=30 {
        let url = format!(
            "https://push2.eastmoney.com/api/qt/clist/get?pn={page}&pz=100&po=1&np=1&fltt=2&invt=2&fid=f3&fs={filter}&fields=f12,f13,f14,{}",
            fields.join(",")
        );
        let v: Value = get_json(&url).await.with_context(|| format!("eastmoney list for {name}"))?;
        let Some(rows) = v["data"]["diff"].as_array() else {
            bail!("eastmoney returned an empty list for {name} (it throttles this list; try again in a few minutes)");
        };
        for r in rows {
            let (Some(code), Some(market), Some(label)) = (r["f12"].as_str(), r["f13"].as_i64(), r["f14"].as_str()) else { continue };
            let item = format!("{label}|{}", member_id(market, code));
            if out.iter().any(|m| m.item == item) {
                continue;
            }
            let values = FIELDS.iter().filter_map(|(l, f, k)| Some((l.to_string(), num(&r[*f])? * k))).collect();
            out.push(Member { item, values });
        }
        if out.len() as u64 >= v["data"]["total"].as_u64().unwrap_or(0) || rows.len() < 100 {
            break;
        }
    }
    if out.is_empty() {
        bail!("eastmoney list for {name} came back empty");
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_string(&Cached { at: now_ms(), members: out.clone() })?)?;
    Ok(out)
}
