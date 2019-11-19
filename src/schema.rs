use failure::Error;
use std::net::IpAddr;
use std::str::FromStr;
use tokio_postgres::types::ToSql;
use tokio_postgres::Row;

/// The schema of the database.
const SCHEMA: &str = include_str!("schema.sql");

/// A server inside the wireguard network.
#[derive(Debug, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct Server {
    /// The name of the server, it is unique.
    pub name: String,
    /// The subnet of the network managed by the server.
    pub subnet_addr: IpAddr,
    /// The length of the network managed by the server.
    pub subnet_len: u8,
    /// The address of the server in its subnet.
    pub address: IpAddr,
    /// The address with which the server can be reached from the outside.
    pub public_address: IpAddr,
    /// The port bound to wireguard.
    pub public_port: u16,
    /// The public key of the server.
    pub public_key: String,
}

impl Server {
    /// Build a `Server` from a row returned by an SQL query of the form:
    ///     SELECT name, host(subnet), masklen(subnet), host(servers.address),
    ///            host(public_address), public_port, public_key
    ///     FROM servers
    fn from_sql(row: &Row, start_index: usize) -> Server {
        Server {
            name: row.get(start_index),
            subnet_addr: IpAddr::from_str(row.get(start_index + 1)).unwrap(),
            subnet_len: row.get::<_, i32>(start_index + 2) as u8,
            address: IpAddr::from_str(row.get(start_index + 3)).unwrap(),
            public_address: IpAddr::from_str(row.get(start_index + 4)).unwrap(),
            public_port: row.get::<_, i32>(start_index + 5) as u16,
            public_key: row.get(start_index + 6),
        }
    }
}

/// A client inside the wireguard network.
#[derive(Debug, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct Client {
    /// The name of the client, it is unique.
    pub name: String,
    /// The public key of the client.
    pub public_key: String,
}

/// The authorization for a user to connect to a server, including its private address.
#[derive(Debug, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct ClientConnection {
    /// The name of the server.
    pub server: String,
    /// The client.
    pub client: Client,
    /// The address of the client, connecting to that server.
    pub address: IpAddr,
}

/// The details of the connection to a server.
#[derive(Debug, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct ServerConnection {
    /// The name of the server.
    pub server: Server,
    /// The address of the client, connecting to that server.
    pub address: IpAddr,
}

/// Makes sure the schema is present. If the schema is outdated only bad things can happen.
pub async fn create_schema(client: &tokio_postgres::Client) -> Result<(), Error> {
    client.batch_execute(SCHEMA).await.map_err(|e| e.into())
}

/// Retrieve a list of all the servers in the database.
pub async fn get_servers(client: &tokio_postgres::Client) -> Result<Vec<Server>, Error> {
    let stmt = client
        .prepare(
            "SELECT name, host(subnet), masklen(subnet), host(address), host(public_address), public_port, public_key \
             FROM servers",
        )
        .await?;
    let rows = client.query(&stmt, &[]).await?;
    Ok(rows
        .into_iter()
        .map(|row| Server::from_sql(&row, 0))
        .collect())
}

/// Retrieve a list of all the clients allowed to connect to the specified server.
/// If the specified server is `None`, all the clients are returned.
pub async fn get_clients<S: AsRef<str>>(
    client: &tokio_postgres::Client,
    server: Option<S>,
) -> Result<Vec<ClientConnection>, Error> {
    let mut query = "SELECT server, name, public_key, host(address) \
                     FROM connections \
                     JOIN clients ON client = name"
        .to_string();
    if server.is_some() {
        query += " WHERE server = $1";
    }
    // build the server name, the optional parameter of the query. Cannot build it conditionally
    // because a ref to it is needed when passing the parameter to `client.query`, which has a very
    // picky type.
    let server_name = server
        .as_ref()
        .map(|s| s.as_ref().to_string())
        .unwrap_or_default();
    let params: Vec<&(dyn ToSql + Sync)> = if server.is_some() {
        vec![&server_name]
    } else {
        vec![]
    };
    let stmt = client.prepare(&query).await?;
    let rows = client.query(&stmt, params.as_slice()).await?;
    Ok(rows
        .into_iter()
        .map(|row| ClientConnection {
            server: row.get(0),
            client: Client {
                name: row.get(1),
                public_key: row.get(2),
            },
            address: IpAddr::from_str(row.get(3)).unwrap(),
        })
        .collect())
}

/// Fetch the list of servers the client can connect to.
pub async fn get_client_connections<S: Into<String>>(
    client: &tokio_postgres::Client,
    name: S,
) -> Result<Vec<ServerConnection>, Error> {
    let stmt = client
        .prepare(
            "SELECT name, host(subnet), masklen(subnet), host(servers.address), \
             host(public_address), public_port, public_key, host(connections.address) \
             FROM servers JOIN connections ON servers.name = connections.server \
             WHERE connections.client = $1",
        )
        .await?;
    let rows = client.query(&stmt, &[&name.into()]).await?;
    Ok(rows
        .into_iter()
        .map(|row| ServerConnection {
            server: Server::from_sql(&row, 0),
            address: IpAddr::from_str(row.get(7)).unwrap(),
        })
        .collect())
}
