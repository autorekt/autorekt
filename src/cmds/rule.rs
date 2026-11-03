//! Rule management: add, list, show, remove, enable/disable, check now, and backtest
//! with forward returns of the tradable instrument behind the signal.

use super::*;

fn rule_or_adhoc(target: &str, tf: Option<String>) -> Result<rules::Rule> {
    if let Ok(r) = rules::find(target) {
        let mut r = r;
        if tf.is_some() {
            r.tf = tf;
        }
        return Ok(r);
    }
    if expr::parse(target).is_ok() && (expr::is_expression(target) || ids::parse(target).is_ok()) {
        return Ok(rules::Rule { name: "adhoc".into(), expr: Some(target.to_string()), tf, ..Default::default() });
    }
    bail!("'{target}' is neither a rule name nor a valid expression")
}

pub async fn rule(op: RuleOp, g: &Global, o: &Opts) -> Result<()> {
    match op {
        RuleOp::Add(rule, check) => {
            rule.validate()?;
            if check && rule.expr.is_some() {
                // Catch bad ids / unreachable data now rather than in the daemon log.
                let c = rules::check(&rule).await.context("rule does not evaluate (fix it, or pass --no-check)")?;
                println!("check now: condition={} value={} -> \"{}\"", c.condition, c.value.map(render::fmt_auto).unwrap_or("-".into()), c.message);
            }
            let path = rules::save(&rule)?;
            println!("saved rule '{}' to {}", rule.name, path.display());
            let i = daemon::ensure().await?;
            println!("daemon running (pid {}); it evaluates rules every ~{}", i.pid, rule.tf().map(|t| if t.secs < 60 { "15s".to_string() } else { "60s".into() }).unwrap_or_default());
            if rule.hook.is_none() {
                println!("no --hook: firings are logged; use `autorekt wait` or `autorekt diff` to receive them");
            }
        }
        RuleOp::Ls => {
            let c = store::open()?;
            let list = rules::load()?;
            if g.json {
                let v: Vec<_> = list.iter().map(|(r, p)| json!({"rule": r, "file": p, "state": store::get_rule_state(&c, &r.name).unwrap_or_default()})).collect();
                return print_json(&v);
            }
            if list.is_empty() {
                println!("no rules (add one: autorekt rule add NAME --expr '...')");
                return Ok(());
            }
            let mut rows = vec![vec!["name".to_string(), "state".into(), "value".into(), "last fired".into(), "last eval".into(), "rule".into()]];
            for (r, _) in &list {
                let s = store::get_rule_state(&c, &r.name)?;
                let what = match (&r.expr, &r.every, &r.schedule) {
                    (None, None, None) if r.calendar_before.is_some() => format!("{} before {} macro releases{}", r.calendar_before.clone().unwrap_or_default(), ["any", "low+", "medium+", "high"][r.calendar_min_impact.unwrap_or(3).min(3) as usize], if r.calendar_countries.is_empty() { String::new() } else { format!(" ({})", r.calendar_countries.join(",")) }),
                    (Some(e), _, _) => format!("{e}{}{}", r.for_.as_ref().map(|f| format!(" for {f}")).unwrap_or_default(), r.tf.as_ref().map(|t| format!(" @{t}")).unwrap_or_default()),
                    (None, Some(e), _) => format!("every {e}"),
                    (None, None, Some(s)) => format!("cron {s}"),
                    _ => String::new(),
                };
                let state = if r.disabled { "disabled".to_string() } else if s.error.is_some() { "ERROR".into() } else { s.state.clone() };
                rows.push(vec![
                    r.name.clone(),
                    state,
                    s.last_value.map(render::fmt_auto).unwrap_or("-".into()),
                    s.last_fired.map(ago).unwrap_or("never".into()),
                    s.last_eval.map(ago).unwrap_or("-".into()),
                    what,
                ]);
                if let Some(e) = &s.error {
                    rows.push(vec!["".into(), "".into(), "".into(), "".into(), "".into(), format!("  error: {}", e.lines().next().unwrap_or(""))]);
                }
            }
            print!("{}", render::table(&rows));
            if daemon::running().is_none() {
                println!("(daemon not running — start it with `autorekt daemon start`)");
            }
        }
        RuleOp::Show(name) => {
            let r = rules::find(&name)?;
            let s = store::get_rule_state(&store::open()?, &name)?;
            if g.json {
                return print_json(&json!({"rule": r, "state": s}));
            }
            print!("{}", serde_yaml::to_string(&r)?);
            println!("# state: {} | last fired: {} | last value: {}", s.state, s.last_fired.map(ago).unwrap_or("never".into()), s.last_value.map(render::fmt_auto).unwrap_or("-".into()));
        }
        RuleOp::Rm(name) => {
            let p = rules::remove(&name)?;
            println!("removed '{name}' ({})", p.display());
        }
        RuleOp::Enable(name, on) => {
            let mut r = rules::find(&name)?;
            r.disabled = !on;
            rules::save(&r)?;
            println!("{} '{name}'", if on { "enabled" } else { "disabled" });
        }
        RuleOp::Check(target, tf) => {
            let r = rule_or_adhoc(&target, tf)?;
            let c = rules::check(&r).await?;
            if g.json {
                return print_json(&c);
            }
            println!(
                "{}  at {}  condition={}  value={}  rhs={}\nmessage: {}",
                r.name,
                o.full_time(c.at),
                c.condition,
                c.value.map(render::fmt_auto).unwrap_or("-".into()),
                c.rhs.map(render::fmt_auto).unwrap_or("-".into()),
                c.message
            );
        }
        RuleOp::Test(target, range, tf, for_, cooldown, fwd) => {
            let mut r = rule_or_adhoc(&target, tf.clone())?;
            if for_.is_some() {
                r.for_ = for_;
            }
            if cooldown.is_some() {
                r.cooldown = cooldown;
            }
            let secs = parse_duration(&range)?;
            let fwd_id = fwd.as_deref().map(ids::parse).transpose()?;
            let (fires, n, prim) = rules::backtest(&r, secs, tf.as_deref().map(Timeframe::parse).transpose()?, fwd_id).await?;
            if g.json {
                return print_json(&json!({"rule": r.name, "bars": n, "primary": prim, "firings": fires}));
            }
            let tfl = r.tf.clone().unwrap_or_else(|| "1m".into());
            println!(
                "backtest {}  expr: {}  tf {}  range {}  for {}  cooldown {}  ->  {} firing(s) over {} bars",
                r.name,
                r.expr.clone().unwrap_or_default(),
                tfl,
                range,
                r.for_.clone().unwrap_or_else(|| "0".into()),
                r.cooldown.clone().unwrap_or_else(|| "1h".into()),
                fires.len(),
                n
            );
            if fires.is_empty() {
                println!("never fired; loosen the threshold or extend --range");
                return Ok(());
            }
            let hl: Vec<String> = fires.first().map(|f| f.fwd.iter().map(|x| x.0.clone()).collect()).unwrap_or_default();
            let fwd_label = prim.clone().map(|p| ids::label(&ids::parse(&p).unwrap())).unwrap_or("fwd".into());
            let mut rows = vec![std::iter::once("time".to_string()).chain(std::iter::once("value".to_string())).chain(hl.iter().enumerate().map(|(k, h)| if k == 0 { format!("{fwd_label} +{h}") } else { format!("+{h}") })).collect::<Vec<_>>()];
            for f in fires.iter().rev().take(40).rev() {
                let mut row = vec![o.full_time(f.at), f.value.map(render::fmt_auto).unwrap_or("-".into())];
                for (_, v) in &f.fwd {
                    row.push(render::fmt_pct(*v));
                }
                rows.push(row);
            }
            print!("{}", render::table(&rows));
            if fires.len() > 40 {
                println!("... {} earlier firings not shown", fires.len() - 40);
            }
            // Average forward returns: does the signal mean anything?
            for (k, label) in hl.iter().map(|h| format!("+{h}")).enumerate() {
                let xs: Vec<f64> = fires.iter().filter_map(|f| f.fwd.get(k).and_then(|x| x.1)).collect();
                if !xs.is_empty() {
                    let up = xs.iter().filter(|x| **x > 0.0).count();
                    print!("{label}: mean {:+.2}% median {:+.2}% up {}/{}   ", render::mean(&xs), median(&xs), up, xs.len());
                }
            }
            println!();
        }
    }
    Ok(())
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if s.is_empty() {
        return f64::NAN;
    }
    s[s.len() / 2]
}
