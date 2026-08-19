//! Leo bridge – malá lokální HTTP proxy pro OpenAI-kompatibilní klienty (Brave Leo, …),
//! kteří neumí vypnout „thinking" u modelů jako Gemma 4 / Qwen 3.
//!
//! Poslouchá na 127.0.0.1:{port}, všechno přeposílá 1:1 na Ollamu (`ollama.url`),
//! jen do JSON těla `/v1/chat/completions` doplní `reasoning_effort: "none"`
//! (Ollama ≥ 0.32 to na OpenAI endpointu respektuje). Streaming (SSE) prochází beze změny.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::Response,
    Router,
};
use futures_util::TryStreamExt;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
struct Ctx {
    upstream: String,
    http: reqwest::Client,
}

/// Spustí proxy v Tauri async runtime. Chyba bindu (port obsazený) se jen zaloguje.
pub fn start(upstream: String, port: u16) {
    tauri::async_runtime::spawn(async move {
        let ctx = Arc::new(Ctx { upstream: upstream.trim_end_matches('/').to_string(), http: reqwest::Client::new() });
        let app = Router::new().fallback(proxy).with_state(ctx);
        let addr = format!("127.0.0.1:{port}");
        match TcpListener::bind(&addr).await {
            Ok(l) => {
                log::info!("Leo bridge poslouchá na http://{addr} → {}", upstream);
                if let Err(e) = axum::serve(l, app).await {
                    log::error!("Leo bridge: {e}");
                }
            }
            Err(e) => log::warn!("Leo bridge: port {port} nejde otevřít ({e}) – běží už jiná instance?"),
        }
    });
}

async fn proxy(State(ctx): State<Arc<Ctx>>, req: Request) -> Result<Response, StatusCode> {
    let method = req.method().clone();
    let uri: Uri = req.uri().clone();
    let headers = req.headers().clone();
    let path_q = uri.path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
    let body_bytes = axum::body::to_bytes(req.into_body(), 64 * 1024 * 1024)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Do chat completions vložit reasoning_effort=none (pokud klient sám neposlal).
    let body = if method == Method::POST && path_q.starts_with("/v1/chat/completions") {
        match serde_json::from_slice::<serde_json::Value>(&body_bytes) {
            Ok(mut v) => {
                if let Some(o) = v.as_object_mut() {
                    o.entry("reasoning_effort").or_insert(serde_json::json!("none"));
                }
                serde_json::to_vec(&v).unwrap_or_else(|_| body_bytes.to_vec())
            }
            Err(_) => body_bytes.to_vec(),
        }
    } else {
        body_bytes.to_vec()
    };

    let url = format!("{}{}", ctx.upstream, path_q);
    let mut rb = ctx.http.request(method, &url).body(body);
    for (k, v) in forwardable(&headers) {
        rb = rb.header(k, v);
    }
    let resp = rb.send().await.map_err(|e| {
        log::warn!("Leo bridge → Ollama: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    let mut out = Response::builder().status(resp.status());
    for (k, v) in resp.headers() {
        // hop-by-hop hlavičky nepřeposílat
        if k == "transfer-encoding" || k == "connection" || k == "content-length" {
            continue;
        }
        out = out.header(k, v);
    }
    let stream = resp.bytes_stream().map_err(std::io::Error::other);
    out.body(Body::from_stream(stream)).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn forwardable(h: &HeaderMap) -> Vec<(axum::http::HeaderName, axum::http::HeaderValue)> {
    h.iter()
        .filter(|(k, _)| {
            let k = k.as_str();
            k != "host" && k != "content-length" && k != "connection" && k != "transfer-encoding"
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
