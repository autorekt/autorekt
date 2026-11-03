//! Shareable charts: share specs, the cloudflared quick tunnel, the chart page and its
//! data endpoint (the only routes that are public through the tunnel).

use super::*;
use tokio::io::AsyncBufReadExt;

const CHART_HTML: &str = include_str!("chart.html");
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSpec {
    pub kind: String, // candles | line
    pub id: String,   // instrument id or expression
    pub tf: String,
    pub limit: usize,
    pub expires: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub overlays: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct ShareReq {
    id: String,
    #[serde(default = "d_kind_c")]
    kind: String,
    #[serde(default = "d_tf")]
    tf: String,
    #[serde(default = "d_limit")]
    limit: usize,
    #[serde(default = "d_ttl")]
    ttl_secs: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    overlays: Vec<String>,
    #[serde(default)]
    tunnel: Option<bool>,
}
pub(super) fn d_kind_c() -> String {
    "candles".into()
}
pub(super) fn d_ttl() -> i64 {
    7 * 86400
}

pub(super) fn shares_path() -> PathBuf {
    config::home().join("shares.json")
}
pub(super) fn load_shares() -> HashMap<String, ShareSpec> {
    let mut m: HashMap<String, ShareSpec> = std::fs::read_to_string(shares_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    m.retain(|_, s| s.expires > now_ms());
    m
}

pub(super) async fn create_share_local(id: &str, kind: &str, tf: &str, limit: usize, ttl: i64) -> Result<String> {
    let v = api("share", json!({"id": id, "kind": kind, "tf": tf, "limit": limit, "ttl_secs": ttl})).await?;
    v["url"].as_str().map(|s| s.to_string()).ok_or_else(|| anyhow!("no url"))
}

pub(super) async fn api_share(State(st): State<St>, h: HeaderMap, Json(r): Json<ShareReq>) -> Response {
    let info = st.info.lock().await.clone();
    if !authed(&info, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    if Timeframe::parse(&r.tf).is_err() {
        return err(StatusCode::BAD_REQUEST, "bad tf");
    }
    let token = rand_token(12);
    let spec = ShareSpec { kind: r.kind, id: r.id, tf: r.tf, limit: r.limit.min(2000), expires: now_ms() + r.ttl_secs * 1000, title: r.title, overlays: r.overlays };
    {
        let mut sh = st.shares.lock().await;
        sh.retain(|_, s| s.expires > now_ms());
        sh.insert(token.clone(), spec);
        let _ = std::fs::write(shares_path(), serde_json::to_string(&*sh).unwrap_or_default());
    }
    let want_tunnel = r.tunnel.unwrap_or(!config::get().share_no_tunnel);
    let mut base = st.info.lock().await.public_url.clone();
    if base.is_none() && want_tunnel {
        match start_tunnel(&st, info.port).await {
            Ok(u) => base = Some(u),
            Err(e) => return err(StatusCode::SERVICE_UNAVAILABLE, format!("{e:#} (or share with --local for a link on this machine only)")),
        }
    }
    let local = format!("http://127.0.0.1:{}/c/{token}", info.port);
    let url = base.map(|b| format!("{}/c/{token}", b.trim_end_matches('/'))).unwrap_or(local.clone());
    Json(json!({"url": url, "local_url": local, "token": token})).into_response()
}

pub fn cloudflared_path() -> Option<PathBuf> {
    let local = config::home().join("bin").join("cloudflared");
    if local.exists() {
        return Some(local);
    }
    std::env::var("PATH").ok()?.split(':').map(|d| PathBuf::from(d).join("cloudflared")).find(|p| p.exists())
}

pub(super) async fn start_tunnel(st: &St, port: u16) -> Result<String> {
    let mut guard = st.tunnel.lock().await;
    if let Some(u) = st.info.lock().await.public_url.clone() {
        return Ok(u);
    }
    let bin = cloudflared_path().ok_or_else(|| anyhow!("cloudflared not installed; run `autorekt daemon tunnel`"))?;
    let mut child = tokio::process::Command::new(bin)
        .args(["tunnel", "--no-autoupdate", "--url", &format!("http://127.0.0.1:{port}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;
    let mut lines = tokio::io::BufReader::new(stderr).lines();
    let found = tokio::time::timeout(Duration::from_secs(30), async {
        while let Ok(Some(l)) = lines.next_line().await {
            if let Some(i) = l.find("https://") {
                let u: String = l[i..].split_whitespace().next().unwrap_or("").trim_end_matches('|').to_string();
                if u.contains("trycloudflare.com") {
                    return Some(u);
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
    .ok_or_else(|| anyhow!("cloudflared did not report a URL within 30s"))?;
    // Keep draining stderr so the process never blocks.
    tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
    *guard = Some(child);
    let mut info = st.info.lock().await;
    info.public_url = Some(found.clone());
    let _ = std::fs::write(info_path(), serde_json::to_string_pretty(&*info).unwrap_or_default());
    log(&format!("tunnel up: {found}"));
    // Quick tunnels take a few seconds to become routable.
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(found)
}

pub(super) async fn chart_page(State(st): State<St>, Path(token): Path<String>) -> Response {
    let sh = st.shares.lock().await;
    match sh.get(&token) {
        Some(s) if s.expires > now_ms() => {
            let title = s.title.clone().unwrap_or_else(|| format!("{} · {}", s.id, s.tf));
            let esc = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            Html(CHART_HTML.replace("{{TITLE}}", &esc)).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Html("<h3>This chart link has expired.</h3>")).into_response(),
    }
}

pub(super) async fn chart_data(State(st): State<St>, Path(token): Path<String>) -> Response {
    let spec = {
        let sh = st.shares.lock().await;
        match sh.get(&token) {
            Some(s) if s.expires > now_ms() => s.clone(),
            _ => return err(StatusCode::NOT_FOUND, "expired"),
        }
    };
    // 10s cache per share so many viewers do not hammer providers.
    if let Some((t, v)) = st.cache.lock().await.get(&token) {
        if now_ms() - t < 10_000 {
            return Json(v.clone()).into_response();
        }
    }
    let tf = Timeframe::parse(&spec.tf).unwrap_or(Timeframe::H1);
    let fetch = |id: String| async move {
        if crate::expr::is_expression(&id) {
            crate::expr::evaluate(&id, tf, Range::last(spec.limit)).await.map(|(g, v)| g.into_iter().zip(v).filter(|(_, x)| x.is_finite()).map(|(t, x)| Bar::point(t, x)).collect::<Vec<_>>())
        } else {
            let iid = ids::parse(&id)?;
            providers::ohlcv(&iid, tf, Range::last(spec.limit)).await
        }
    };
    let main = match fetch(spec.id.clone()).await {
        Ok(b) => b,
        Err(e) => return err(StatusCode::BAD_GATEWAY, format!("{e:#}")),
    };
    let mut overlays = Vec::new();
    let first = main.first().map(|b| b.timestamp).unwrap_or(0);
    for o in &spec.overlays {
        if let Ok(mut b) = fetch(o.clone()).await {
            // Keep the overlay inside the main series' window (plus one prior point so the
            // line starts at the left edge), so a sparse series does not stretch the axis.
            let prior = b.iter().rposition(|x| x.timestamp < first);
            let cut = prior.unwrap_or(0);
            b.drain(..cut);
            if let Some(p) = b.first_mut() {
                if p.timestamp < first {
                    p.timestamp = first;
                }
            }
            overlays.push(json!({"id": o, "bars": b}));
        }
    }
    // Rule firings and alerts within the window, as markers.
    let start = main.first().map(|b| b.timestamp).unwrap_or(0);
    let markers: Vec<Value> = store::open()
        .and_then(|c| store::events_since(&c, start, 200))
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, e, _)| e.kind != "news")
        .map(|(_, e, _)| json!({"t": e.timestamp, "text": e.title}))
        .collect();
    let kind = if spec.kind == "line" || main.iter().all(|b| b.open == b.close && b.high == b.low) { "line" } else { "candles" };
    let v = json!({"id": spec.id, "tf": spec.tf, "kind": kind, "bars": main, "overlays": overlays, "markers": markers, "updated": now_ms()});
    st.cache.lock().await.insert(token, (now_ms(), v.clone()));
    Json(v).into_response()
}
