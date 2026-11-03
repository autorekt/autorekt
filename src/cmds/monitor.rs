//! Python monitors: create from the template, install, list with process state, remove,
//! and print the SDK.

use super::*;

fn monitors_dir() -> std::path::PathBuf {
    config::home().join("monitors")
}

pub async fn monitor_ls(g: &Global) -> Result<()> {
    let dir = monitors_dir();
    let files: Vec<String> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
    let status = if daemon::running().is_some() { daemon::api("status", json!({})).await.ok() } else { None };
    if g.json {
        return print_json(&json!({"files": files, "status": status.map(|s| s["monitors"].clone())}));
    }
    if files.is_empty() {
        println!("no monitors in {} (create one: autorekt monitor new NAME)", dir.display());
        return Ok(());
    }
    for f in files {
        let name = f.rsplit_once('.').map(|x| x.0).unwrap_or(&f).to_string();
        let st = status.as_ref().and_then(|s| s["monitors"].as_array().cloned()).unwrap_or_default().into_iter().find(|m| m["name"].as_str() == Some(&name));
        match st {
            Some(m) => println!("{f:<28} pid {:<8} restarts {}  {}", m["state"]["pid"], m["state"]["restarts"], m["state"]["last_exit"].as_str().map(|e| format!("last exit: {e}")).unwrap_or_default()),
            None => println!("{f:<28} {}", if status.is_some() { "not running" } else { "(daemon stopped)" }),
        }
    }
    Ok(())
}

pub fn monitor_new(name: &str) -> Result<()> {
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("monitor name: letters, digits, _");
    }
    std::fs::create_dir_all(monitors_dir())?;
    let p = monitors_dir().join(format!("{name}.py"));
    if p.exists() {
        bail!("{} exists", p.display());
    }
    let cls: String = name.split('_').map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect();
    std::fs::write(
        &p,
        format!(
            r#""""{name}: describe what this monitor watches."""
from autorekt import Monitor


class {cls}(Monitor):
    def on_start(self):
        # Closed 1m bars of the BTC perp; any autorekt id works.
        self.subscribe_bars("BTC", "1m")
        self.schedule(every="1h", fn=self.heartbeat)

    def on_bar(self, bar):
        closes = [b["close"] for b in self.history(bar.id, "1m", 60)]
        move = closes[-1] / closes[0] - 1
        self.record("{name}.move_60m", move)        # readable as local:{name}.move_60m
        if abs(move) > 0.02:
            self.alert(f"{{bar.id}} moved {{move:+.2%}} in 60m", severity=2)

    def heartbeat(self):
        self.log("alive")


if __name__ == "__main__":
    {cls}().run()
"#
        ),
    )?;
    println!("created {} — edit it; the daemon (re)starts monitors automatically. Logs: autorekt monitor logs {name}", p.display());
    Ok(())
}

pub fn monitor_add(file: &str) -> Result<()> {
    let src = Path::new(file);
    let name = src.file_name().ok_or_else(|| anyhow::anyhow!("bad path"))?;
    std::fs::create_dir_all(monitors_dir())?;
    let dst = monitors_dir().join(name);
    std::fs::copy(src, &dst)?;
    println!("installed {}", dst.display());
    Ok(())
}

pub fn monitor_rm(name: &str) -> Result<()> {
    for ext in ["py", ""] {
        let p = if ext.is_empty() { monitors_dir().join(name) } else { monitors_dir().join(format!("{name}.{ext}")) };
        if p.exists() {
            std::fs::remove_file(&p)?;
            println!("removed {}", p.display());
            return Ok(());
        }
    }
    bail!("no monitor '{name}'")
}

pub fn monitor_sdk() -> Result<()> {
    let p = config::home().join("sdk").join("autorekt.py");
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, daemon::SDK_PY)?;
    println!("SDK: {}  (the daemon puts it on PYTHONPATH for monitors)\n", p.display());
    let doc = daemon::SDK_PY.split("\"\"\"").nth(1).unwrap_or("");
    println!("{}", doc.trim());
    Ok(())
}
