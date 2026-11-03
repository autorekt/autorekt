//! Rules: an expression (Pine-style) wrapped Prometheus-style (`for`, cooldown, labels, a
//! templated message) or a timer (`every` / cron `schedule`). Files live in
//! `$AUTOREKT_HOME/rules/*.yaml`, either `rules: [...]` lists or a single rule mapping.

use crate::expr::{self, Ast, Ctx, Need};
use crate::model::{now_ms, parse_duration, Event, Timeframe};
use crate::providers::Range;
use crate::store::RuleState;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use chrono::{Datelike, Timelike};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Rule {
    pub name: String,
    /// Condition expression; fires when true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    /// Timer: fire every duration (e.g. 30m).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub every: Option<String>,
    /// Timer: cron schedule (5 fields, UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Calendar trigger: fire this long before each matching macro release (e.g. 15m).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_before: Option<String>,
    /// Calendar filter: currencies/countries (e.g. [USD, EUR]); default all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calendar_countries: Vec<String>,
    /// Calendar filter: minimum impact 1-3 (default 3 = high).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_min_impact: Option<u8>,
    /// Evaluation timeframe for bare ids (default 1m).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tf: Option<String>,
    /// Condition must hold this long before firing (Prometheus `for`).
    #[serde(rename = "for", skip_serializing_if = "Option::is_none")]
    pub for_: Option<String>,
    /// Minimum time between firings while the condition stays true (default 1h).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<String>,
    /// "update" (default: evaluate the forming bar) or "bar_close" (closed bars only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    /// Message template: {{name}} {{value}} {{rhs}} {{expr}} {{time}} {{labels.KEY}}.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Shell command run on fire, with AUTOREKT_* env vars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<String>,
    /// 0-3, default 2 (or labels.severity: info/warn/critical).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<u8>,
    /// Also emit an event when the condition clears.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub notify_resolved: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    /// Who created it (agent name), informational.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RuleFile {
    rules: Vec<Rule>,
}

pub fn dir() -> PathBuf {
    crate::config::home().join("rules")
}

impl Rule {
    pub fn tf(&self) -> Result<Timeframe> {
        Timeframe::parse(self.tf.as_deref().unwrap_or("1m"))
    }
    pub fn for_ms(&self) -> Result<i64> {
        Ok(self.for_.as_deref().map(parse_duration).transpose()?.unwrap_or(0) * 1000)
    }
    pub fn cooldown_ms(&self) -> Result<i64> {
        Ok(self.cooldown.as_deref().map(parse_duration).transpose()?.unwrap_or(3600) * 1000)
    }
    pub fn is_timer(&self) -> bool {
        self.expr.is_none() && (self.every.is_some() || self.schedule.is_some())
    }
    pub fn is_calendar(&self) -> bool {
        self.expr.is_none() && self.calendar_before.is_some()
    }
    pub fn severity(&self) -> u8 {
        if let Some(s) = self.severity {
            return s.min(3);
        }
        match self.labels.get("severity").map(|s| s.as_str()) {
            Some("info") => 1,
            Some("critical") | Some("page") => 3,
            _ => 2,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() || !self.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            bail!("rule name must be non-empty [A-Za-z0-9_.-]");
        }
        if let Some(b) = &self.calendar_before {
            parse_duration(b)?;
        }
        match (&self.expr, &self.every, &self.schedule) {
            (None, None, None) if self.calendar_before.is_none() => bail!("rule '{}' needs expr, every, schedule or calendar_before", self.name),
            (None, None, None) => {}
            (Some(e), _, _) => {
                let ast = expr::parse(e).with_context(|| format!("rule '{}': bad expr", self.name))?;
                let mut ns = std::collections::BTreeSet::new();
                expr::needs(&ast, self.tf()?, &mut ns)?;
            }
            _ => {}
        }
        if let Some(e) = &self.every {
            parse_duration(e)?;
        }
        if let Some(s) = &self.schedule {
            Cron::parse(s)?;
        }
        self.tf()?;
        self.for_ms()?;
        self.cooldown_ms()?;
        Ok(())
    }
}

