use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::Empty;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

struct Config {
    redirect_host: String,
    redirect_path: Option<String>,
    redirect_code: u16,
    health_path: Option<String>,
}

fn resolve_code(raw: &str) -> Result<u16, String> {
    let allowed = [301u16, 302, 303, 307, 308];
    let code = raw
        .parse::<u16>()
        .map_err(|_| format!("'{}' is not a valid number", raw))?;
    if allowed.contains(&code) {
        Ok(code)
    } else {
        Err(format!(
            "'{}' is not a supported redirect code (allowed: 301, 302, 303, 307, 308)",
            code
        ))
    }
}

async fn handle<B>(
    req: Request<B>,
    config: Arc<Config>,
) -> Result<Response<Empty<Bytes>>, Infallible> {
    if let Some(ref health_path) = config.health_path {
        if req.uri().path() == health_path {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Length", "0")
                .body(Empty::new())
                .expect("health response must be valid");
            return Ok(response);
        }
    }

    let request_uri = req.uri().to_string();

    let path = match &config.redirect_path {
        Some(p) => p.clone(),
        None => req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string()),
    };

    let location = format!("https://{}{}", config.redirect_host, path);

    let status =
        StatusCode::from_u16(config.redirect_code).unwrap_or(StatusCode::MOVED_PERMANENTLY);

    let response = match Response::builder()
        .status(status)
        .header("Location", &location)
        .header("Content-Length", "0")
        .body(Empty::new())
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Failed to build response: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Length", "0")
                .body(Empty::new())
                .expect("fallback response must be valid")
        }
    };

    println!(
        "{} {} → {} [{}]",
        req.method(),
        request_uri,
        location,
        config.redirect_code
    );

    Ok(response)
}

