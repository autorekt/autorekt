//! Shared reports: markdown with ```chart blocks, rendered server-side (raw HTML escaped) into
//! the same page template as single charts. Each chart line becomes a chart share; a block
//! with several lines becomes one component with tabs.

use super::*;
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event as Md, HeadingLevel, Options, Parser, Tag, TagEnd};

pub(super) const PAGE_HTML: &str = include_str!("page.html");

#[derive(Deserialize, Serialize, Clone)]
pub struct ChartReq {
    pub id: String,
    pub tf: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub range_secs: Option<i64>,
    #[serde(default)]
    pub end: Option<i64>,
    #[serde(default)]
    pub overlays: Vec<String>,
    pub label: String,
}

#[derive(Deserialize)]
pub(super) struct ReportReq {
    markdown: String,
    blocks: Vec<Vec<ChartReq>>,
    #[serde(default = "d_ttl")]
    ttl_secs: i64,
    #[serde(default)]
    tunnel: Option<bool>,
}

pub struct Report {
    pub title: String,
    pub html: String,
    pub expires: i64,
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// The contents of the ```chart blocks of a markdown document, in order.
pub fn chart_blocks(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for ev in Parser::new_ext(md, options()) {
        match ev {
            Md::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) if lang.trim() == "chart" => cur = Some(String::new()),
            Md::Text(t) if cur.is_some() => cur.as_mut().unwrap().push_str(&t),
            Md::End(TagEnd::CodeBlock) if cur.is_some() => out.push(cur.take().unwrap()),
            _ => {}
        }
    }
    out
}

pub(super) fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

pub(super) fn page(title: &str, class: &str, body: &str) -> String {
    PAGE_HTML.replace("{{TITLE}}", &escape(title)).replace("{{CLASS}}", class).replace("{{BODY}}", body)
}

/// A chart component: one or more (token, label) charts behind tabs.
pub(super) fn block_html(charts: &[(String, String)]) -> String {
    let data: Vec<Value> = charts.iter().map(|(t, l)| json!({"token": t, "label": l})).collect();
    format!("<div class=\"chart-block\" data-charts=\"{}\"></div>\n", escape(&Value::Array(data).to_string()))
}

/// Render markdown to HTML, replacing the n-th ```chart block with `blocks[n]` and escaping
/// raw HTML. Returns (title from the first H1, html).
fn render(md: &str, blocks: &[String]) -> (String, String) {
    let mut events: Vec<Md> = Vec::new();
    let mut in_chart = false;
    let mut n = 0;
    let mut title = String::new();
    let mut in_h1 = false;
    for ev in Parser::new_ext(md, options()) {
        match ev {
            Md::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref lang))) if lang.trim() == "chart" => {
                in_chart = true;
                events.push(Md::Html(CowStr::from(blocks.get(n).cloned().unwrap_or_default())));
                n += 1;
            }
            Md::End(TagEnd::CodeBlock) if in_chart => in_chart = false,
            _ if in_chart => {}
            Md::Html(s) | Md::InlineHtml(s) => events.push(Md::Text(s)),
            Md::Start(Tag::Heading { level: HeadingLevel::H1, .. }) if title.is_empty() => {
                in_h1 = true;
                events.push(ev);
            }
            Md::End(TagEnd::Heading(HeadingLevel::H1)) if in_h1 => {
                in_h1 = false;
                events.push(ev);
            }
            Md::Text(ref t) if in_h1 => {
                title.push_str(t);
                events.push(ev);
            }
            other => events.push(other),
        }
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    (title, out)
}

pub(super) async fn api_report(State(st): State<St>, h: HeaderMap, Json(r): Json<ReportReq>) -> Response {
    let info = st.info.lock().await.clone();
    if !authed(&info, &h) {
        return err(StatusCode::UNAUTHORIZED, "bad token");
    }
    let found = chart_blocks(&r.markdown).len();
    if found != r.blocks.len() {
        return err(StatusCode::BAD_REQUEST, format!("{found} chart blocks in the markdown but {} sent", r.blocks.len()));
    }
    let expires = now_ms() + r.ttl_secs * 1000;
    let mut blocks = Vec::new();
    for b in &r.blocks {
        let mut charts = Vec::new();
        for c in b {
            let spec = ShareSpec { kind: "candles".into(), id: c.id.clone(), tf: c.tf.clone(), limit: c.limit.unwrap_or(200).min(2000), expires, title: Some(c.label.clone()), overlays: c.overlays.clone(), range_secs: c.range_secs, end: c.end };
            charts.push((insert_share(&st, spec).await, c.label.clone()));
        }
        blocks.push(block_html(&charts));
    }
    let (title, html) = render(&r.markdown, &blocks);
    let token = rand_token(12);
    st.reports.lock().await.insert(token.clone(), Report { title: if title.is_empty() { "report".into() } else { title }, html, expires });
    match public_url(&st, info.port, r.tunnel, &format!("r/{token}")).await {
        Ok(v) => Json(v).into_response(),
        Err(resp) => resp,
    }
}

pub(super) async fn report_page(State(st): State<St>, Path(token): Path<String>) -> Response {
    let reports = st.reports.lock().await;
    match reports.get(&token) {
        Some(r) if r.expires > now_ms() => Html(page(&r.title, "report", &format!("<article>\n{}</article>", r.html))).into_response(),
        _ => (StatusCode::NOT_FOUND, Html("<h3>This report link has expired.</h3>")).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charts_and_html_are_handled() {
        let md = "# Title <b>x</b>\n\ntext <script>alert(1)</script>\n\n```chart\nBTC -i 1h\nETH\n```\n\n```rust\nlet a = 1;\n```\n";
        assert_eq!(chart_blocks(md), vec!["BTC -i 1h\nETH\n".to_string()]);
        let (title, html) = render(md, &["<div class=\"chart-block\"></div>".to_string()]);
        assert_eq!(title, "Title x");
        assert!(html.contains("<div class=\"chart-block\"></div>"));
        assert!(!html.contains("<script>"), "raw HTML must be escaped: {html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("let a = 1;"), "other code blocks stay");
    }
}