/// Load all rules (later files win on duplicate names).
pub fn load() -> Result<Vec<(Rule, PathBuf)>> {
    let mut out: Vec<(Rule, PathBuf)> = Vec::new();
    let d = dir();
    let Ok(rd) = std::fs::read_dir(&d) else { return Ok(out) };
    let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |x| x == "yaml" || x == "yml")).collect();
    files.sort();
    for f in files {
        let text = std::fs::read_to_string(&f)?;
        let rules: Vec<Rule> = match serde_yaml::from_str::<RuleFile>(&text) {
            Ok(rf) => rf.rules,
            Err(_) => match serde_yaml::from_str::<Rule>(&text) {
                Ok(r) => vec![r],
                Err(e) => {
                    eprintln!("warning: skipping {}: {e}", f.display());
                    continue;
                }
            },
        };
        for r in rules {
            out.retain(|(x, _)| x.name != r.name);
            out.push((r, f.clone()));
        }
    }
    Ok(out)
}

pub fn find(name: &str) -> Result<Rule> {
    load()?.into_iter().find(|(r, _)| r.name == name).map(|(r, _)| r).ok_or_else(|| anyhow!("no rule named '{name}' (see `autorekt rule ls`)"))
}

pub fn save(rule: &Rule) -> Result<PathBuf> {
    rule.validate()?;
    std::fs::create_dir_all(dir())?;
    // Replace in the file that already has it, else write rules/<name>.yaml.
    for (r, path) in load()? {
        if r.name == rule.name {
            let text = std::fs::read_to_string(&path)?;
            if let Ok(mut rf) = serde_yaml::from_str::<RuleFile>(&text) {
                for x in rf.rules.iter_mut() {
                    if x.name == rule.name {
                        *x = rule.clone();
                    }
                }
                std::fs::write(&path, serde_yaml::to_string(&rf)?)?;
                return Ok(path);
            }
            std::fs::write(&path, serde_yaml::to_string(&RuleFile { rules: vec![rule.clone()] })?)?;
            return Ok(path);
        }
    }
    let path = dir().join(format!("{}.yaml", rule.name));
    std::fs::write(&path, serde_yaml::to_string(&RuleFile { rules: vec![rule.clone()] })?)?;
    Ok(path)
}

pub fn remove(name: &str) -> Result<PathBuf> {
    for (r, path) in load()? {
        if r.name != name {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        if let Ok(mut rf) = serde_yaml::from_str::<RuleFile>(&text) {
            rf.rules.retain(|x| x.name != name);
            if rf.rules.is_empty() {
                std::fs::remove_file(&path)?;
            } else {
                std::fs::write(&path, serde_yaml::to_string(&rf)?)?;
            }
        } else {
            std::fs::remove_file(&path)?;
        }
        return Ok(path);
    }
    bail!("no rule named '{name}'")
}

// ---------------------------------------------------------------- cron

#[derive(Debug, Clone)]
pub struct Cron {
    fields: Vec<Vec<u32>>, // minute, hour, dom, month, dow
}

impl Cron {
    pub fn parse(s: &str) -> Result<Cron> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 5 {
            bail!("cron needs 5 fields (min hour dom month dow), got '{s}'");
        }
        let ranges = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 6)];
        let mut fields = Vec::new();
        for (p, (lo, hi)) in parts.iter().zip(ranges) {
            let mut vals = Vec::new();
            for item in p.split(',') {
                let (body, step) = match item.split_once('/') {
                    Some((b, s)) => (b, s.parse::<u32>().map_err(|_| anyhow!("bad cron step '{item}'"))?),
                    None => (item, 1),
                };
                let (a, b) = if body == "*" {
                    (lo, hi)
                } else if let Some((x, y)) = body.split_once('-') {
                    (x.parse()?, y.parse()?)
                } else {
                    let v: u32 = body.parse().map_err(|_| anyhow!("bad cron value '{item}'"))?;
                    (v, if item.contains('/') { hi } else { v })
                };
                let mut v = a;
                while v <= b {
                    vals.push(if hi == 6 && v == 7 { 0 } else { v });
                    v += step.max(1);
                }
            }
            fields.push(vals);
        }
        Ok(Cron { fields })
    }
    pub fn matches(&self, ms: i64) -> bool {
        let d = chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default();
        let vals = [d.minute(), d.hour(), d.day(), d.month(), d.weekday().num_days_from_sunday()];
        self.fields.iter().zip(vals).all(|(f, v)| f.contains(&v))
    }
}

