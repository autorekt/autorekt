//! Background loops of the daemon: rule and timer evaluation with hooks, the monitor
//! supervisor, periodic recorders for metrics without public history, and the OKX
//! liquidation collector.

use super::*;
use crate::http::{get_json, num};

pub(super) async fn rule_loop() {
    let mut next_due: HashMap<String, i64> = HashMap::new();
    loop {
        let now = now_ms();
        let list = rules::load().unwrap_or_default();
        let mut tasks = Vec::new();
        for (rule, _) in list {
            if rule.disabled {
                continue;
            }
            // Calendar rules fire inside a 10-minute window, so a 5-minute cadence is enough
            // and keeps the rate-limited calendar host happy.
            let interval = match rule.tf().map(|t| t.secs).unwrap_or(60) {
                _ if rule.is_calendar() => 300_000,
                s if s <= 60 => 15_000,
                s if s <= 900 => 30_000,
                s if s < 86400 => 60_000,
                _ => 300_000,
            };
            if next_due.get(&rule.name).map_or(false, |d| *d > now) {
                continue;
            }
            next_due.insert(rule.name.clone(), now + interval);
            tasks.push(tokio::spawn(async move {
                if let Err(e) = eval_rule(&rule).await {
                    log(&format!("rule {} error: {e:#}", rule.name));
                    if let Ok(c) = store::open() {
                        let mut s = store::get_rule_state(&c, &rule.name).unwrap_or_default();
                        s.name = rule.name.clone();
                        s.error = Some(format!("{e:#}"));
                        s.last_eval = Some(now_ms());
                        let _ = store::set_rule_state(&c, &s);
                    }
                }
            }));
        }
        for t in tasks {
            let _ = tokio::time::timeout(Duration::from_secs(90), t).await;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

pub(super) async fn eval_rule(rule: &rules::Rule) -> Result<()> {
    let now = now_ms();
    let c = store::open()?;
    let mut st = store::get_rule_state(&c, &rule.name)?;
    st.name = rule.name.clone();
    st.error = None;
    let ev = if rule.is_calendar() {
        st.last_eval = Some(now);
        let before = crate::model::parse_duration(rule.calendar_before.as_deref().unwrap_or("15m"))? * 1000;
        let min_imp = rule.calendar_min_impact.unwrap_or(3);
        let cal = calendar_cached().await?;
        let mut hit = None;
        for e in cal {
            if e.severity < min_imp || e.timestamp - before > now || e.timestamp - before < now - 600_000 {
                continue;
            }
            if !rule.calendar_countries.is_empty() && !rule.calendar_countries.iter().any(|c| c.eq_ignore_ascii_case(&e.source)) {
                continue;
            }
            let key = format!("cal:{}:{}", e.timestamp, e.title);
            if store::get_cursor(&c, &rule.name, &key)?.is_some() {
                continue;
            }
            store::set_cursor(&c, &rule.name, &key, "1")?;
            let mins = ((e.timestamp - now) / 60_000).max(0);
            let msg = match &rule.message {
                Some(m) => m.replace("{{event}}", &e.title).replace("{{country}}", &e.source).replace("{{numbers}}", &e.body).replace("{{minutes}}", &mins.to_string()),
                None => format!("In {mins}m: {} {}{}", e.source, e.title, if e.body.is_empty() { String::new() } else { format!(" ({})", e.body) }),
            };
            st.last_fired = Some(now);
            st.state = "firing".into();
            hit = Some((Event { timestamp: now, kind: "calendar".into(), source: format!("rule/{}", rule.name), title: msg, body: e.body.clone(), url: None, severity: rule.severity(), symbols: vec![] }, None, None));
            break;
        }
        hit
    } else if rule.is_timer() {
        if rules::timer_tick(rule, &mut st, now)? {
            let msg = rules::render_message(rule, None, None, now);
            Some((Event { timestamp: now, kind: "timer".into(), source: format!("rule/{}", rule.name), title: msg, body: String::new(), url: None, severity: rule.severity.unwrap_or(1).min(3), symbols: vec![] }, None, None))
        } else {
            None
        }
    } else {
        drop(c);
        let chk = rules::check(rule).await?;
        let c = store::open()?;
        let e = rules::step(rule, &mut st, chk.condition, chk.value, &chk.message, now)?;
        store::set_rule_state(&c, &st)?;
        e.map(|e| (e, chk.value, chk.rhs))
    };
    let c = store::open()?;
    store::set_rule_state(&c, &st)?;
    if let Some((e, value, rhs)) = ev {
        let id = store::add_event(&c, &e, Some(&rule.name))?;
        log(&format!("fired {} #{id}: {}", rule.name, e.title));
        if let Some(h) = &rule.hook {
            run_hook(h, rule, &e, value, rhs).await;
        }
    }
    Ok(())
}

/// The macro calendar, cached for an hour.
pub(super) async fn calendar_cached() -> Result<Vec<Event>> {
    static CAL: std::sync::LazyLock<Mutex<(i64, Vec<Event>)>> = std::sync::LazyLock::new(|| Mutex::new((0, vec![])));
    let mut g = CAL.lock().await;
    if now_ms() - g.0 > 3_600_000 || g.1.is_empty() {
        g.1 = crate::providers::news::calendar().await?;
        g.0 = now_ms();
    }
    Ok(g.1.clone())
}

/// Short text context for hooks: recent candles of the rule's primary instrument.
pub(super) async fn context_text(rule: &rules::Rule) -> String {
    let Some(src) = &rule.expr else { return String::new() };
    let Ok(ast) = crate::expr::parse(src) else { return String::new() };
    let tf = rule.tf().unwrap_or(Timeframe::M1);
    let Some(p) = rules::primary(&ast, tf) else { return String::new() };
    let view_tf = if tf.secs < 900 { Timeframe { secs: 900 } } else { tf };
    let Ok(bars) = providers::ohlcv(&p.id, view_tf, Range::last(12)).await else { return String::new() };
    let o = crate::render::Opts { tz: chrono_tz::UTC, width: 20, ascii: true };
    crate::render::candles::render(&crate::render::candles::CandleView { title: p.id.to_string(), tf_label: view_tf.to_string(), bars: &bars, step_ms: view_tf.ms(), notes: vec![], events: &[], extra: &[] }, &o)
}

pub(super) async fn run_hook(cmd: &str, rule: &rules::Rule, e: &Event, value: Option<f64>, rhs: Option<f64>) {
    let ctx = context_text(rule).await;
    let mut chart_url = String::new();
    if config::get().hook_chart_url {
        if let Some(p) = rule.expr.as_deref().and_then(|s| crate::expr::parse(s).ok()).and_then(|ast| rules::primary(&ast, rule.tf().unwrap_or(Timeframe::M1))) {
            match create_share_local(&p.id.to_string(), "candles", "15m", 96, 86400 * 3).await {
                Ok(v) => chart_url = v,
                Err(e) => log(&format!("hook {}: chart link failed: {e:#}", rule.name)),
            }
        }
    }
    let mut c = crate::os::shell(cmd);
    c.env("AUTOREKT_EVENT", "rule")
        .env("AUTOREKT_RULE", &rule.name)
        .env("AUTOREKT_EXPR", rule.expr.as_deref().unwrap_or(""))
        .env("AUTOREKT_MESSAGE", &e.title)
        .env("AUTOREKT_TIME", chrono::DateTime::from_timestamp_millis(e.timestamp).unwrap_or_default().to_rfc3339())
        .env("AUTOREKT_SEVERITY", e.severity.to_string())
        .env("AUTOREKT_VALUE", value.map(|v| v.to_string()).unwrap_or_default())
        .env("AUTOREKT_RHS", rhs.map(|v| v.to_string()).unwrap_or_default())
        .env("AUTOREKT_CONTEXT", &ctx)
        .env("AUTOREKT_CHART_URL", &chart_url);
    if let Some(o) = &rule.owner {
        c.env("AUTOREKT_AGENT", o);
    }
    for (k, v) in &rule.labels {
        c.env(format!("AUTOREKT_LABEL_{}", k.to_uppercase()), v);
    }
    c.stdin(std::process::Stdio::null());
    match tokio::time::timeout(Duration::from_secs(120), c.output()).await {
        Ok(Ok(out)) => log(&format!(
            "hook {} exit={} out={} err={}",
            rule.name,
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).chars().take(300).collect::<String>().replace('\n', " "),
            String::from_utf8_lossy(&out.stderr).chars().take(300).collect::<String>().replace('\n', " ")
        )),
        Ok(Err(e)) => log(&format!("hook {} failed to start: {e}", rule.name)),
        Err(_) => log(&format!("hook {} timed out", rule.name)),
    }
}

/// Supervise monitor scripts in $AUTOREKT_HOME/monitors (python files or executables).
pub(super) async fn monitor_loop(st: St) {
    let mut children: HashMap<String, tokio::process::Child> = HashMap::new();
    let mut backoff: HashMap<String, (i64, u32)> = HashMap::new();
    loop {
        let dir = config::home().join("monitors");
        let mut want: HashMap<String, PathBuf> = HashMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                if name.is_empty() || name.starts_with('.') || name.starts_with('_') || p.is_dir() {
                    continue;
                }
                let is_py = p.extension().map_or(false, |x| x == "py");
                if is_py || crate::os::is_executable(&p) {
                    want.insert(name, p);
                }
            }
        }
        // Reap exited, stop removed.
        let names: Vec<String> = children.keys().cloned().collect();
        for n in names {
            let gone = !want.contains_key(&n);
            let ch = children.get_mut(&n).unwrap();
            if gone {
                if let Some(pid) = ch.id() {
                    crate::os::end_group(pid);
                }
                children.remove(&n);
                st.monitors.lock().await.remove(&n);
                log(&format!("monitor {n} removed"));
                continue;
            }
            if let Ok(Some(status)) = ch.try_wait() {
                log(&format!("monitor {n} exited: {status}"));
                let mut ms = st.monitors.lock().await;
                if let Some(m) = ms.get_mut(&n) {
                    m.pid = None;
                    m.last_exit = Some(status.to_string());
                }
                children.remove(&n);
                let b = backoff.entry(n.clone()).or_insert((0, 0));
                b.1 += 1;
                b.0 = now_ms() + (5_000i64 * 2i64.pow(b.1.min(6))).min(600_000);
            }
        }
        // Start missing.
        for (n, p) in want {
            if children.contains_key(&n) {
                continue;
            }
            if backoff.get(&n).map_or(false, |b| b.0 > now_ms()) {
                continue;
            }
            let log_path = logs().join(format!("monitor-{n}.log"));
            let Ok(logf) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) else { continue };
            let is_py = p.extension().map_or(false, |x| x == "py");
            let mut cmd = if is_py {
                let mut c = tokio::process::Command::new(crate::os::PYTHON);
                c.arg("-u").arg(&p);
                c
            } else {
                tokio::process::Command::new(&p)
            };
            let sdk = config::home().join("sdk");
            let others = std::env::var_os("PYTHONPATH").unwrap_or_default();
            let Ok(pypath) = std::env::join_paths(std::iter::once(sdk).chain(std::env::split_paths(&others))) else { continue };
            cmd.current_dir(config::home().join("monitors"))
                .env("AUTOREKT_HOME", config::home())
                .env("AUTOREKT_MONITOR", &n)
                .env("PYTHONPATH", pypath)
                .stdin(std::process::Stdio::null())
                .stdout(logf.try_clone().unwrap())
                .stderr(logf)
                .kill_on_drop(true);
            crate::os::new_group(&mut cmd);
            match cmd.spawn() {
                Ok(ch) => {
                    let pid = ch.id();
                    log(&format!("monitor {n} started pid={pid:?}"));
                    let mut ms = st.monitors.lock().await;
                    let restarts = backoff.get(&n).map(|b| b.1).unwrap_or(0);
                    ms.insert(n.clone(), MonitorState { path: p.display().to_string(), pid, restarts, last_start: now_ms(), last_exit: None });
                    children.insert(n, ch);
                }
                Err(e) => log(&format!("monitor {n} failed to start: {e}")),
            }
        }
        // A monitor that has stayed up 10 minutes gets its backoff reset.
        for (n, m) in st.monitors.lock().await.iter() {
            if m.pid.is_some() && now_ms() - m.last_start > 600_000 {
                backoff.remove(n);
            }
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// Sample series that providers expose only as a current value (config `record:`).
pub(super) async fn record_loop() {
    let mut last: HashMap<String, i64> = HashMap::new();
    loop {
        for r in config::record_specs() {
            let every = crate::model::parse_duration(&r.every).unwrap_or(300) * 1000;
            if last.get(&r.id).map_or(false, |t| now_ms() - t < every) {
                continue;
            }
            last.insert(r.id.clone(), now_ms());
            let id = r.id.clone();
            tokio::spawn(async move {
                let Ok(iid) = ids::parse(&id) else { return };
                match providers::ticker(&iid).await {
                    Ok(t) => {
                        if let Ok(c) = store::open() {
                            let _ = store::record(&c, &iid.to_string(), &[MetricPoint { timestamp: now_ms(), value: t.last }]);
                        }
                    }
                    Err(e) => log(&format!("record {id}: {e}")),
                }
            });
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Collect OKX liquidation orders (the public feed only keeps the most recent ones) and store
/// per-minute USD notional as `okx:<BASE>/USDT:USDT#liquidations_long` / `_short`.
pub(super) async fn liquidation_loop() {
    let mut ct_val: HashMap<String, f64> = HashMap::new();
    let mut seen: HashMap<String, i64> = HashMap::new(); // family -> newest ts processed
    loop {
        for fam in config::liquidation_families() {
            let inst = format!("{fam}-SWAP");
            if !ct_val.contains_key(&fam) {
                if let Ok(v) = get_json::<Value>(&format!("https://www.okx.com/api/v5/public/instruments?instType=SWAP&instId={inst}")).await {
                    if let Some(c) = num(&v["data"][0]["ctVal"]) {
                        ct_val.insert(fam.clone(), c);
                    }
                }
            }
            let Some(cv) = ct_val.get(&fam).copied() else { continue };
            let url = format!("https://www.okx.com/api/v5/public/liquidation-orders?instType=SWAP&instFamily={fam}&state=filled&limit=100");
            let Ok(v) = get_json::<Value>(&url).await else { continue };
            let details = v["data"][0]["details"].as_array().cloned().unwrap_or_default();
            let last = seen.get(&fam).copied().unwrap_or(now_ms() - 120_000);
            let mut newest = last;
            let mut buckets: HashMap<(i64, String), f64> = HashMap::new();
            for d in details {
                let ts = num(&d["ts"]).unwrap_or(0.0) as i64;
                if ts <= last {
                    continue;
                }
                newest = newest.max(ts);
                let usd = num(&d["sz"]).unwrap_or(0.0) * cv * num(&d["bkPx"]).unwrap_or(0.0);
                let side = d["posSide"].as_str().unwrap_or("").to_string();
                *buckets.entry((ts - ts.rem_euclid(60_000), side)).or_default() += usd;
            }
            seen.insert(fam.clone(), newest);
            if buckets.is_empty() {
                continue;
            }
            let base = fam.split('-').next().unwrap_or(&fam).to_string();
            if let Ok(c) = store::open() {
                for ((minute, side), usd) in buckets {
                    let series = format!("okx:{base}/USDT:USDT#liquidations_{side}");
                    // Add to whatever this minute already has.
                    let prev = store::points(&c, &series, minute, minute).ok().and_then(|p| p.first().map(|x| x.value)).unwrap_or(0.0);
                    let _ = store::record(&c, &series, &[MetricPoint { timestamp: minute, value: prev + usd }]);
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(20)).await;
    }
}
