//! The daemon's local HTTP API (bearer-token protected): status, ohlcv, eval, record,
//! event, shutdown. Monitors and CLI commands talk to the daemon through it.

use super::*;

pub(super) fn authed(st: &Info, h: &HeaderMap) -> bool {
    h.get("authorization").and_then(|v| v.to_str().ok()).map_or(false, |v| v == format!("Bearer {}", st.token))
}

pub(super) fn err(code: StatusCode, msg: impl ToString) -> Response {
    (code, Json(json!({"error": msg.to_string()}))).into_response()
}

pub(super) async fn api_shutdown(State(st): State<St>, h: HeaderMap) -> Response {
    if !authed(&*st.info.lock().await, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    st.shutdown.notify_one();
    Json(json!({"ok": true})).into_response()
}

#[derive(Deserialize)]
pub(super) struct OhlcvReq {
    id: String,
    #[serde(default = "d_tf")]
    tf: String,
    #[serde(default = "d_limit")]
    limit: usize,
    start: Option<i64>,
    end: Option<i64>,
}
pub(super) fn d_tf() -> String {
    "1h".into()
}
pub(super) fn d_limit() -> usize {
    200
}

pub(super) async fn api_status(State(st): State<St>, h: HeaderMap) -> Response {
    let info = st.info.lock().await.clone();
    if !authed(&info, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    let c = match store::open() {
        Ok(c) => c,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let rules: Vec<Value> = rules::load()
        .unwrap_or_default()
        .into_iter()
        .map(|(r, path)| {
            let s = store::get_rule_state(&c, &r.name).unwrap_or_default();
            json!({"rule": r, "file": path.display().to_string(), "state": s})
        })
        .collect();
    let monitors: Vec<Value> = st.monitors.lock().await.iter().map(|(n, m)| json!({"name": n, "state": m})).collect();
    let shares = st.shares.lock().await.len();
    Json(json!({"pid": info.pid, "port": info.port, "started": info.started, "public_url": info.public_url, "rules": rules, "monitors": monitors, "shares": shares})).into_response()
}

pub(super) async fn api_ohlcv(State(st): State<St>, h: HeaderMap, Json(r): Json<OhlcvReq>) -> Response {
    if !authed(&*st.info.lock().await, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    let tf = match Timeframe::parse(&r.tf) {
        Ok(t) => t,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };
    let range = Range { start: r.start, end: r.end, limit: r.limit.min(5000) };
    let res = if crate::expr::is_expression(&r.id) {
        crate::expr::evaluate(&r.id, tf, range).await.map(|(g, v)| g.into_iter().zip(v).map(|(t, x)| Bar::point(t, x)).collect::<Vec<_>>())
    } else {
        match ids::parse(&r.id) {
            Ok(id) => providers::ohlcv(&id, tf, range).await,
            Err(e) => Err(e),
        }
    };
    match res {
        Ok(b) => Json(json!(b)).into_response(),
        Err(e) => err(StatusCode::BAD_GATEWAY, format!("{e:#}")),
    }
}

#[derive(Deserialize)]
pub(super) struct EvalReq {
    expr: String,
    #[serde(default = "d_tf")]
    tf: String,
    #[serde(default = "d_limit")]
    limit: usize,
}

pub(super) async fn api_eval(State(st): State<St>, h: HeaderMap, Json(r): Json<EvalReq>) -> Response {
    if !authed(&*st.info.lock().await, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    let tf = match Timeframe::parse(&r.tf) {
        Ok(t) => t,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };
    match crate::expr::evaluate(&r.expr, tf, Range::last(r.limit)).await {
        Ok((g, v)) => Json(json!({"timestamp": g, "value": v.iter().map(|x| if x.is_finite() { json!(x) } else { Value::Null }).collect::<Vec<_>>()})).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, format!("{e:#}")),
    }
}

#[derive(Deserialize)]
pub(super) struct RecordReq {
    series: String,
    points: Vec<MetricPoint>,
}

pub(super) async fn api_record(State(st): State<St>, h: HeaderMap, Json(r): Json<RecordReq>) -> Response {
    if !authed(&*st.info.lock().await, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    match store::open().and_then(|c| store::record(&c, &r.series, &r.points)) {
        Ok(()) => Json(json!({"ok": true, "n": r.points.len()})).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[derive(Deserialize)]
pub(super) struct EventReq {
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default = "d_sev")]
    severity: u8,
    #[serde(default = "d_kind")]
    kind: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    symbols: Vec<String>,
    #[serde(default)]
    url: Option<String>,
    /// Optional hook command to run with the event in env vars.
    #[serde(default)]
    hook: Option<String>,
}
pub(super) fn d_sev() -> u8 {
    2
}
pub(super) fn d_kind() -> String {
    "alert".into()
}

pub(super) async fn api_event(State(st): State<St>, h: HeaderMap, Json(r): Json<EventReq>) -> Response {
    if !authed(&*st.info.lock().await, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    let e = Event { timestamp: now_ms(), kind: r.kind, source: r.source.unwrap_or_else(|| "monitor".into()), title: r.title, body: r.body, url: r.url, severity: r.severity.min(3), symbols: r.symbols };
    let id = match store::open().and_then(|c| store::add_event(&c, &e, None)) {
        Ok(id) => id,
        Err(err_) => return err(StatusCode::INTERNAL_SERVER_ERROR, err_),
    };
    log(&format!("event #{id} from {}: {}", e.source, e.title));
    if let Some(hk) = r.hook.or_else(|| config::get().default_hook.clone()) {
        let pseudo = rules::Rule { name: e.source.replace('/', "-"), ..Default::default() };
        let ev = e.clone();
        tokio::spawn(async move { run_hook(&hk, &pseudo, &ev, None, None).await });
    }
    Json(json!({"ok": true, "id": id})).into_response()
}
