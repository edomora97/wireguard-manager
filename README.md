# wireguard-manager

## Docker build

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
In order to use it you may need to have `wireguard` installed on the host.

To run a new container with this image you can use:
```bash
docker run --cap-add NET_ADMIN --sysctl=net.ipv6.conf.all.disable_ipv6=0 -v $(realpath config.yaml):/config.yaml wireguard-manager
```

**Note**: you may need to add the port binding from the container to the host, the exact port numbers are specified in the `config.yaml` file.
