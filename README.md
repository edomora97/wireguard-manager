# wireguard-manager

Build and manage a network of servers that provides a distributed VPN using wireguard.

The network topology is the following:
- The servers are connected in complete mesh.
  This allows a 2-hops-max connection between every client and the routing is trivial.
- Each client is connected to a single server but it's reachable from every other client in the network.

All the dynamic configuration is kept inside a postgres database and the servers listen for database changes for automatic reloading.

Each client and server is identified by a unique name, which is also used for DNS name resolution.
The system is built for being used with IPv4 or IPv6 in the internal network.

The requirements on the server side are:
- `wireguard` kernel module installed and active.
- `wireguard-tools` installed.
- _(optional)_ `dnsmasq` installed and running.

All but the first requirement are already provided by the docker image.

On the client side nothing more than `wg-quick` (or compatible) is required.
Android is supported using the official app.

## Database structure

The schema of the database is very simple, you can find it at `src/schema.sql`.
There are 3 main tables:

- `servers` with the configuration of the servers in the network, including private and public addresses, port numbers and public keys.
- `clients` with the name and public key of the clients.
- `connections` with the association of client → server.

Note that on the database only public keys are stored.

Editing those tables automatically updated the configurations on the server.
This is done using postgres' pub/sub functionalities.

## Initial Setup

At first you have to setup postgres somewhere accessible from all the servers.
No specific configuration is needed, a user must be present and be able to connect and create tables.

From now on it's assumed to be known an address like `postgresql://user:pass@host/db` for connecting to the database.

Some decisions must be taken in advance:
- A base domain under which all the DNS names will be put, in the examples `vpn.example.com`.
- A network prefix for the entire VPN, in the examples `fd12::/48`.

In order to have the DNS working some extra steps are required:
- Add a `NS` entry for the zone of the network, in the example `vpn.example.com`, pointing to the servers used as _authoritative DNS server_.
- Remember to bind the port 53 (tcp and udp) of those servers.

## Adding a server to the network

- Generate a private and a public key for wireguard (`wg genkey` and `wg pubkey`), in the example `SERVER_PRIVATE_KEY` and `SERVER_PUBLIC_KEY`.
- Choose a name for the server, in the example `srv1`.
- Copy `example.config.yaml` and put it in `config.yaml`.
- Update the setting needed, at least `name`, `private_key`, `database_url`, `base_domain`, `network`, `netmask_len`.
- Add the entry for the server in the `servers` table in the database.
  - It's important that the `name` column matches the values set in `config.yaml`.
  - The subnet value must be smaller or equal than the entire network one and inside of it, in the example `fd12::/64`.
  - A second server must use a network that does not intersect with it, for example `fd12:0:0:1::/64`.
- Start¹ `wireguard-manager` in the same directory of `config.yaml`.
- Start `dnsmasq` pointing `--addn-hosts` to the path specified with `dns_hosts_file` in `config.yaml`.

**Note** Running `dnsmasq` is only required if you want this server to be an _authoritative DNS server_ for the zone specified in the configuration file.

**Note** For interacting with the database you can use `psql "postgresql://...."` where the string in quotes is the same as the one in the configuration file.

**Note** The server information's page is accessible at http://srv1.vpn.example.com inside the network, and only if the DNS has been configured well.
Note also that you can change the address and the port of the web server in the configuration file.
Furthermore note that `web_static_dir` must point to the `static` directory of this repository.

¹ You can directly use the docker image which starts both `wireguard-manager` and `dnsmasq`

## Adding a client to the network

- Generate a private and a public key for wireguard (`wg genkey` and `wg pubkey`), in the example `CLIENT_PRIVATE_KEY` and `CLIENT_PUBLIC_KEY`.
- Choose a name for the client, in the example `client1`.
- Add an entry in the `clients` table in the database.
  - Note that the name must be different from all the other clients and all the servers.
- Add an entry in the `connections` table in the database.
  - The address you set must be inside the network of the server the client connects to.
  - Note that a client can connect to at most one server.
- Generate the configuration file for the client.
  - If you are already in the network you can use the information's page of any server.
  - Otherwise you can use the CLI tool: `cargo run --bin gen-client -- client1`.
- Patch the client configuration file setting the private key.
- Install the configuration on the client:
  - Using `wg-quick`: put the configuration in `/etc/wireguard/wg0.conf` and start/enable the service `wg-quick@wg0`.
  - On Android: make a QR with the configuration file and scan it with the app (for example using `qrencode -t ansiutf8`).

**Note** The client will be accessible at http://client1.vpn.example.com.

## Using Docker

Build the binary in release mode using the `x86_64-unknown-linux-musl` target.

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --target x86_64-unknown-linux-musl --release
```

Then build the container for the server
```bash
cd docker
tar -czh . | docker build -t wireguard-manager -
```

Now you have a docker image named `wireguard-manager` with the server inside.

### Pre built image

In order to use it you may need to have `wireguard` installed on the host.

To run a new container with this image you can use:
```bash
docker run --cap-add NET_ADMIN \
    --sysctl=net.ipv6.conf.all.disable_ipv6=0 \
    --sysctl=net.ipv6.conf.all.forwarding=1 \
    -v /etc/wireguard-manager/config.yaml:/config.yaml \
    -e DOMAIN=YYYY \
    -p 0.0.0.0:XXXX:XXXX/udp \
    -p 0.0.0.0:53:53/tcp \
    -p 0.0.0.0:53:53/udp \
    --name wireguard-manager \
    --restart=unless-stopped \
    edomora97/wireguard-manager
```

**Note** You have to change `XXXX` to the port number of wireguard (set in the database) and `YYYY` to the `base_domain` set in `config.yaml`.

**Note** If you don't want to make this server an _authoritative DNS server_, remove the two lines that publish the port 53.

**Note** If you are not using IPv6 for the internal network you can remove the first `sysctl` rule and enable the forwarding only for IPv4 (`net.ipv4.ip_forward=1`).
