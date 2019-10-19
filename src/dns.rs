use failure::Error;
use tokio_postgres::Client;

use crate::config::ServerConfig;
use crate::schema;
use crate::schema::{ClientConnection, Server};

/// Update the hosts file of the DNS server.
pub async fn update_dns(config: &ServerConfig, client: &Client) -> Result<(), Error> {
    let conf = gen_dns_config(config, client).await?;
    debug!("DNS configuration:\n{}", conf);
    tokio::fs::write(config.dns_hosts_file.clone(), conf).await?;
    Ok(())
}

/// Generate the dns configuration.
async fn gen_dns_config(config: &ServerConfig, client: &Client) -> Result<String, Error> {
    let servers = schema::get_servers(client).await?;
    let clients = schema::get_clients(client, &config.name).await?;
    let mut conf = gen_server_entries(config, &servers);
    conf += &gen_clients_entries(config, &clients);
    Ok(conf)
}

/// Generate the entries of the servers.
fn gen_server_entries(config: &ServerConfig, servers: &[Server]) -> String {
    let mut res = String::new();
    res += "# Servers\n";
    for server in servers {
        res += &format!(
            "{:<20} {}.{}\n",
            server.address.to_string(),
            server.name,
            config.base_domain
        );
    }
    res
}

/// Generate the entries of the clients.
fn gen_clients_entries(config: &ServerConfig, clients: &[ClientConnection]) -> String {
    let mut res = String::new();
    res += "\n# Clients\n";
    for client in clients {
        res += &format!(
            "{:<20} {}.{}.{}\n",
            client.address.to_string(),
            client.client.name,
            client.server,
            config.base_domain
        );
    }
    res
}
