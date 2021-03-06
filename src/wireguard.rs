use std::process::Stdio;

use failure::{bail, Error};
use regex::Regex;
use tempfile::NamedTempFile;
use tokio::net::process::Command;
use tokio_postgres::Client;

use crate::config::ServerConfig;
use crate::schema;
use crate::schema::{ClientConnection, Server};
use std::net::IpAddr;
use std::str::FromStr;

lazy_static! {
    /// Search for the ip addresses of the network interface.
    static ref RE: Regex = Regex::new(r"inet6? ([^\s]+)/(\d+)").unwrap();
}

/// Setup the server's wireguard configuration.
pub async fn setup_server(config: &ServerConfig) -> Result<(), Error> {
    make_interface(config).await?;
    Ok(())
}

/// Tear down the server synchronously.
pub fn unsetup_server(config: &ServerConfig) -> Result<(), Error> {
    let child = std::process::Command::new("ip")
        .args(&["link", "delete", &config.device_name])
        .spawn()?
        .wait()?;
    if child.success() {
        info!("Removed device {}", config.device_name);
        Ok(())
    } else {
        bail!(
            "Failed to delete the device {}: exit code {:?}",
            config.device_name,
            child.code()
        );
    }
}

/// Update the wireguard server configuration.
pub async fn update_server(config: &ServerConfig, client: &Client) -> Result<(), Error> {
    ensure_conf(config, client).await?;
    ensure_ip(config, client).await?;
    Ok(())
}

