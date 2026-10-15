//! Process entry point: run the health check when asked, and
//! serve otherwise.

use praeludium::db::{App, open_db};
use praeludium::handlers::router;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Config {
    data_dir: String,
    port: String,
}

fn load_config() -> Config {
    let data_dir = std::env::var("PRAELUDIUM_DATA_DIR").unwrap_or_default();
    let port = std::env::var("PORT").unwrap_or_default();
    Config {
        data_dir: if data_dir.is_empty() {
            "/data".to_string()
        } else {
            data_dir
        },
        port: if port.is_empty() {
            "8080".to_string()
        } else {
            port
        },
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|a| a == "-healthcheck" || a == "--healthcheck")
    {
        let port_raw = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let port: u16 = match port_raw.parse() {
            Ok(p) if p >= 1 => p,
            _ => std::process::exit(1),
        };
        std::process::exit(run_healthcheck(port));
    }
    let cfg = load_config();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    match runtime.block_on(run(cfg)) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

/// Open the database and serve HTTP until it fails.
async fn run(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&cfg.data_dir).map_err(|e| format!("data dir: {e}"))?;
    let db = open_db(
        std::path::Path::new(&cfg.data_dir)
            .join("praeludium.db")
            .as_path(),
    )
    .map_err(|e| format!("open db: {e}"))?;
    let app = Arc::new(App {
        db: std::sync::Mutex::new(db),
        data_dir: std::path::PathBuf::from(&cfg.data_dir),
    });
    // Authentication is enforced by the reverse proxy in front of
    // praeludium: the app itself serves everything, including
    // /healthz, without auth.
    let healthz = axum::Router::new().route(
        "/healthz",
        axum::routing::any(|method: axum::http::Method| async move {
            if method == axum::http::Method::GET {
                axum::response::Response::new(axum::body::Body::from("ok\n"))
            } else {
                praeludium::handlers::not_found()
            }
        }),
    );
    let routes = healthz
        .merge(router(app))
        .layer(axum::middleware::from_fn(go_method_not_allowed))
        .layer(axum::middleware::from_fn(log_requests));
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", cfg.port)).await?;
    eprintln!(
        "praeludium in ascolto su :{} (dati in {})",
        cfg.port, cfg.data_dir
    );
    axum::serve(listener, routes).await?;
    Ok(())
}

/// Give 405 responses the body Go's method-aware mux writes
/// ("Method Not Allowed" as plain text), where the framework's
/// own 405 is empty.
async fn go_method_not_allowed(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let resp = next.run(req).await;
    if resp.status() != axum::http::StatusCode::METHOD_NOT_ALLOWED {
        return resp;
    }
    let (mut parts, _body) = resp.into_parts();
    parts.headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    parts.headers.insert(
        axum::http::HeaderName::from_static("x-content-type-options"),
        axum::http::HeaderValue::from_static("nosniff"),
    );
    (parts, "Method Not Allowed\n").into_response()
}

/// Write one line per request (method, path, status, duration) to
/// stderr. The health endpoint is skipped so container health
/// probes do not drown the journal.
async fn log_requests(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path().to_string();
    if path == "/healthz" {
        return next.run(req).await;
    }
    let method = req.method().to_string();
    let start = Instant::now();
    let resp = next.run(req).await;
    eprintln!(
        "{} {} -> {} ({}ms)",
        go_quote(&method),
        go_quote(&path),
        resp.status().as_u16(),
        start.elapsed().as_millis()
    );
    resp
}

/// Quote a string the way Go's %q verb does for the request values
/// that reach the log: the quoted method and path cannot inject
/// log lines, because every control character is escaped.
pub fn go_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Probe the local /healthz endpoint for container HEALTHCHECK.
/// The host is a loopback literal and the port is a validated
/// integer: no part of the URL comes from request data.
fn run_healthcheck(port: u16) -> i32 {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("healthcheck: {err}");
            return 1;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    if let Err(err) = stream.write_all(b"GET /healthz HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n") {
        eprintln!("healthcheck: {err}");
        return 1;
    }
    let mut buf = String::new();
    if let Err(err) = stream.read_to_string(&mut buf) {
        eprintln!("healthcheck: {err}");
        return 1;
    }
    if buf.starts_with("HTTP/1.1 200") || buf.starts_with("HTTP/1.0 200") {
        0
    } else {
        let status = buf.lines().next().unwrap_or("");
        eprintln!("healthcheck: unexpected status {status}");
        1
    }
}
