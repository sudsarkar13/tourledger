//! Development only. Serves the app's `api` call over HTTP so the UI can be run
//! and tested in an ordinary browser. It has no authentication, listens on this
//! machine only and is not part of any shipped build.
//!
//!     tourledger-devbridge [data-dir] [port] [--public]

use std::io::Read;

use serde_json::{Value, json};
use tourledger_sync::{Network, Service};

const MAX_BODY: u64 = 25 * 1024 * 1024;

/// Refuses browsers that reached this server under another name (DNS rebinding)
/// and form posts from other sites, which cannot set a JSON content type.
fn allowed(request: &tiny_http::Request) -> bool {
    let header = |name: &str| {
        request
            .headers()
            .iter()
            .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str().to_owned())
    };
    let local_host =
        header("host").is_some_and(|h| h.starts_with("127.0.0.1:") || h.starts_with("localhost:"));
    let local_origin = header("origin")
        .is_none_or(|o| o.starts_with("http://127.0.0.1:") || o.starts_with("http://localhost:"));
    let json_body = header("content-type").is_some_and(|t| t.starts_with("application/json"));
    local_host && local_origin && json_body
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let public = args.iter().any(|a| a == "--public");
    args.retain(|a| a != "--public");
    let dir = args.first().map_or("dev-data", String::as_str);
    let port: u16 = args.get(1).map_or(Ok(4777), |p| p.parse())?;

    let runtime = tokio::runtime::Runtime::new()?;
    let network = if public {
        Network::Public
    } else {
        Network::Direct
    };
    let service = Service::open(dir.as_ref(), &format!("Dev {port}"), network)?;
    runtime.block_on(service.start());

    let server = tiny_http::Server::http(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    println!("devbridge on http://127.0.0.1:{port}, data in {dir}");

    for mut request in server.incoming_requests() {
        if request.url() != "/api"
            || request.method() != &tiny_http::Method::Post
            || !allowed(&request)
        {
            let _ = request.respond(tiny_http::Response::empty(403));
            continue;
        }
        let service = service.clone();
        runtime.spawn(async move {
            let mut body = String::new();
            let read = request.as_reader().take(MAX_BODY).read_to_string(&mut body);
            let parsed: Option<Value> = read.ok().and_then(|_| serde_json::from_str(&body).ok());
            let (status, reply) = match parsed {
                None => (400, json!({"message": "Unreadable request."})),
                Some(value) => match service.handle(value).await {
                    Ok(reply) => (200, reply),
                    Err(error) => (
                        422,
                        json!({"message": error.to_string(), "field": error.field()}),
                    ),
                },
            };
            let header = tiny_http::Header::from_bytes("Content-Type", "application/json");
            let mut response =
                tiny_http::Response::from_string(reply.to_string()).with_status_code(status);
            if let Ok(header) = header {
                response = response.with_header(header);
            }
            let _ = request.respond(response);
        });
    }
    Ok(())
}