/// Create the wireguard interface.
async fn make_interface(config: &ServerConfig) -> Result<(), Error> {
    let child = Command::new("ip")
        .args(&["link", "show", &config.device_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?
        .await?;
    // ip link show didn't fail => the device already exists
    if child.success() {
        return Ok(());
    }
    let child = Command::new("ip")
        .args(&[
            "link",
            "add",
            "dev",
            &config.device_name,
            "type",
            "wireguard",
        ])
        .spawn()?
        .await?;
    if child.success() {
        info!("Interface {} created successfully", config.device_name);
    } else {
        bail!("Failed to add the device: exit code {:?}", child.code());
    }
    let child = Command::new("ip")
        .args(&["link", "set", "up", "dev", &config.device_name])
        .spawn()?
        .await?;
    if child.success() {
        info!("Interface {} brought up successfully", config.device_name);
        Ok(())
    } else {
        bail!(
            "Failed to bring up the device: exit code {:?}",
            child.code()
        );
    }
}

/// Make sure the interface has the correct ip addresses.
async fn ensure_ip(config: &ServerConfig, client: &Client) -> Result<(), Error> {
    let servers = schema::get_servers(&client).await?;
    let server = servers
        .iter()
        .find(|s| s.name == config.name)
        .expect("Server is not registered in the db");
    let ips = Command::new("ip")
        .args(&["addr", "show", &config.device_name])
        .output()
        .await?;
    if !ips.status.success() {
        bail!("Failed to get ips of interface: {:?}", ips);
    }
    let stdout = String::from_utf8_lossy(&ips.stdout);
    let mut present = false; // whether the correct address is already present
                             // need to collect the results because the regex state cannot be sent between thread, and this
                             // doesn't play well with the await inside the loop.
    let ips: Vec<_> = RE.captures_iter(&stdout).collect();
    for ip in ips.into_iter() {
        let addr = IpAddr::from_str(&ip[1])?;
        let len = u8::from_str(&ip[2])?;
        // wrong ip or wrong network length
        if addr != server.address || len != config.netmask_len {
            warn!(
                "Wrong address {}/{} found in {}, removing it",
                addr.to_string(),
                len,
                config.device_name
            );
            remove_ip(config, addr, len).await?;
        } else {
            present = true;
        }
    }
    // address is not already present, add it
    if !present {
        info!(
            "Adding address {}/{} to device {}",
            server.address.to_string(),
            config.netmask_len,
            config.device_name
        );
        add_ip(config, server.address, config.netmask_len).await?;
    }
    Ok(())
}

/// Remove an ip address from the network device.
async fn remove_ip(config: &ServerConfig, address: IpAddr, len: u8) -> Result<(), Error> {
    let addr = format!("{}/{}", address.to_string(), len);
    let cmd = Command::new("ip")
        .args(&["addr", "delete", "dev", &config.device_name, &addr])
        .spawn()?
        .await?;
    if cmd.success() {
        info!(
            "Removed {}/{} from {}",
            address.to_string(),
            len,
            config.device_name
        );
        Ok(())
    } else {
        bail!(
            "Failed to remove {}/{} from {}: exit code {:?}",
            address.to_string(),
            len,
            config.device_name,
            cmd.code()
        );
    }
}

/// Add an ip address to the network device.
async fn add_ip(config: &ServerConfig, address: IpAddr, len: u8) -> Result<(), Error> {
    let addr = format!("{}/{}", address.to_string(), len);
    let cmd = Command::new("ip")
        .args(&["addr", "add", "dev", &config.device_name, &addr])
        .spawn()?
        .await?;
    if cmd.success() {
        info!(
            "Added {}/{} to {}",
            address.to_string(),
            len,
            config.device_name
        );
        Ok(())
    } else {
        bail!(
            "Failed to add {}/{} to {}: exit code {:?}",
            address.to_string(),
            len,
            config.device_name,
            cmd.code()
        );
    }
}

/// Build the last version of the wireguard configuration and use it.
async fn ensure_conf(config: &ServerConfig, client: &Client) -> Result<(), Error> {
    let server_config = gen_server_config(config, client).await?;
    debug!("Wireguard configuration is:\n{}", server_config);
    let tmpfile = NamedTempFile::new()?;
    tokio::fs::write(tmpfile.path().to_path_buf(), server_config.as_bytes()).await?;
    let child = Command::new("wg")
        .arg("setconf")
        .arg(&config.device_name)
        .arg(tmpfile.path())
        .spawn()?
        .await?;
    if child.success() {
        info!("Wireguard configuration updated successfully");
        Ok(())
    } else {
        bail!("Wireguard failed with {:?}", child.code());
    }
}

/// Generate the configuration of this server fetching its configuration from the database.
async fn gen_server_config(config: &ServerConfig, client: &Client) -> Result<String, Error> {
    let servers = schema::get_servers(&client).await?;
    let server = servers
        .iter()
        .find(|s| s.name == config.name)
        .expect("Server is not registered in the db");
    let clients = schema::get_clients(client, Some(&config.name)).await?;
    let mut server_conf = gen_server_interface(config, server);
    server_conf += &gen_server_to_server_peers(config, &servers);
    server_conf += &gen_server_to_client_peers(&clients);
    Ok(server_conf)
}

/// Generate the `[Interface]` part of the server configuration.
fn gen_server_interface(config: &ServerConfig, server: &Server) -> String {
    let mut conf = String::new();
    conf += "[Interface]\n";
    conf += &format!("ListenPort = {}\n", server.public_port);
    conf += &format!("PrivateKey = {}\n", config.private_key);
    conf
}

/// Generate the `[Peer]` part of the server configuration relative to the connection with the other
/// servers in the network.
fn gen_server_to_server_peers(config: &ServerConfig, servers: &[Server]) -> String {
    let mut conf = String::new();
    for server in servers {
        if server.name == config.name {
            continue;
        }
        conf += "\n";
        conf += "[Peer]\n";
        conf += &format!("PublicKey = {}\n", server.public_key);
        conf += &format!(
            "AllowedIPs = {}/{}\n",
            server.subnet_addr, server.subnet_len
        );
        conf += &format!(
            "Endpoint = {}:{}\n",
            server.public_address, server.public_port
        );
        if let Some(keepalive) = config.keepalive {
            conf += &format!("PersistentKeepalive = {}\n", keepalive);
        }
    }
    conf
}

/// Generate the `[Peer]` part of the server configuration relative to the connection with the
/// authorized clients.
fn gen_server_to_client_peers(clients: &[ClientConnection]) -> String {
    let mut conf = String::new();
    for client in clients {
        conf += "\n";
        conf += "[Peer]\n";
        conf += &format!("PublicKey = {}\n", client.client.public_key);
        let len = if client.address.is_ipv4() { 32 } else { 128 };
        conf += &format!("AllowedIPs = {}/{}\n", client.address, len);
    }
    conf
}

/// Generate the configuration file of a client. If the private key has not been passed, a
/// placeholder is used instead.
pub async fn gen_client_config(
    config: &ServerConfig,
    client: &Client,
    name: String,
    private_key: Option<String>,
) -> Result<String, Error> {
    let connections = schema::get_client_connections(&client, &name).await?;
    if connections.is_empty() {
        bail!("The user doesn't have a connection to any server");
    }
    let private_key = private_key.unwrap_or_else(|| "<insert your private key>".to_string());
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

    Ok(conf)
}
