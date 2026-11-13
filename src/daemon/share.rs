//! Shareable charts: share specs (kept in the store, so links survive restarts), the cloudflared
//! quick tunnel, the single-chart page and the chart data endpoint (with reports, the only
//! routes that are public through the tunnel).

use super::*;
use tokio::io::AsyncBufReadExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSpec {
    pub kind: String, // candles | line
    pub id: String,   // instrument id or expression
    pub tf: String,
    pub limit: usize,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub overlays: Vec<String>,
    /// Show this many seconds back from `end` (else the last `limit` bars).
    #[serde(default)]
    pub range_secs: Option<i64>,
    /// Fixed end of the view in Unix ms; none = live.
    #[serde(default)]
    pub end: Option<i64>,
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
    #[serde(default)]
    range_secs: Option<i64>,
    #[serde(default)]
    end: Option<i64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default = "d_origin")]
    origin: String,
}
pub(super) fn d_origin() -> String {
    "manual".into()
}
pub(super) fn d_kind_c() -> String {
    "candles".into()
}
pub(super) fn d_ttl() -> i64 {
    7 * 86400
}

pub(super) async fn create_share_local(id: &str, kind: &str, tf: &str, limit: usize, ttl: i64) -> Result<String> {
    let v = api("share", json!({"id": id, "kind": kind, "tf": tf, "limit": limit, "ttl_secs": ttl, "origin": "hook"})).await?;
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
    let spec = ShareSpec { kind: r.kind, id: r.id, tf: r.tf, limit: r.limit.min(2000), title: r.title, overlays: r.overlays, range_secs: r.range_secs, end: r.end };
    let key = format!("chart:{}|{}", serde_json::to_string(&spec).unwrap_or_default(), r.name.unwrap_or_default());
    if r.ttl_secs == 0 {
        return match revoke(&key) {
            Ok(n) => Json(json!({"revoked": n})).into_response(),
            Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
        };
    }
    let title = spec.title.clone().unwrap_or_else(|| format!("{} · {}", spec.id, spec.tf));
    let token = match put_share(&key, "chart", &title, &spec, None, &r.origin, r.ttl_secs) {
        Ok(t) => t,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
    };
    match public_url(&st, info.port, r.tunnel, &format!("c/{token}")).await {
        Ok(v) => Json(v).into_response(),
        Err(resp) => resp,
    }
}

/// Store a share under `key` and return its token: the existing link for that key if it is
/// still live (its content and expiry are updated), else a new one.
pub(super) fn put_share<T: Serialize>(key: &str, kind: &str, title: &str, spec: &T, parent: Option<&str>, origin: &str, ttl_secs: i64) -> Result<String> {
    let c = store::open()?;
    let now = now_ms();
    let token = store::share_by_key(&c, key, now)?.map(|s| s.token).unwrap_or_else(|| rand_token(12));
    let s = store::Share {
        token: token.clone(),
        key: key.into(),
        kind: kind.into(),
        title: title.into(),
        spec: serde_json::to_string(spec)?,
        parent: parent.map(|p| p.to_string()),
        origin: origin.into(),
        created: now,
        expires: now + ttl_secs * 1000,
        views: 0,
        last_view: None,
    };
    store::share_put(&c, &s)?;
    Ok(token)
}

/// Remove the live share under `key`; returns how many links went (with charts inside).
pub(super) fn revoke(key: &str) -> Result<usize> {
    let c = store::open()?;
    match store::share_by_key(&c, key, now_ms())? {
        Some(s) => store::share_remove(&c, &s.token),
        None => Ok(0),
    }
}

/// The live share behind `token`, counting a view when `view` is set.
pub(super) fn get_share(token: &str, view: bool) -> Option<store::Share> {
    let c = store::open().ok()?;
    let s = store::share_get(&c, token, now_ms()).ok()??;
    if view {
        let _ = store::share_viewed(&c, token, now_ms());
    }
    Some(s)
}

/// The public URL of `path`, starting the tunnel when wanted; JSON {url, local_url}.
pub(super) async fn public_url(st: &St, port: u16, tunnel: Option<bool>, path: &str) -> std::result::Result<Value, Response> {
    let want_tunnel = tunnel.unwrap_or(!config::get().share_no_tunnel);
    let mut base = st.info.lock().await.public_url.clone();
    if base.is_none() && want_tunnel {
        match start_tunnel(st, port).await {
            Ok(u) => base = Some(u),
            Err(e) => return Err(err(StatusCode::SERVICE_UNAVAILABLE, format!("{e:#} (or share with --local for a link on this machine only)"))),
        }
    }
    let local = format!("http://127.0.0.1:{port}/{path}");
    let url = base.map(|b| format!("{}/{path}", b.trim_end_matches('/'))).unwrap_or(local.clone());
    Ok(json!({"url": url, "local_url": local}))
}

pub fn cloudflared_path() -> Option<PathBuf> {
    let name = format!("cloudflared{}", std::env::consts::EXE_SUFFIX);
    let local = config::home().join("bin").join(&name);
    if local.exists() {
        return Some(local);
    }
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(&name)).find(|p| p.exists())
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

pub(super) async fn chart_page(Path(token): Path<String>) -> Response {
    match get_share(&token, true).filter(|s| s.kind == "chart") {
        Some(s) => Html(page(&s.title, "single", &block_html(&[(token.clone(), s.title.clone())]))).into_response(),
        None => (StatusCode::NOT_FOUND, Html("<h3>This chart link has expired or was removed.</h3>")).into_response(),
    }
}

pub(super) async fn chart_data(State(st): State<St>, Path(token): Path<String>) -> Response {
    let Some(spec) = get_share(&token, false).filter(|s| s.kind == "chart").and_then(|s| serde_json::from_str::<ShareSpec>(&s.spec).ok()) else {
        return err(StatusCode::NOT_FOUND, "expired");
    };
    // 10s cache per share so many viewers do not hammer providers.
    if let Some((t, v)) = st.cache.lock().await.get(&token) {
        if now_ms() - t < 10_000 {
            return Json(v.clone()).into_response();
        }
    }
    let tf = Timeframe::parse(&spec.tf).unwrap_or(Timeframe::H1);
    let range = match spec.range_secs {
        Some(s) => Range { start: Some(spec.end.unwrap_or_else(now_ms) - s * 1000), end: spec.end, limit: 5000 },
        None => Range { start: None, end: spec.end, limit: spec.limit },
    };
    let fetch = |id: String| async move {
        if crate::expr::is_expression(&id) {
            crate::expr::evaluate(&id, tf, range).await.map(|(g, v)| g.into_iter().zip(v).filter(|(_, x)| x.is_finite()).map(|(t, x)| Bar::point(t, x)).collect::<Vec<_>>())
        } else {
            let iid = ids::parse(&id)?;
            providers::ohlcv(&iid, tf, range).await
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
        // Only alerts about this chart: a rule whose expression names it, or tagged symbols.
        .filter(|(_, e, _)| matches!(e.kind.as_str(), "rule" | "alert" | "resolved") && (e.body.contains(spec.id.as_str()) || e.symbols.iter().any(|s| *s == spec.id)))
        .map(|(_, e, _)| json!({"t": e.timestamp, "text": e.title}))
        .collect();
    let kind = if spec.kind == "line" || main.iter().all(|b| b.open == b.close && b.high == b.low) { "line" } else { "candles" };
    let v = json!({"id": spec.id, "tf": spec.tf, "kind": kind, "bars": main, "overlays": overlays, "markers": markers, "updated": now_ms()});
    st.cache.lock().await.insert(token, (now_ms(), v.clone()));
    Json(v).into_response()
}
