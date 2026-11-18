# OpenWX Relay container. Build from the repository root:
#   docker build -f server/Dockerfile -t openwx-relay .
# First run (writes ./config/relay.toml; choose "A shared server"):
#   docker run --rm -it -v "$PWD/config:/etc/openwx" openwx-relay setup
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY relay relay
# Build the relay on its own: the desktop app and its map engine are not needed on a server.
RUN sed -i -e 's/^members = .*/members = ["relay"]/' -e 's/^default-members = .*/default-members = ["relay"]/' Cargo.toml \
 && cargo build --release -p openwx-relay

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --home /var/lib/openwx openwx && mkdir -p /etc/openwx /var/lib/openwx && chown openwx /etc/openwx /var/lib/openwx
COPY --from=build /src/target/release/openwx-relay /usr/local/bin/openwx-relay
USER openwx
EXPOSE 8737
ENV OPENWX_RELAY_CONFIG=/etc/openwx/relay.toml
ENTRYPOINT ["/usr/local/bin/openwx-relay"]
CMD ["serve"]