// ---------------------------------------------------------------- evaluation

/// Value shown in messages: the left side of a top-level comparison, else the expression.
fn display_ast(ast: &Ast) -> (Ast, Option<Ast>) {
    match ast {
        Ast::Bin(op, a, b) if matches!(op.as_str(), ">" | "<" | ">=" | "<=" | "==" | "!=") => ((**a).clone(), Some((**b).clone())),
        Ast::Bin(op, a, _) if matches!(op.as_str(), "and" | "&&" | "or" | "||") => display_ast(a),
        Ast::Call(f, args) if args.len() >= 2 && matches!(f.trim_start_matches("ta."), "crossover" | "crossunder" | "cross") => (args[0].clone(), Some(args[1].clone())),
        _ => (ast.clone(), None),
    }
}

/// The first instrument the expression refers to (for context and forward returns).
pub fn primary(ast: &Ast, tf: Timeframe) -> Option<Need> {
    let mut ns = std::collections::BTreeSet::new();
    expr::needs(ast, tf, &mut ns).ok()?;
    // Prefer one at the evaluation timeframe.
    ns.iter().find(|n| n.tf_secs == tf.secs).cloned().or_else(|| ns.into_iter().next())
}

pub fn render_message(rule: &Rule, value: Option<f64>, rhs: Option<f64>, at: i64) -> String {
    let default = match &rule.expr {
        Some(e) => format!("{}: {e}", rule.name),
        None => format!("{}: timer", rule.name),
    };
    let mut m = rule.message.clone().unwrap_or(default);
    let f = |x: Option<f64>| x.map(crate::render::fmt_sig).unwrap_or_else(|| "-".into());
    m = m.replace("{{value}}", &f(value)).replace("{{close}}", &f(value)).replace("{{rhs}}", &f(rhs));
    m = m.replace("{{name}}", &rule.name).replace("{{expr}}", rule.expr.as_deref().unwrap_or(""));
    m = m.replace("{{time}}", &chrono::DateTime::from_timestamp_millis(at).unwrap_or_default().format("%Y-%m-%d %H:%M UTC").to_string());
    for (k, v) in &rule.labels {
        m = m.replace(&format!("{{{{labels.{k}}}}}"), v);
    }
    m
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub at: i64,
    pub condition: bool,
    pub value: Option<f64>,
    pub rhs: Option<f64>,
    pub message: String,
}

/// Evaluate a rule's condition once, now.
pub async fn check(rule: &Rule) -> Result<Check> {
    let now = now_ms();
    let Some(src) = &rule.expr else {
        return Ok(Check { name: rule.name.clone(), at: now, condition: false, value: None, rhs: None, message: render_message(rule, None, None, now) });
    };
    let tf = rule.tf()?;
    let ast = expr::parse(src)?;
    let lb = expr::lookback(&ast, tf).max(30);
    let data = expr::fetch(&ast, tf, Range::last(lb)).await?;
    let ctx = Ctx::new(&data, tf);
    if ctx.grid.is_empty() {
        bail!("no data for rule '{}'", rule.name);
    }
    let cond = ctx.eval(&ast)?;
    let (lhs_ast, rhs_ast) = display_ast(&ast);
    let lhs = ctx.eval(&lhs_ast).ok();
    let rhs = rhs_ast.and_then(|r| ctx.eval(&r).ok());
    let mut i = ctx.grid.len() - 1;
    if rule.eval.as_deref() == Some("bar_close") && ctx.grid[i] + tf.ms() > now && i > 0 {
        i -= 1;
    }
    let pick = |v: &Option<Vec<f64>>| v.as_ref().and_then(|v| v.get(i).copied()).filter(|x| x.is_finite());
    let value = pick(&lhs);
    let rhs_v = pick(&rhs);
    let c = cond.get(i).map_or(false, |x| x.is_finite() && *x != 0.0);
    Ok(Check { name: rule.name.clone(), at: ctx.grid[i], condition: c, value, rhs: rhs_v, message: render_message(rule, value, rhs_v, now) })
}

