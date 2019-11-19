//! Command line tool for generating the configuration file of a client.
//!
//! Usage: gen-client username [private key]
//!
//! The username must be attached to a server. If the private key is not provided, it must be added
//! manually to the produced configuration.

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use failure::Error;
use tokio::prelude::*;
use tokio_postgres::NoTls;

pub mod config;
pub mod schema;
pub mod wireguard;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    if args.len() < 2 || args.iter().any(|a| *a == "--help" || *a == "-h") {
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

    let username = args[1].to_string();
    let private_key = args.get(2).map(|s| s.to_string());
    let conf = wireguard::gen_client_config(&config, &client, username, private_key).await;

    match conf {
        Ok(conf) => println!("{}", conf),
        Err(e) => {
            eprintln!("Error: {}", e.to_string());
            std::process::exit(1);
        }
    }

    Ok(())
}
