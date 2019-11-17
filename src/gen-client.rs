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

    let conf = wireguard::gen_client_config(
        &config,
        &client,
        args[1].to_string(),
        args.get(2).map(|s| s.to_string()),
    )
    .await?;

    println!("{}", conf);

    Ok(())
}