#[tokio::main]
async fn main() {
    let redirect_host = std::env::var("SERVER_REDIRECT").unwrap_or_else(|_| {
        eprintln!("Error: SERVER_REDIRECT environment variable is required but not set.");
        std::process::exit(1);
    });

    // Validate that redirect_host produces a valid Location header value
    let test_location = format!("https://{}/", redirect_host);
    if hyper::header::HeaderValue::from_str(&test_location).is_err() {
        eprintln!(
            "Error: SERVER_REDIRECT contains invalid characters for a Location header: {}",
            redirect_host
        );
        std::process::exit(1);
    }

    let redirect_path = std::env::var("SERVER_REDIRECT_PATH").ok();

    // Validate that redirect_path starts with '/' and produces a valid header value
    if let Some(ref path) = redirect_path {
        if !path.starts_with('/') {
            eprintln!("Error: SERVER_REDIRECT_PATH must start with '/': {}", path);
            std::process::exit(1);
        }
        let test_location = format!("https://{}{}", redirect_host, path);
        if hyper::header::HeaderValue::from_str(&test_location).is_err() {
            eprintln!(
                "Error: SERVER_REDIRECT_PATH contains invalid characters for a Location header: {}",
                path
            );
            std::process::exit(1);
        }
    }

    let redirect_code = match std::env::var("SERVER_REDIRECT_CODE") {
        Ok(raw) => match resolve_code(&raw) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Error: invalid SERVER_REDIRECT_CODE: {}", e);
                std::process::exit(1);
            }
        },
        Err(_) => 301,
    };

    let health_path = std::env::var("SERVER_HEALTH_PATH").ok();

    let port: u16 = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "80".to_string())
        .parse()
        .unwrap_or_else(|_| {
            eprintln!("Error: SERVER_PORT must be a valid port number.");
            std::process::exit(1);
        });

    let config = Arc::new(Config {
        redirect_host: redirect_host.clone(),
        redirect_path,
        redirect_code,
        health_path,
    });

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = TcpListener::bind(addr).await.unwrap_or_else(|e| {
        eprintln!("Error: failed to bind to {}: {}", addr, e);
        std::process::exit(1);
    });

    println!(
        "Listening on 0.0.0.0:{} → https://{}",
        port, redirect_host
    );

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _) = match result {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                        continue;
                    }
                };

                let io = TokioIo::new(stream);
                let config = Arc::clone(&config);

                tokio::task::spawn(async move {
                    if let Err(e) = http1::Builder::new()
                        .serve_connection(io, service_fn(move |req| handle(req, Arc::clone(&config))))
                        .await
                    {
                        eprintln!("Connection error: {}", e);
                    }
                });
            }
            _ = &mut shutdown => {
                println!("Shutting down gracefully...");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Uri;

    #[test]
    fn resolve_code_valid_codes() {
        assert_eq!(resolve_code("301").unwrap(), 301);
        assert_eq!(resolve_code("302").unwrap(), 302);
        assert_eq!(resolve_code("303").unwrap(), 303);
        assert_eq!(resolve_code("307").unwrap(), 307);
        assert_eq!(resolve_code("308").unwrap(), 308);
    }

    #[test]
    fn resolve_code_invalid_returns_error() {
        assert!(resolve_code("200").is_err());
        assert!(resolve_code("404").is_err());
        assert!(resolve_code("500").is_err());
        assert!(resolve_code("abc").is_err());
        assert!(resolve_code("").is_err());
    }

    fn make_config(host: &str, path: Option<&str>, code: u16) -> Arc<Config> {
        Arc::new(Config {
            redirect_host: host.to_string(),
            redirect_path: path.map(|p| p.to_string()),
            redirect_code: code,
            health_path: None,
        })
    }

    fn make_request(uri: &str) -> Request<Empty<Bytes>> {
        Request::builder()
            .uri(uri.parse::<Uri>().unwrap())
            .body(Empty::new())
            .unwrap()
    }

    #[tokio::test]
    async fn handle_redirects_to_host_with_path() {
        let config = make_config("example.com", None, 301);
        let req = make_request("/foo/bar?q=1");
        let resp = handle(req, config).await.unwrap();

        assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(
            resp.headers().get("Location").unwrap(),
            "https://example.com/foo/bar?q=1"
        );
    }

    #[tokio::test]
    async fn handle_uses_configured_redirect_path() {
        let config = make_config("example.com", Some("/fixed"), 302);
        let req = make_request("/ignored");
        let resp = handle(req, config).await.unwrap();

        assert_eq!(resp.status(), StatusCode::FOUND);
        assert_eq!(
            resp.headers().get("Location").unwrap(),
            "https://example.com/fixed"
        );
    }

    #[tokio::test]
    async fn handle_defaults_to_root_when_no_path_and_query() {
        let config = make_config("example.com", None, 301);
        let req = Request::builder()
            .uri("/")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let resp = handle(req, config).await.unwrap();

        assert_eq!(
            resp.headers().get("Location").unwrap(),
            "https://example.com/"
        );
    }

    #[tokio::test]
    async fn handle_uses_path_and_query_not_full_uri() {
        let config = make_config("target.com", None, 307);
        let req = make_request("http://origin.com/path?key=val");
        let resp = handle(req, config).await.unwrap();

        assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            resp.headers().get("Location").unwrap(),
            "https://target.com/path?key=val"
        );
    }

    #[tokio::test]
    async fn handle_all_status_codes() {
        for (code, expected) in [
            (301, StatusCode::MOVED_PERMANENTLY),
            (302, StatusCode::FOUND),
            (303, StatusCode::SEE_OTHER),
            (307, StatusCode::TEMPORARY_REDIRECT),
            (308, StatusCode::PERMANENT_REDIRECT),
        ] {
            let config = make_config("example.com", None, code);
            let req = make_request("/");
            let resp = handle(req, config).await.unwrap();
            assert_eq!(resp.status(), expected);
        }
    }

    #[tokio::test]
    async fn handle_sets_content_length_zero() {
        let config = make_config("example.com", None, 301);
        let req = make_request("/");
        let resp = handle(req, config).await.unwrap();

        assert_eq!(resp.headers().get("Content-Length").unwrap(), "0");
    }

    #[tokio::test]
    async fn handle_health_check() {
        let config = Arc::new(Config {
            redirect_host: "example.com".to_string(),
            redirect_path: None,
            redirect_code: 301,
            health_path: Some("/healthz".to_string()),
        });
        let req = make_request("/healthz");
        let resp = handle(req, config).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().get("Location").is_none());
    }

    #[tokio::test]
    async fn handle_health_check_does_not_match_other_paths() {
        let config = Arc::new(Config {
            redirect_host: "example.com".to_string(),
            redirect_path: None,
            redirect_code: 301,
            health_path: Some("/healthz".to_string()),
        });
        let req = make_request("/other");
        let resp = handle(req, config).await.unwrap();

        assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
    }
}
