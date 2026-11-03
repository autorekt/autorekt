//! `daemon`: lifecycle (run, start, stop, restart, status, install as a systemd user
//! service), logs of the daemon or a monitor, and installing cloudflared for shared charts.

use super::*;
use crate::cli::DaemonArgs;

pub async fn daemon(a: DaemonArgs, g: &Global) -> Result<()> {
    match a.action.as_str() {
        "logs" => {
            let file = match &a.name {
                Some(m) => format!("monitor-{m}.log"),
                None => "daemon.log".into(),
            };
            tail(&daemon::logs().join(file), a.lines)
        }
        "tunnel" => share::install_cloudflared().await,
        other => service(other, g).await,
    }
}

async fn service(cmd: &str, g: &Global) -> Result<()> {
    match cmd {
        "run" => daemon::run().await,
        "start" => {
            let i = daemon::ensure().await?;
            println!("daemon running: pid {} port {}", i.pid, i.port);
            Ok(())
        }
        "stop" => {
            println!("{}", if daemon::stop()? { "stopped" } else { "not running" });
            Ok(())
        }
        "restart" => {
            daemon::stop()?;
            let i = daemon::ensure().await?;
            println!("daemon running: pid {} port {}", i.pid, i.port);
            Ok(())
        }
        "status" => {
            let Some(_) = daemon::running() else {
                println!("not running");
                return Ok(());
            };
            let v = daemon::api("status", json!({})).await?;
            if g.json {
                return print_json(&v);
            }
            println!(
                "pid {}  port {}  up {}  public {}  shares {}",
                v["pid"],
                v["port"],
                ago(v["started"].as_i64().unwrap_or(0)).replace(" ago", ""),
                v["public_url"].as_str().unwrap_or("-"),
                v["shares"]
            );
            println!("rules: {}", v["rules"].as_array().map(|a| a.len()).unwrap_or(0));
            for m in v["monitors"].as_array().cloned().unwrap_or_default() {
                println!("monitor {}: pid {} restarts {}", m["name"].as_str().unwrap_or(""), m["state"]["pid"], m["state"]["restarts"]);
            }
            Ok(())
        }
        "install" => {
            let exe = std::env::current_exe()?;
            let dir = std::path::PathBuf::from(std::env::var("HOME")?).join(".config/systemd/user");
            std::fs::create_dir_all(&dir)?;
            let unit = format!(
                "[Unit]\nDescription=autorekt daemon\nAfter=network-online.target\n\n[Service]\nExecStart={} daemon run\nRestart=on-failure\nRestartSec=5\nEnvironment=AUTOREKT_HOME={}\n\n[Install]\nWantedBy=default.target\n",
                exe.display(),
                config::home().display()
            );
            let p = dir.join("autorekt.service");
            std::fs::write(&p, unit)?;
            println!("wrote {}", p.display());
            daemon::stop()?;
            let st = std::process::Command::new("systemctl").args(["--user", "daemon-reload"]).status();
            let st2 = std::process::Command::new("systemctl").args(["--user", "enable", "--now", "autorekt.service"]).status();
            match (st, st2) {
                (Ok(a), Ok(b)) if a.success() && b.success() => println!("enabled and started (systemctl --user status autorekt)"),
                _ => println!("could not run systemctl --user; enable it manually: systemctl --user enable --now autorekt"),
            }
            Ok(())
        }
        _ => bail!("unknown daemon action '{cmd}' (status, start, stop, restart, logs, install, run, tunnel)"),
    }
}

fn tail(path: &Path, n: usize) -> Result<()> {
    let t = std::fs::read_to_string(path).with_context(|| format!("no log at {}", path.display()))?;
    let lines: Vec<&str> = t.lines().collect();
    for l in &lines[lines.len().saturating_sub(n)..] {
        println!("{l}");
    }
    Ok(())
}
