#[macro_use]
extern crate log;

use failure::Error;
use tokio::prelude::*;
use tokio_postgres::NoTls;

mod config;
mod schema;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} client [private key]", args[0]);
        std::process::exit(1);
    }

    let config = config::read()?;

    // Connect to the database.
    debug!("Connecting to the database");
    let (client, connection) = tokio_postgres::connect(&config.database_url, NoTls).await?;
    let connection = connection.map(|r| {
        if let Err(e) = r {
            eprintln!("connection error: {}", e);
        }
    });
    tokio::spawn(connection);
    debug!("Connected to the database");

    let connections = schema::get_client_connections(&client, &args[1]).await?;
    if connections.is_empty() {
        eprintln!("The user doesn't have a connection to any server");
        return Ok(());
    }
    let private_key = if args.len() < 3 {
        "<insert your private key>"
    } else {
        &args[2]
    };
    let addresses: Vec<_> = connections
        .iter()
        .map(|c| format!("{}/{}", c.address.to_string(), config.netmask_len))
        .collect();
    let addresses = addresses.join(",");

    let mut conf = String::new();
    conf += "[Interface]\n";
    conf += &format!("PrivateKey = {}\n", private_key);
    conf += &format!("Address = {}\n", addresses);

    // this is a list, even tho there could be at most one entry
    for connection in connections {
        let server = connection.server;
        conf += "\n";
        conf += "[Peer]\n";
        conf += &format!("PublicKey = {}\n", server.public_key);
        conf += &format!("AllowedIPs = {}/{}\n", config.network, config.netmask_len);
        conf += &format!(
            "Endpoint = {}:{}\n",
            server.public_address.to_string(),
            server.public_port
        );
        if let Some(keepalive) = config.keepalive {
            conf += &format!("PersistentKeepalive = {}\n", keepalive);
        }
    }

    println!("{}", conf);

    Ok(())
}
