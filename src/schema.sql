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

-- TODO add trigger for checking that the ip addresses in `connections` are valid.
