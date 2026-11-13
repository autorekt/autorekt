//! The tunnel that makes shared links public: a cloudflared process that runs on its own, so it
//! keeps its address while the daemon restarts. tunnel.json records it. A quick tunnel gets a
//! random trycloudflare.com address; a named tunnel (share_tunnel_token_file) serves the fixed
//! address in share_base_url.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tunnel {
    pub kind: String,
    pub pid: u32,
    pub url: String,
    pub port: u16,
    pub started: i64,
}

fn path() -> PathBuf {
    config::home().join("tunnel.json")
}

/// The recorded tunnel, if its process is still running.
pub fn read() -> Option<Tunnel> {
    let t: Tunnel = serde_json::from_str(&std::fs::read_to_string(path()).ok()?).ok()?;
    crate::os::alive(t.pid).then_some(t)
}

/// The port for the daemon's web server: share_port, else the one a running tunnel points at.
pub fn port() -> u16 {
    config::get().share_port.or_else(|| read().map(|t| t.port)).unwrap_or(0)
}

/// Whether the daemon runs a named tunnel (share_tunnel_token_file is set).
pub fn named() -> bool {
    config::get().share_tunnel_token_file.is_some()
}

/// End the tunnel process and forget it. Returns whether one was running.
pub fn stop() -> bool {
    let t = read();
    if let Some(t) = &t {
        crate::os::end_group(t.pid);
    }
    let _ = std::fs::remove_file(path());
    t.is_some()
}

/// The address of a running tunnel of the wanted kind that reaches the daemon on `port`.
pub async fn adopt(port: u16) -> Option<String> {
    let kind = if named() { "named" } else { "quick" };
    let t = read().filter(|t| t.port == port && t.kind == kind)?;
    let r = crate::http::client().get(format!("{}/health", t.url.trim_end_matches('/'))).timeout(Duration::from_secs(15)).send().await.ok()?;
    r.status().is_success().then_some(t.url)
}

pub fn cloudflared_path() -> Option<PathBuf> {
    let name = format!("cloudflared{}", std::env::consts::EXE_SUFFIX);
    let local = config::home().join("bin").join(&name);
    if local.exists() {
        return Some(local);
    }
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(&name)).find(|p| p.exists())
}

/// Start cloudflared for the daemon on `port` and return the public base URL.
pub async fn start(port: u16) -> Result<String> {
    stop();
    let cfg = config::get();
    let bin = cloudflared_path().ok_or_else(|| anyhow!("cloudflared not installed; run `autorekt daemon tunnel`"))?;
    let log_path = logs().join("tunnel.log");
    let log_file = std::fs::File::create(&log_path)?;
    let mut cmd = std::process::Command::new(bin);
    let (kind, fixed) = match &cfg.share_tunnel_token_file {
        Some(f) => {
            let file = match f.strip_prefix("~/") {
                Some(rest) => std::env::home_dir().context("no home directory")?.join(rest),
                None => PathBuf::from(f),
            };
            let token = std::fs::read_to_string(&file).with_context(|| format!("reading share_tunnel_token_file {}", file.display()))?.trim().to_string();
            let url = cfg.share_base_url.clone().ok_or_else(|| anyhow!("share_tunnel_token_file needs share_base_url: the address the named tunnel serves"))?;
            if cfg.share_port.is_none() {
                bail!("share_tunnel_token_file needs share_port: the local port the named tunnel points at");
            }
            cmd.args(["tunnel", "--no-autoupdate", "run"]).env("TUNNEL_TOKEN", token);
            ("named", Some(url))
        }
        None => {
            cmd.args(["tunnel", "--no-autoupdate", "--url", &format!("http://127.0.0.1:{port}")]);
            ("quick", None)
        }
    };
    cmd.stdin(std::process::Stdio::null()).stdout(log_file.try_clone()?).stderr(log_file);
    crate::os::detach(&mut cmd);
    let mut child = cmd.spawn().context("starting cloudflared")?;
    let pid = child.id();
    // Reap it if it exits while this daemon runs, so a dead tunnel is not taken for a live one.
    std::thread::spawn(move || child.wait());
    let url = match fixed {
        Some(u) => u,
        None => quick_url(&log_path).await?,
    };
    let t = Tunnel { kind: kind.into(), pid, url: url.clone(), port, started: now_ms() };
    std::fs::write(path(), serde_json::to_string_pretty(&t)?)?;
    log(&format!("{kind} tunnel up: {url} (pid {pid})"));
    // A new tunnel takes a few seconds to become routable.
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(url)
}

/// The trycloudflare.com address a quick tunnel prints to its log.
async fn quick_url(log_path: &std::path::Path) -> Result<String> {
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let text = std::fs::read_to_string(log_path).unwrap_or_default();
        let found = text.split_whitespace().map(|w| w.trim_end_matches('|')).find(|w| w.starts_with("https://") && w.contains("trycloudflare.com"));
        if let Some(u) = found {
            return Ok(u.to_string());
        }
    }
    bail!("cloudflared did not report an address within 30s; see {}", log_path.display())
}
