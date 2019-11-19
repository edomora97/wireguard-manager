#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use crate::config::ServerConfig;
use failure::Error;
use futures::future;
use futures::future::Ready;
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::prelude::*;
use tokio_net::signal;
use tokio_net::signal::unix::SignalKind;
use tokio_postgres::{AsyncMessage, Client};

pub mod config;
pub mod dns;
pub mod schema;
pub mod web;
pub mod wireguard;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let config = config::read()?;

    // Exit tearing down on Control-C.
    let config2 = config.clone();
    tokio::spawn(signal::ctrl_c()?.for_each(move |_| -> Ready<()> {
        if let Err(e) = wireguard::unsetup_server(&config2) {
            error!("Error tearing down the server: {:?}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }));

    // Connect to the database.
    debug!("Connecting to the database");
    let (client, rx) = schema::connect_with_notifications(&config.database_url).await?;
    debug!("Connected to the database");

    let client_arc = Arc::new(client);
    let client = client_arc.as_ref();

    // Reload the configuration from the DB on SIGUSR1.
    let client_arc2 = client_arc.clone();
    let config3 = config.clone();
    tokio::spawn(
        signal::unix::signal(SignalKind::user_defined1())?.for_each(move |_| {
            info!("Reloading due to SIGUSR1");
            let config = config3.clone();
            let client = client_arc2.clone();
            async move { update_server(&config, client.as_ref()).await }
        }),
    );

    // Make sure the schema is present
    schema::create_schema(&client).await?;
    debug!("Schema created");

    // Start listening for server notifications
    client.batch_execute("LISTEN update_server").await?;

    // Initial server setup
    wireguard::setup_server(&config).await?;
    info!("Server setup done");
    update_server(&config, &client).await;

    // Spawn the web server for the network statistics
    spawn_web_server(&config, client_arc.clone())?;

    // Listen for server notifications
    rx.filter_map(|m| match m {
        AsyncMessage::Notification(n) => future::ready(Some(n)),
        _ => future::ready(None),
    })
    .for_each(|m| {
        info!("Database update notification: {:?}", m);
        update_server(&config, &client)
    })
    .await;
    Ok(())
}

/// Update the server, first updating wireguard and then the DNS.
async fn update_server(config: &ServerConfig, client: &Client) {
    info!("Updating server configuration");
    wireguard::update_server(&config, &client)
        .map(|res| res.unwrap())
        .await;
    dns::update_dns(&config, &client)
        .map(|res| res.unwrap())
        .await;
}

/// Spawn the web server and listen to the port specified in the configuration file.
fn spawn_web_server(config: &ServerConfig, client_arc: Arc<Client>) -> Result<(), Error> {
    let addr = SocketAddr::new(
        IpAddr::from_str(&config.web_listen_address)?,
        config.web_listen_port,
    );
    let config = Arc::new(config.clone());
    let service = make_service_fn(move |_| {
        let client_arc = client_arc.clone();
        let config = config.clone();

        async move {
            Ok::<_, Error>(service_fn(move |req| {
                let client = client_arc.clone();
                let config = config.clone();
                async move { web::handle_request(req, client.as_ref(), config.as_ref()).await }
            }))
        }
    });

    let server = Server::bind(&addr).serve(service);
    info!("Web interface listening on http://{}", addr);
    tokio::spawn(server.map(|_| ()));
    Ok(())
}
