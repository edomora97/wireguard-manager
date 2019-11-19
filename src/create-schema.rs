//! Command line tool for creating the schema of the database.
//!
//! The configuration file must be in the current working directory and be named `config.yaml`.

#[macro_use]
extern crate log;

use failure::Error;

pub mod config;
pub mod schema;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let config = config::read()?;

    // Connect to the database.
    debug!("Connecting to the database");
    let client = schema::connect(&config.database_url).await?;
    debug!("Connected to the database");

    // Make sure the schema is present
    schema::create_schema(&client).await?;
    info!("Schema created");

    Ok(())
}
