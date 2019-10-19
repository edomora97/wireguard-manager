use std::io::Write;

use failure::{bail, Error};
use tempfile::NamedTempFile;
use tokio::net::process::Command;
use tokio_postgres::Client;

use crate::config::ServerConfig;
use crate::schema;
use crate::schema::{ClientConnection, Server};

/// Generate and update the currently running wireguard configuration.
pub async fn setup_server(config: &ServerConfig, client: &Client) -> Result<(), Error> {
    make_interface(config).await?;
    update_server(config, client).await?;
    Ok(())
}

pub async fn update_server(config: &ServerConfig, client: &Client) -> Result<(), Error> {
    let server_config = gen_server_config(config, client).await?;
    let mut tmpfile = NamedTempFile::new()?;
    tmpfile.write_all(server_config.as_bytes())?;
    // TODO: remove `echo`
    let child = Command::new("echo")
        .arg("wg")
        .arg("setconf")
        .arg(&config.device_name)
        .arg(tmpfile.path())
        .spawn()?
        .await?;
    if child.success() {
        Ok(())
    } else {
        bail!("Wireguard failed with {:?}", child.code());
    }
}

async fn make_interface(config: &ServerConfig) -> Result<(), Error> {
    // TODO: remove `echo`
    let child = Command::new("echo")
        .arg("ip")
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
        Ok(())
    } else {
        bail!(
            "Failed to add the device: ip link add failed with {:?}",
            child.code()
        );
    }
}

/// Generate the configuration of this server fetching its configuration from the database.
async fn gen_server_config(config: &ServerConfig, client: &Client) -> Result<String, Error> {
    let servers = schema::get_servers(&client).await?;
    let server = servers
        .iter()
        .find(|s| s.name == config.name)
        .expect("Server is not registered in the db");
    let clients = schema::get_clients(client, &config.name).await?;
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
