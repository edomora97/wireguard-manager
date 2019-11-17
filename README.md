# wireguard-manager

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

**Note**: you have to change `XXXX` to the port number of wireguard (set in the database) and `YYYY` to the `base_domain` set in `config.yaml`.
