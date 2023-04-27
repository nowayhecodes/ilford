mod parse;

use parse::{load_config, GatewayConfig, ServiceConfig};

use hyper::header::HeaderValue;
use hyper::http::request::Parts;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};

use reqwest::header::{HeaderMap, AUTHORIZATION};

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let config = load_config("ilford.config.yml");
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    let make_svc = make_service_fn(move |conn| {
        let config = config.clone();

        async {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let config = config.clone();
                handle_request(req, config);
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprint!("server error: {}", e);
    }
}

async fn handle_request(
    req: Request<Body>,
    config: GatewayConfig,
) -> Result<Response<Body>, hyper::Error> {
    let path = req.uri().path();
    let svc_config = match get_svc_config(path.clone(), &config.services) {
        Some(svc_config) => svc_config,
        None => {
            return not_found();
        }
    };

    let auth_token = match authorize_user(&req.headers(), &config.authorization_api_url).await {
        Ok(header) => header,
        Err(_) => {
            return service_unavailable("Failed to connecto to Authorization API {}");
        }
    };

    let (parts, body) = req.into_parts();
    let downstream_req = build_downstream_req(parts, body, svc_config, auth_token).await?;

    match forward_request(downstream_req).await {
        Ok(res) => Ok(res),
        Err(_) => service_unavailable("Failed to connect to downstream service"),
    }
}

fn get_svc_config<'a>(path: &str, services: &'a [ServiceConfig]) -> Option<&'a ServiceConfig> {
    services.iter().find(|s| path.starts_with(&s.path))
}

async fn authorize_user(headers: &HeaderMap, auth_api_url: &str) -> Result<String, ()> {
    let auth_header_value = match headers.get(AUTHORIZATION) {
        Some(value) => value.to_str().unwrap_or_default(),
        None => "",
    };

    let auth_request = reqwest::Client::new()
        .get(auth_api_url)
        .header(AUTHORIZATION, auth_header_value);

    match auth_request.send().await {
        Ok(res) if res.status().is_success() => Ok(auth_header_value.to_string()),
        _ => Err(()),
    }
}

async fn build_downstream_req(
    parts: Parts,
    body: Body,
    svc_config: &ServiceConfig,
    auth_token: String,
) -> Result<Request<Body>, hyper::Error> {
}
