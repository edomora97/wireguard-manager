#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use failure::Error;
use futures::channel::mpsc;
use futures::{future, stream};
use futures_util::stream::StreamExt;
use futures_util::try_stream::TryStreamExt;
use signal_hook::iterator::Signals;
use tokio::prelude::*;
use tokio_postgres::{AsyncMessage, Client, NoTls};

use crate::config::ServerConfig;

pub mod config;
pub mod dns;
pub mod schema;
pub mod wireguard;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let config = config::read()?;

    let signals = Signals::new(&[
        signal_hook::SIGUSR1,
        signal_hook::SIGTERM,
        signal_hook::SIGINT,
    ])?;
    let config2 = config.clone();
    std::thread::spawn(move || {
        for signal in &signals {
            match signal {
                signal_hook::SIGUSR1 => {
                    // // This does not compile and I have no idea why
                    // let config = config2.clone();
                    // tokio::spawn(async move {
                    //     let (client, _) = tokio_postgres::connect(&config.database_url, NoTls)
                    //         .await
                    //         .unwrap();
                    //     update_server(&config, &client).await;
                    // });
                }
                signal_hook::SIGTERM | signal_hook::SIGINT => {
                    if let Err(e) = wireguard::unsetup_server(&config2) {
                        error!("Error tearing down the server: {:?}", e);
                        std::process::exit(1);
                    }
                    std::process::exit(0);
                }
                _ => unreachable!(),
            }
        }
    });

    // Connect to the database.
    debug!("Connecting to the database");
    let (client, mut connection) = tokio_postgres::connect(&config.database_url, NoTls).await?;
    debug!("Connected to the database");

    // Forward the notifications to the channel
    let (tx, rx) = mpsc::unbounded();
    let stream = stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!(e));
    let connection = stream.forward(tx).map(|r| r.unwrap());
    tokio::spawn(connection);

    // Make sure the schema is present
    schema::create_schema(&client).await?;
    debug!("Schema created");

    // Start listening for server notifications
    client.batch_execute("LISTEN update_server").await?;

    // Initial server setup
    wireguard::setup_server(&config).await?;
    info!("Server setup done");
    update_server(&config, &client).await;

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
