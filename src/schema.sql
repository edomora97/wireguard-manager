-- The table where the servers are stored.
CREATE TABLE IF NOT EXISTS servers (
  name TEXT PRIMARY KEY,
  subnet cidr NOT NULL,
  address inet NOT NULL CHECK(address << subnet),
  public_address inet NOT NULL,
  public_port INT NOT NULL,
  public_key TEXT NOT NULL
);
-- The table where the clients are stored.
CREATE TABLE IF NOT EXISTS clients (
  name TEXT PRIMARY KEY,
  public_key TEXT NOT NULL
);
-- The table where the relation between the clients and the servers is stored.
-- A client can connect only to the servers listed here.
CREATE TABLE IF NOT EXISTS connections (
  server TEXT REFERENCES servers(name),
  client TEXT UNIQUE REFERENCES clients(name),
  address inet NOT NULL UNIQUE,
  PRIMARY KEY (server, client)
);

-- Calling this function will publish an event sent to all the servers that are
-- listening, causing them to reload the changes in the database.
CREATE OR REPLACE FUNCTION notify_changes()
  RETURNS trigger
  AS $$
    BEGIN
      NOTIFY update_server;
      RETURN NULL;
    END;
  $$
  LANGUAGE PLPGSQL;

-- Send an update to the servers if the `servers` table changes.
DROP TRIGGER IF EXISTS notify_servers_changed ON public.servers;
CREATE TRIGGER notify_servers_changed
  AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE
  ON servers
  EXECUTE PROCEDURE notify_changes();

-- Send an update to the servers if the `clients` table changes.
DROP TRIGGER IF EXISTS notify_clients_changed ON public.clients;
CREATE TRIGGER notify_clients_changed
  AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE
  ON clients
  EXECUTE PROCEDURE notify_changes();

-- Send an update to the servers if the `connections` table changes.
DROP TRIGGER IF EXISTS notify_connections_changed ON public.connections;
CREATE TRIGGER notify_connections_changed
  AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE
  ON connections
  EXECUTE PROCEDURE notify_changes();

-- This function will raise an exception if some integrity constraints are
-- violated. The constraints checked here are hard/impossible to do with CHECK
-- clauses.
CREATE OR REPLACE FUNCTION check_integrity()
  RETURNS trigger
  AS $$
    BEGIN
      -- Make sure the client addresses are inside the correct network.
      IF (SELECT COUNT(*)
          FROM connections c
          JOIN servers s ON c.server = s.name
          WHERE NOT (c.address << subnet)) > 0
      THEN
        RAISE EXCEPTION 'Connection with address outside the server';
      END IF;

      -- Make sure that all the server networks are disjoint.
      IF (SELECT COUNT(*)
          FROM servers s1
          JOIN servers s2 ON s1.name != s2.name
          WHERE (s1.subnet <<= s2.subnet) OR (s1.subnet >>= s2.subnet)) > 0
      THEN
        RAISE EXCEPTION 'Overlapping server subnets';
      END IF;

      -- Make sure the intersection of server IPs and client IPs is empty.
      IF (SELECT COUNT(*)
          FROM connections c
          JOIN servers s ON c.server = s.name
          WHERE c.address = s.address) > 0
      THEN
        RAISE EXCEPTION 'Client with server ip address';
      END IF;

      -- Make sure no client has the name of a server.
      IF (SELECT COUNT(*) FROM clients c JOIN servers s ON c.name = s.name) > 0
      THEN
          RAISE EXCEPTION 'Client with server name';
      END IF;

      RETURN NULL;
    END
  $$
  LANGUAGE PLPGSQL;

-- Check the constraints on `servers` changes.
DROP TRIGGER IF EXISTS check_integrity_servers ON public.servers;
CREATE TRIGGER check_integrity_servers
  AFTER INSERT OR UPDATE
  ON servers
  EXECUTE PROCEDURE check_integrity();

-- Check the constraints on `clients` changes.
DROP TRIGGER IF EXISTS check_integrity_clients ON public.clients;
CREATE TRIGGER check_integrity_clients
  AFTER INSERT OR UPDATE
  ON clients
  EXECUTE PROCEDURE check_integrity();

-- Check the constraints on `connections` changes.
DROP TRIGGER IF EXISTS check_integrity_connections ON public.connections;
CREATE TRIGGER check_integrity_connections
  AFTER INSERT OR UPDATE
  ON connections
  EXECUTE PROCEDURE check_integrity();
