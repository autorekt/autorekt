//! `show ID --share`: publish an interactive chart through the daemon (and a cloudflared
//! tunnel); `daemon tunnel` installs cloudflared.

use super::*;
use std::os::unix::fs::PermissionsExt;

/// Publish `s` as an interactive chart; prints the public (or with `local`, local) URL.
pub async fn share(s: &view::Series, overlay: &[String], ttl: &str, local: bool, g: &Global) -> Result<()> {
    let tf = Timeframe::parse(&s.interval)?;
    if !expr::is_expression(&s.id) {
        ids::parse(&s.id)?;
    }
    let limit = match (&s.range, s.limit) {
        (Some(r), _) => (parse_duration(r)? / tf.secs) as usize + 2,
        (None, n) => n.unwrap_or(200),
    };
    let ttl = parse_duration(ttl)?;
    let v = daemon::api(
        "share",
        json!({"id": s.id, "kind": "candles", "tf": s.interval, "limit": limit, "ttl_secs": ttl, "overlays": overlay, "tunnel": if local { Some(false) } else { None }}),
    )
    .await?;
    if g.json {
        return print_json(&v);
    }
    println!("{}", v["url"].as_str().unwrap_or(""));
    Ok(())
}

pub async fn install_cloudflared() -> Result<()> {
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
