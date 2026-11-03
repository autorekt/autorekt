//! `share`: publish an interactive chart through the daemon (and a cloudflared tunnel),
//! and install cloudflared on request.

use super::*;
use std::os::unix::fs::PermissionsExt;

pub async fn share(id: Option<String>, interval: &str, limit: usize, kind: &str, overlay: Vec<String>, title: Option<String>, ttl: &str, local: bool, install: bool, g: &Global) -> Result<()> {
    if install {
        return install_cloudflared().await;
    }
    let id = id.ok_or_else(|| anyhow::anyhow!("give an id or expression to chart"))?;
    Timeframe::parse(interval)?;
    if !expr::is_expression(&id) {
        ids::parse(&id)?;
    }
    let ttl = parse_duration(ttl)?;
    let v = daemon::api(
        "share",
        json!({"id": id, "kind": kind, "tf": interval, "limit": limit, "ttl_secs": ttl, "title": title, "overlays": overlay, "tunnel": if local { Some(false) } else { None }}),
    )
    .await?;
    if g.json {
        return print_json(&v);
    }
    println!("{}", v["url"].as_str().unwrap_or(""));
    Ok(())
}

async fn install_cloudflared() -> Result<()> {
    if let Some(p) = daemon::cloudflared_path() {
        println!("cloudflared already available at {}", p.display());
        return Ok(());
    }
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        a => bail!("no cloudflared build known for {a}; install it yourself"),
    };
    let os = std::env::consts::OS;
    if os != "linux" {
        bail!("on {os}, install cloudflared with your package manager (e.g. brew install cloudflared)");
    }
    let url = format!("https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-{arch}");
    println!("downloading {url}");
    let bytes = crate::http::client().get(&url).timeout(std::time::Duration::from_secs(300)).send().await?.error_for_status()?.bytes().await?;
    let dir = config::home().join("bin");
    std::fs::create_dir_all(&dir)?;
    let p = dir.join("cloudflared");
    std::fs::write(&p, &bytes)?;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))?;
    println!("installed {} ({} MB)", p.display(), bytes.len() / 1_000_000);
    Ok(())
}
