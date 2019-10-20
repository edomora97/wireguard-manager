use failure::{format_err, Error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The private configuration of a server.
#[derive(Debug, Clone, Eq, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct ServerConfig {
    /// The name of the server, there must be an entry in the database with the same name.
    pub name: String,
    /// The private key of the server.
    pub private_key: String,
    /// An optional keep-alive to use for every peer.
    pub keepalive: Option<u32>,
    /// The name of the network device to create.
    pub device_name: String,
    /// The connection string to the database.
    pub database_url: String,
    /// Domain suffix to use for the DNS, without the leading dot.
    pub base_domain: String,
    /// Path to the file where to put the hosts entries. Use --hostsdir in dnsmasq.
    pub dns_hosts_file: PathBuf,
    /// Length of the subnet of the entire private network.
    pub netmask_len: u8,
}

/// Read the configuration file.
pub fn read() -> Result<ServerConfig, Error> {
    let file = std::fs::File::open("config.yaml")
        .map_err(|e| format_err!("Cannot read configuration file: {}", e))?;
    serde_yaml::from_reader(file).map_err(|e| e.into())
}
