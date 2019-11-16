FROM alpine:latest
RUN apk add wireguard-tools
ENV RUST_LOG=debug
ENTRYPOINT /wireguard-manager

COPY wireguard-manager /wireguard-manager
