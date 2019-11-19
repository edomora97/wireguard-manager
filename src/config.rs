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
    /// The entire private network.
    pub network: String,
    /// Length of the subnet of the entire private network.
    pub netmask_len: u8,
    /// Which address to listen to for the web interface
    pub web_listen_address: String,
    /// Which port to listen to for the web interface
    pub web_listen_port: u16,
    /// Path to where the static web content is stored
    pub web_static_dir: PathBuf,
}

/// Read the configuration file.
pub fn read() -> Result<ServerConfig, Error> {
    let file = std::fs::File::open("config.yaml")
        .map_err(|e| format_err!("Cannot read configuration file: {}", e))?;
    let mut config: ServerConfig = serde_yaml::from_reader(file)?;
    // Make sure the directory is absolute.
    config.web_static_dir = config.web_static_dir.canonicalize()?;
    Ok(config)
}
