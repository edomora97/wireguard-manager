use std::path::PathBuf;

/// The private configuration of a server.
#[derive(Debug, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct ServerConfig {
    /// The name of the server, there must be an entry in the database with the same name.
    pub name: String,
    /// The private key of the server.
    pub private_key: String,
    /// An optional keep-alive to use for every peer.
    pub keepalive: Option<u32>,
    /// The name of the network device to create.
    pub device_name: String,
    /// The path where to store the configuration file.
    pub config_path: PathBuf,
}
