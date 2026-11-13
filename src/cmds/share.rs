//! `show ID --share`: publish an interactive chart through the daemon (and a cloudflared
//! tunnel); `show shares` lists the links; `daemon tunnel` installs cloudflared.

use super::*;
use crate::cli::ShowArgs;

/// Link lifetime in seconds; "0" means remove the link.
pub fn ttl_secs(ttl: &str) -> Result<i64> {
    if ttl == "0" {
        return Ok(0);
    }
    parse_duration(ttl)
}

/// Print the daemon's answer to a share request: the URL, or how many links were removed.
pub fn print_result(v: &serde_json::Value, g: &Global) -> Result<()> {
    if g.json {
        return print_json(v);
    }
    match v["revoked"].as_u64() {
        Some(0) => println!("no live link to remove"),
        Some(_) => println!("link removed"),
        None => println!("{}", v["url"].as_str().unwrap_or("")),
    }
    Ok(())
}

/// Publish `s` as an interactive chart; prints the public (or with --local, local) URL.
pub async fn share(s: &view::Series, a: &ShowArgs, g: &Global) -> Result<()> {
    let tf = Timeframe::parse(&s.interval)?;
    if !expr::is_expression(&s.id) {
        ids::parse(&s.id)?;
    }
    let limit = match (&s.range, s.limit) {
        (Some(r), _) => (parse_duration(r)? / tf.secs) as usize + 2,
        (None, n) => n.unwrap_or(200),
    };
    let v = daemon::api(
        "share",
        json!({"id": s.id, "kind": "candles", "tf": s.interval, "limit": limit, "range_secs": s.range.as_deref().map(parse_duration).transpose()?, "ttl_secs": ttl_secs(&a.ttl)?, "overlays": a.overlay, "name": a.name, "tunnel": if a.local { Some(false) } else { None }}),
    )
    .await?;
    print_result(&v, g)
}

/// `show shares`: every live link, newest first.
pub fn list(g: &Global) -> Result<()> {
    let rows = store::shares(&store::open()?, now_ms())?;
    let base = daemon::running().map(|i| i.public_url.unwrap_or_else(|| format!("http://127.0.0.1:{}", i.port)));
    let url = |s: &store::Share| base.as_ref().map(|b| format!("{}/{}/{}", b.trim_end_matches('/'), if s.kind == "doc" { "r" } else { "c" }, s.token)).unwrap_or_else(|| "-".into());
    if g.json {
        let v: Vec<_> = rows.iter().map(|s| json!({"token": s.token, "kind": s.kind, "title": s.title, "origin": s.origin, "created": s.created, "expires": s.expires, "views": s.views, "last_view": s.last_view, "url": url(s)})).collect();
        return print_json(&v);
    }
    if rows.is_empty() {
        println!("no live shared links (make one with --share)");
        return Ok(());
    }
    println!("{:<5}  {:<36}  {:>8}  {:>8}  {:>5}  url", "kind", "title", "created", "expires", "views");
    for s in &rows {
        let title: String = s.title.chars().take(36).collect();
        let kind = if s.origin == "hook" { "alert" } else { s.kind.as_str() };
        println!("{:<5}  {:<36}  {:>8}  {:>8}  {:>5}  {}", kind, title, ago(s.created).replace(" ago", ""), until(s.expires), s.views, url(s));
    }
    Ok(())
}

fn until(ms: i64) -> String {
    let d = (ms - now_ms()) / 1000;
    match d {
        d if d < 5400 => format!("{}m", d / 60),
        d if d < 172_800 => format!("{}h", d / 3600),
        d => format!("{}d", d / 86400),
    }
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
    let file = match std::env::consts::OS {
        "linux" => format!("cloudflared-linux-{arch}"),
        "windows" if arch == "amd64" => "cloudflared-windows-amd64.exe".into(),
        os => bail!("no cloudflared download known for {os} on {arch}; install it with your package manager (e.g. brew install cloudflared)"),
    };
    let url = format!("https://github.com/cloudflare/cloudflared/releases/latest/download/{file}");
    println!("downloading {url}");
    let bytes = crate::http::client().get(&url).timeout(std::time::Duration::from_secs(300)).send().await?.error_for_status()?.bytes().await?;
    let dir = config::home().join("bin");
    std::fs::create_dir_all(&dir)?;
    let p = dir.join(format!("cloudflared{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&p, &bytes)?;
    crate::os::make_executable(&p)?;
    println!("installed {} ({} MB)", p.display(), bytes.len() / 1_000_000);
    Ok(())
}
