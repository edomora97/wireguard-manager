FROM alpine:latest
RUN apk add wireguard-tools dnsmasq
RUN mkdir -p /var/run
ENV RUST_LOG="wireguard_manager=debug"
CMD dnsmasq --addn-hosts=/var/run/wg-hosts.conf --log-facility=- --log-queries --auth-server=$DOMAIN --auth-zone=$DOMAIN && /wireguard-manager

COPY wireguard-manager /wireguard-manager
COPY static /static
