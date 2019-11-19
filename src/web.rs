use crate::config::ServerConfig;
use crate::schema;
use crate::wireguard::gen_client_config;
use failure::Error;
use hyper::{Body, Request, Response, StatusCode};
use serde::Serialize;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio_postgres::Client;

/// Status of a server in the network. This will be serialized and exposed in the JSON API.
#[derive(Debug, Clone, Serialize)]
struct NetworkStatusServer {
    /// The name of the server.
    pub name: String,
    /// The subnet the server manages.
    pub subnet: String,
    /// The length of the subnet of the server.
    pub subnet_len: u8,
    /// The address of the server inside its subnet.
    pub address: String,
    /// The public address of the server.
    pub endpoint: String,
    /// The public port of the server.
    pub endpoint_port: u16,
}

/// Status of a client in the network. This will be serialized and exposed in the JSON API.
#[derive(Debug, Clone, Serialize)]
struct NetworkStatusClient {
    /// The name of the client.
    pub name: String,
    /// The name of the server.
    pub server: String,
    /// The private IP address of the client in the server's network.
    pub address: String,
}

/// The response of the `/data` JSON API.
#[derive(Debug, Clone, Serialize)]
struct NetworkStatus {
    /// The list of the servers in the network.
    pub servers: Vec<NetworkStatusServer>,
    /// The list of the clients in the network.
    pub clients: Vec<NetworkStatusClient>,
    /// The base domain of the DNS.
    pub base_domain: String,
}

/// Handle a web request asynchronously.
pub async fn handle_request<T>(
    req: Request<T>,
    client: &Client,
    config: &ServerConfig,
) -> Result<Response<Body>, Error> {
    match req.uri().path() {
        // JSON API with the status of the network.
        "/data" => {
            let servers = schema::get_servers(client)
                .await?
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
            let clients = schema::get_clients(client, None::<&str>)
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
        // Generate the client configuration for a given username.
        url if url.starts_with("/conf/") => {
            let name = &url[6..];
            let conf = gen_client_config(config, client, name.to_owned(), None).await;
            match conf {
                Ok(conf) => Ok(Response::builder()
                    .status(200)
                    .body(Body::from(conf))
                    .unwrap()),
                Err(err) => Ok(Response::builder()
                    .status(404)
                    .body(Body::from(err.to_string()))
                    .unwrap()),
            }
        }
        // Any other static file.
        _ => {
            // if asking for an index, manually change the file name.
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
                if let Ok(mut file) = File::open(&path).await {
                    debug!("Sending file {:?}", path);
                    let mut buf = Vec::new();
                    if file.read_to_end(&mut buf).await.is_ok() {
                        return Ok(Response::new(buf.into()));
                    }
                }
            }
            warn!("404 File Not Found: {} -> {:?}", req.uri().path(), path);
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}
