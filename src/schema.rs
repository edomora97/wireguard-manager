use failure::Error;
use std::net::IpAddr;
use std::str::FromStr;

/// The schema of the database.
const SCHEMA: &'static str = "
CREATE TABLE IF NOT EXISTS servers (
  name TEXT PRIMARY KEY,
  subnet cidr NOT NULL,
  address inet NOT NULL CHECK(address << subnet),
  public_address inet NOT NULL,
  public_port INT NOT NULL,
  public_key TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS clients (
  name TEXT PRIMARY KEY,
  public_key TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS connections (
  server TEXT REFERENCES servers(name),
  client TEXT REFERENCES clients(name),
  address inet NOT NULL,
  PRIMARY KEY (server, client)
);

CREATE OR REPLACE FUNCTION notify_changes()
  RETURNS trigger
  AS $$
    BEGIN
      NOTIFY update_server;
      RETURN NULL;
    END;
  $$
  LANGUAGE PLPGSQL;

DROP TRIGGER IF EXISTS notify_servers_changed ON public.servers;
CREATE TRIGGER notify_servers_changed
  AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE
  ON servers
  EXECUTE FUNCTION notify_changes();
DROP TRIGGER IF EXISTS notify_clients_changed ON public.clients;
CREATE TRIGGER notify_clients_changed
  AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE
  ON clients
  EXECUTE FUNCTION notify_changes();
DROP TRIGGER IF EXISTS notify_connections_changed ON public.connections;
CREATE TRIGGER notify_connections_changed
  AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE
  ON connections
  EXECUTE FUNCTION notify_changes();
";
// TODO add trigger for checking that the ip addresses in `connections` are valid.

/// A server inside the wireguard network.
#[derive(Debug, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct Server {
    /// The name of the server, it is unique.
    pub name: String,
    /// The subnet of the network managed by the server.
    pub subnet_addr: IpAddr,
    /// The length of the network managed by the server.
    pub subnet_len: u8,
    /// The address with which the server can be reached from the outside.
    pub public_address: IpAddr,
    /// The port bound to wireguard.
    pub public_port: u16,
    /// The public key of the server.
    pub public_key: String,
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

/// Makes sure the schema is present. If the schema is outdated only bad things can happen.
pub async fn create_schema(client: &tokio_postgres::Client) -> Result<(), Error> {
    client.batch_execute(SCHEMA).await.map_err(|e| e.into())
}

/// Retrieve a list of all the servers in the database.
pub async fn get_servers(client: &tokio_postgres::Client) -> Result<Vec<Server>, Error> {
    let stmt = client
        .prepare(
            "SELECT name, host(subnet), masklen(subnet), host(public_address), public_port, public_key \
             FROM servers",
        )
        .await?;
    let rows = client.query(&stmt, &[]).await?;
    Ok(rows
        .into_iter()
        .map(|row| Server {
            name: row.get(0),
            subnet_addr: IpAddr::from_str(row.get(1)).unwrap(),
            subnet_len: row.get::<_, i32>(2) as u8,
            public_address: IpAddr::from_str(row.get(3)).unwrap(),
            public_port: row.get::<_, i32>(4) as u16,
            public_key: row.get(5),
        })
        .collect())
}

/// Retrieve a list of all the clients allowed to connect to the specified server.
pub async fn get_clients(
    client: &tokio_postgres::Client,
    server: &str,
) -> Result<Vec<ClientConnection>, Error> {
    let stmt = client
        .prepare(
            "SELECT server, name, public_key, host(address) \
             FROM connections \
             JOIN clients ON client = name \
             WHERE server = $1",
        )
        .await?;
    let rows = client.query(&stmt, &[&server.to_string()]).await?;
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
