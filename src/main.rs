#[macro_use]
extern crate lazy_static;

use crate::config::ServerConfig;
use failure::Error;
use futures::channel::mpsc;
use futures::{future, stream};
use futures_util::stream::StreamExt;
use futures_util::try_stream::TryStreamExt;
use tokio::prelude::*;
use tokio_postgres::{AsyncMessage, Client, NoTls};

mod config;
mod dns;
mod schema;
mod wireguard;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = config::read()?;

    // Connect to the database.
    let (client, mut connection) = tokio_postgres::connect(&config.database_url, NoTls).await?;

    // Forward the notifications to the channel
    let (tx, rx) = mpsc::unbounded();
    let stream = stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!(e));
    let connection = stream.forward(tx).map(|r| r.unwrap());
    tokio::spawn(connection);

    // Make sure the schema is present
    schema::create_schema(&client).await?;

    // Start listening for server notifications
    client.batch_execute("LISTEN update_server").await?;

    // Initial server setup
    wireguard::setup_server(&config).await?;
    update_server(&config, &client).await;

    // Listen for server notifications
    rx.filter_map(|m| match m {
        AsyncMessage::Notification(n) => future::ready(Some(n)),
        _ => future::ready(None),
    })
    .for_each(|m| {
        println!("notification: {:?}", m);
        update_server(&config, &client)
    })
    .await;
    Ok(())
}

/// Update the server, first updating wireguard and then the DNS.
async fn update_server(config: &ServerConfig, client: &Client) {
    wireguard::update_server(&config, &client)
        .map(|res| res.unwrap())
        .await;
    dns::update_dns(&config, &client)
        .map(|res| res.unwrap())
        .await;
}
