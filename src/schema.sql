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
  address inet NOT NULL UNIQUE,
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

CREATE OR REPLACE FUNCTION check_integrity()
  RETURNS trigger
  AS $$
    BEGIN
      IF (SELECT COUNT(*) FROM connections c JOIN servers s on c.server = s.name WHERE NOT (c.address << subnet)) > 0
      THEN
        RAISE EXCEPTION 'Connection with address outside the server';
      END IF;

      IF (SELECT COUNT(*) FROM servers s1 JOIN servers s2 on s1.name != s2.name WHERE (s1.subnet <<= s2.subnet) OR (s1.subnet >>= s2.subnet)) > 0
      THEN
        RAISE EXCEPTION 'Overlapping server subnets';
      END IF;

      IF (SELECT COUNT(*) FROM connections c JOIN servers s on c.server = s.name WHERE c.address = s.address) > 0
      THEN
        RAISE EXCEPTION 'Client with server ip address';
      END IF;
      RETURN NULL;
    END
  $$
  LANGUAGE PLPGSQL;

DROP TRIGGER IF EXISTS check_integrity_servers ON public.servers;
CREATE TRIGGER check_integrity_servers
  AFTER INSERT OR UPDATE
  ON servers
  EXECUTE FUNCTION check_integrity();
DROP TRIGGER IF EXISTS check_integrity_clients ON public.clients;
CREATE TRIGGER check_integrity_clients
  AFTER INSERT OR UPDATE
  ON clients
  EXECUTE FUNCTION check_integrity();
DROP TRIGGER IF EXISTS check_integrity_connections ON public.connections;
CREATE TRIGGER check_integrity_connections
  AFTER INSERT OR UPDATE
  ON connections
  EXECUTE FUNCTION check_integrity();