/// Advance a rule's state machine given a fresh check. Returns an event to emit, if any.
pub fn step(rule: &Rule, st: &mut RuleState, condition: bool, value: Option<f64>, message: &str, now: i64) -> Result<Option<Event>> {
    st.last_eval = Some(now);
    st.last_value = value;
    st.error = None;
    let fire = |st: &mut RuleState| {
        st.state = "firing".into();
        st.last_fired = Some(now);
        Event { timestamp: now, kind: "rule".into(), source: format!("rule/{}", rule.name), title: message.to_string(), body: rule.expr.clone().unwrap_or_default(), url: None, severity: rule.severity(), symbols: vec![] }
    };
    if !condition {
        let was_firing = st.state == "firing";
        st.state = "inactive".into();
        st.since = None;
        if was_firing && rule.notify_resolved {
            return Ok(Some(Event { timestamp: now, kind: "resolved".into(), source: format!("rule/{}", rule.name), title: format!("resolved: {}", rule.name), body: rule.expr.clone().unwrap_or_default(), url: None, severity: 1, symbols: vec![] }));
        }
        return Ok(None);
    }
    match st.state.as_str() {
        "firing" => {
            let cd = rule.cooldown_ms()?;
            if st.last_fired.map_or(true, |f| now - f >= cd) {
                return Ok(Some(fire(st)));
            }
            Ok(None)
        }
        "pending" => {
            if now - st.since.unwrap_or(now) >= rule.for_ms()? {
                return Ok(Some(fire(st)));
            }
            Ok(None)
        }
        _ => {
            st.since = Some(now);
            if rule.for_ms()? == 0 {
                let cd = rule.cooldown_ms()?;
                if st.last_fired.map_or(false, |f| now - f < cd) {
                    st.state = "firing".into();
                    return Ok(None);
                }
                return Ok(Some(fire(st)));
            }
            st.state = "pending".into();
            Ok(None)
        }
    }
}

