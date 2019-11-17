use crate::config::ServerConfig;
use failure::Error;
use hyper::{Body, Request, Response, StatusCode};
use serde::Serialize;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio_postgres::Client;
use wireguard_manager::schema;

#[derive(Debug, Clone, Serialize)]
struct NetworkStatusServer {
    pub name: String,
    pub subnet: String,
    pub subnet_len: u8,
    pub address: String,
    pub endpoint: String,
    pub endpoint_port: u16,
}

#[derive(Debug, Clone, Serialize)]
struct NetworkStatusClient {
    pub name: String,
    pub server: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize)]
struct NetworkStatus {
    pub servers: Vec<NetworkStatusServer>,
    pub clients: Vec<NetworkStatusClient>,
    pub base_domain: String,
}

pub async fn handle_request<T>(
    req: Request<T>,
    client: &Client,
    config: &ServerConfig,
) -> Result<Response<Body>, Error> {
    match req.uri().path() {
        "/data" => {
            let servers = schema::get_servers(client).await?;
            let servers = servers
                .into_iter()
                .map(|s| NetworkStatusServer {
                    name: s.name,
                    subnet: s.subnet_addr.to_string(),
                    subnet_len: s.subnet_len,
                    address: s.address.to_string(),
                    endpoint: s.public_address.to_string(),
                    endpoint_port: s.public_port,
                })
                .collect();
            let clients = schema::get_clients(client, None)
                .await?
                .into_iter()
                .map(|c| NetworkStatusClient {
                    name: c.client.name,
                    server: c.server,
                    address: c.address.to_string(),
                })
                .collect();
            let status = NetworkStatus {
                servers,
                clients,
                base_domain: config.base_domain.clone(),
            };
            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string_pretty(&status)?))
                .unwrap())
        }
        _ => {
            let path = if req.uri().path() == "/" {
                "/index.html"
            } else {
                req.uri().path()
            };
            let path = config
                .web_static_dir
                .join(&path[1..])
                .canonicalize()
                .unwrap_or_default();
            if path.starts_with(&config.web_static_dir) {
                if let Ok(mut file) = File::open(path).await {
                    let mut buf = Vec::new();
                    if let Ok(_) = file.read_to_end(&mut buf).await {
                        return Ok(Response::new(buf.into()));
                    }
                }
            }
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}
