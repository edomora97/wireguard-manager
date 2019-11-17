FROM alpine:latest
RUN apk add wireguard-tools dnsmasq
RUN mkdir -p /tmp/hosts
ENV RUST_LOG=debug
CMD dnsmasq --addn-hosts=/tmp/hosts/wg-hosts.conf --log-facility=- --log-queries && /wireguard-manager

COPY wireguard-manager /wireguard-manager
