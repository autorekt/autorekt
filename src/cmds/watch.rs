//! `watch`: everything about being alerted. The flags select the action: define a rule
//! (--expr, --every, --cron, --calendar), --test, --rm, --enable/--disable, --wait, --python;
//! with none, a name shows that rule and no name lists rules and monitors.

use super::*;
use crate::cli::WatchArgs;
use rule::RuleOp;

pub async fn watch(a: WatchArgs, g: &Global, o: &Opts) -> Result<()> {
    let defines = a.expr.is_some() || a.every.is_some() || a.cron.is_some() || a.calendar.is_some();
    if a.wait {
        return alerts::wait(a.target, a.timeout, a.min_severity.unwrap_or(0), g, o).await;
    }
    let Some(target) = a.target.clone() else {
        if defines || a.test || a.rm || a.python {
            bail!("give a rule name (or with --test an expression), e.g. `autorekt watch btc_dump --expr 'pct(BTC, \"1h\") < -3'`");
        }
        rule::rule(RuleOp::Ls, g, o).await?;
        if !g.json {
            println!();
            monitor::monitor_ls(g).await?;
        }
        return Ok(());
    };
    if a.python {
        return monitor::monitor_new(&target);
    }
    if target.ends_with(".md") {
        let (file, n) = journal::rules(&target)?;
        println!("{n} rules from {target} written to {} (run this again after editing the journal)", file.display());
        return Ok(());
    }
    if a.rm {
        let monitor = config::home().join("monitors").join(format!("{target}.py"));
        if rules::find(&target).is_err() && monitor.exists() {
            return monitor::monitor_rm(&target);
        }
        return rule::rule(RuleOp::Rm(target), g, o).await;
    }
    if a.test {
        return match &a.range {
            Some(r) => rule::rule(RuleOp::Test(target, r.clone(), a.tf, a.for_, a.cooldown, a.fwd), g, o).await,
            None => rule::rule(RuleOp::Check(target, a.tf), g, o).await,
        };
    }
    if defines {
        let labels = a.label.iter().filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
        let r = rules::Rule {
            name: target,
            expr: a.expr,
            every: a.every,
            schedule: a.cron,
            calendar_before: a.calendar,
            calendar_countries: a.countries.map(|c| c.split(',').map(|x| x.trim().to_uppercase()).collect()).unwrap_or_default(),
            calendar_min_impact: a.min_severity,
            tf: a.tf,
            for_: a.for_,
            cooldown: a.cooldown,
            eval: a.bar_close.then(|| "bar_close".to_string()),
            labels,
            message: a.message,
            hook: a.hook,
            severity: a.severity,
            notify_resolved: a.notify_resolved,
            disabled: a.disable,
            owner: a.agent,
        };
        return rule::rule(RuleOp::Add(r, !a.no_check), g, o).await;
    }
    if a.enable || a.disable {
        return rule::rule(RuleOp::Enable(target, a.enable), g, o).await;
    }
    rule::rule(RuleOp::Show(target), g, o).await
}