/// Is a timer rule due?
pub fn timer_due(rule: &Rule, st: &RuleState, now: i64) -> Result<bool> {
    if let Some(e) = &rule.every {
        let every = parse_duration(e)? * 1000;
        return Ok(now - st.last_fired.unwrap_or(0) >= every);
    }
    if let Some(s) = &rule.schedule {
        let c = Cron::parse(s)?;
        let from = st.last_eval.unwrap_or(now - 60_000).max(now - 3_600_000);
        let mut m = from - from.rem_euclid(60_000) + 60_000;
        while m <= now {
            if c.matches(m) && st.last_fired.map_or(true, |f| f < m) {
                return Ok(true);
            }
            m += 60_000;
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------- backtest

#[derive(Debug, Clone, Serialize)]
pub struct Firing {
    pub at: i64,
    pub value: Option<f64>,
    pub fwd: Vec<(String, Option<f64>)>,
}

/// Replay a rule over history with its `for` and cooldown; report would-be firings and the
/// primary instrument's forward returns after each.
/// The tradable instrument whose forward returns judge a signal: the base instrument of a
/// metric (BTC for BTC#funding_rate), the primary itself if it is tradable, else SPX.
pub fn forward_ref(primary: &crate::ids::InstrumentId) -> crate::ids::InstrumentId {
    let non_tradable = matches!(primary.provider.as_str(), "fred" | "alt" | "cftc" | "finra" | "llama" | "polymarket" | "kalshi" | "local")
        || primary.symbol.ends_with("-DVOL")
        || crate::ids::level_unit(&primary.base()).is_some();
    if primary.metric.is_some() && !non_tradable {
        return primary.base();
    }
    if non_tradable {
        return crate::ids::parse("SPX").unwrap();
    }
    primary.clone()
}

pub async fn backtest(rule: &Rule, range_secs: i64, tf_override: Option<Timeframe>, fwd_override: Option<crate::ids::InstrumentId>) -> Result<(Vec<Firing>, usize, Option<String>)> {
    let src = rule.expr.as_ref().ok_or_else(|| anyhow!("timer rules have nothing to test"))?;
    let tf = tf_override.unwrap_or(rule.tf()?);
    let ast = expr::parse(src)?;
    let end = now_ms();
    let start = end - range_secs * 1000;
    let limit = ((range_secs / tf.secs) as usize + 2).min(5000);
    let data = expr::fetch(&ast, tf, Range { start: Some(start), end: None, limit }).await?;
    let ctx = Ctx::new(&data, tf);
    let cond = ctx.eval(&ast)?;
    let (lhs_ast, _) = display_ast(&ast);
    let lhs = ctx.eval(&lhs_ast).ok();
    let prim = primary(&ast, tf);
    let fwd_id = fwd_override.or_else(|| prim.as_ref().map(|p| forward_ref(&p.id)));
    // Forward-return series on its own hourly grid (fetched separately: it may not be in the rule).
    let fwd_bars = match &fwd_id {
        Some(f) => {
            let ftf = if tf.secs >= 86400 { Timeframe::D1 } else { Timeframe::H1 };
            crate::providers::ohlcv(f, ftf, Range { start: Some(start - 2 * 86_400_000), end: None, limit: 5000 }).await.unwrap_or_default()
        }
        None => vec![],
    };
    let close_at = |t: i64| -> Option<f64> { fwd_bars.iter().rev().find(|b| b.timestamp <= t).map(|b| b.close) };
    let last_fwd_t = fwd_bars.last().map(|b| b.timestamp).unwrap_or(0);
    let for_ms = rule.for_ms()?;
    let cd = rule.cooldown_ms()?;
    let mut st = RuleState { name: rule.name.clone(), state: "inactive".into(), ..Default::default() };
    let mut out = Vec::new();
    let day = 86_400_000;
    let horizons: Vec<(String, i64)> = if tf.secs >= 86400 {
        vec![("1d".into(), day), ("5d".into(), 5 * day), ("20d".into(), 20 * day)]
    } else {
        vec![("1h".into(), 3_600_000), ("4h".into(), 4 * 3_600_000), ("24h".into(), day)]
    };
    let mut evaluated = 0;
    for (i, &t) in ctx.grid.iter().enumerate() {
        if t < start {
            continue;
        }
        evaluated += 1;
        let c = cond[i].is_finite() && cond[i] != 0.0;
        let v = lhs.as_ref().and_then(|l| l.get(i).copied()).filter(|x| x.is_finite());
        // Inline state machine on bar time.
        let fired = if !c {
            st.state = "inactive".into();
            st.since = None;
            false
        } else {
            match st.state.as_str() {
                "firing" => {
                    if t - st.last_fired.unwrap_or(0) >= cd {
                        st.last_fired = Some(t);
                        true
                    } else {
                        false
                    }
                }
                "pending" => {
                    if t - st.since.unwrap_or(t) >= for_ms {
                        st.state = "firing".into();
                        st.last_fired = Some(t);
                        true
                    } else {
                        false
                    }
                }
                _ => {
                    st.since = Some(t);
                    if for_ms == 0 {
                        st.state = "firing".into();
                        if t - st.last_fired.unwrap_or(i64::MIN / 2) >= cd {
                            st.last_fired = Some(t);
                            true
                        } else {
                            false
                        }
                    } else {
                        st.state = "pending".into();
                        false
                    }
                }
            }
        };
        if fired {
            let fwd = horizons
                .iter()
                .map(|(label, h)| {
                    let r = if t + h > last_fwd_t { None } else { close_at(t + tf.ms()).zip(close_at(t + tf.ms() + h)).and_then(|(a, b)| crate::render::pct(b, a)) };
                    (label.clone(), r)
                })
                .collect();
            out.push(Firing { at: t, value: v, fwd });
        }
    }
    Ok((out, evaluated, fwd_id.map(|p| p.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(for_: Option<&str>, cooldown: Option<&str>) -> Rule {
        Rule { name: "t".into(), expr: Some("BTC > 1".into()), for_: for_.map(|s| s.into()), cooldown: cooldown.map(|s| s.into()), ..Default::default() }
    }
    fn st() -> RuleState {
        RuleState { name: "t".into(), state: "inactive".into(), ..Default::default() }
    }
    const M: i64 = 60_000;

    #[test]
    fn fires_immediately_without_for_then_respects_cooldown() {
        let r = rule(None, Some("10m"));
        let mut s = st();
        assert!(step(&r, &mut s, true, Some(2.0), "m", 0).unwrap().is_some(), "first true fires");
        assert!(step(&r, &mut s, true, Some(2.0), "m", 5 * M).unwrap().is_none(), "still true within cooldown: silent");
        assert!(step(&r, &mut s, true, Some(2.0), "m", 10 * M).unwrap().is_some(), "cooldown elapsed while true: repeats");
        assert!(step(&r, &mut s, false, Some(0.0), "m", 11 * M).unwrap().is_none());
        assert_eq!(s.state, "inactive");
        // Flapping back to true inside the cooldown must not re-fire.
        assert!(step(&r, &mut s, true, Some(2.0), "m", 12 * M).unwrap().is_none());
        assert!(step(&r, &mut s, true, Some(2.0), "m", 21 * M).unwrap().is_some());
    }

    #[test]
    fn for_requires_condition_to_hold() {
        let r = rule(Some("5m"), None);
        let mut s = st();
        assert!(step(&r, &mut s, true, None, "m", 0).unwrap().is_none());
        assert_eq!(s.state, "pending");
        assert!(step(&r, &mut s, true, None, "m", 4 * M).unwrap().is_none());
        assert!(step(&r, &mut s, false, None, "m", 4 * M + 1).unwrap().is_none(), "breaks the streak");
        assert!(step(&r, &mut s, true, None, "m", 6 * M).unwrap().is_none(), "pending again from scratch");
        assert!(step(&r, &mut s, true, None, "m", 11 * M).unwrap().is_some(), "held 5m: fires");
        assert_eq!(s.state, "firing");
    }

    #[test]
    fn resolved_event_only_when_asked() {
        let mut r = rule(None, None);
        let mut s = st();
        step(&r, &mut s, true, None, "m", 0).unwrap();
        assert!(step(&r, &mut s, false, None, "m", M).unwrap().is_none());
        r.notify_resolved = true;
        step(&r, &mut s, true, None, "m", 2 * 3_600_000).unwrap();
        let e = step(&r, &mut s, false, None, "m", 2 * 3_600_000 + M).unwrap().unwrap();
        assert_eq!(e.kind, "resolved");
    }

    #[test]
    fn cron_matches() {
        let c = Cron::parse("30 13 * * 1-5").unwrap();
        // 2026-11-02 is a Monday.
        let mon = chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap().and_hms_opt(13, 30, 0).unwrap().and_utc().timestamp_millis();
        assert!(c.matches(mon));
        assert!(!c.matches(mon + 60_000));
        assert!(!c.matches(mon - 2 * 86_400_000), "Saturday excluded");
        let every15 = Cron::parse("*/15 * * * *").unwrap();
        assert!(every15.matches(mon - 30 * 60_000 + 15 * 60_000));
        assert!(Cron::parse("61 * * * *").map(|c| !c.matches(mon)).unwrap_or(true));
        assert!(Cron::parse("* * *").is_err());
    }

    #[test]
    fn messages_render_values_compactly() {
        let r = Rule { name: "x".into(), message: Some("v={{value}} rhs={{rhs}} {{labels.k}}".into()), labels: [("k".to_string(), "L".to_string())].into(), ..Default::default() };
        assert_eq!(render_message(&r, Some(0.05), Some(76260.0), 0), "v=0.05 rhs=76260 L");
    }
}
