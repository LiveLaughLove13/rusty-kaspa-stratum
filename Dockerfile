# ---------------------------------------- Builder image ----------------------------------------
FROM rust:1.90-alpine AS builder

RUN apk --no-cache add \
  musl-dev \
  protobuf-dev \
  g++ \
  clang15-dev \
  linux-headers \
  openssl-dev \
  pkgconfig

ENV RUSTFLAGS="-C target-feature=-crt-static" \
  CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"

WORKDIR /usr/src/rustbridge

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy src/main.rs to build only the dependencies.
# This avoids rebuilding all dependencies when only the source code changes.
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies - this layer will be cached if Cargo.toml/Cargo.lock don't change
RUN cargo build --release --features rkstratum_cpu_miner --bin stratum-bridge

# Dependencies are now cached, we can remove the dummy source and build the real one
RUN rm -f target/release/deps/stratum_bridge* target/release/deps/kaspa_stratum_bridge*

# Copy the actual source code
COPY src ./src
COPY config.yaml ./

# Build the actual binary
RUN cargo build --release --features rkstratum_cpu_miner --bin stratum-bridge \
  && cp target/release/stratum-bridge /stratum-bridge

# ---------------------------------------- Runtime image ----------------------------------------
FROM alpine AS runtime

WORKDIR /app

LABEL org.opencontainers.image.title="Kaspa Rust Stratum Bridge" \
    org.opencontainers.image.description="A high-performance Rust implementation of the Kaspa Stratum Bridge, providing seamless mining pool connectivity for Kaspa ASIC miners." \
    org.opencontainers.image.url="https://github.com/LiveLaughLove13/rusty-kaspa-stratum" \
    org.opencontainers.image.source="https://github.com/LiveLaughLove13/rusty-kaspa-stratum" \
    org.opencontainers.image.vendor="Kluster" \
    org.opencontainers.image.licenses="ISC"

RUN apk --no-cache add \
  libgcc \
  libstdc++ \
  tini \
  ca-certificates \
  && addgroup -S kaspa \
  && adduser -S -G kaspa -h /home/kaspa -s /sbin/nologin kaspa \
  && mkdir -p /home/kaspa /app \
  && chown -R kaspa:kaspa /home/kaspa /app

# Copy the binary from the builder stage
COPY --from=builder --chown=kaspa:kaspa /stratum-bridge .
COPY --from=builder --chown=kaspa:kaspa /usr/src/rustbridge/config.yaml ./config.yaml
COPY LICENSE .

# Expose the default stratum and prometheus ports from the config
# Stratum ports
EXPOSE 5555
EXPOSE 5556
EXPOSE 5557
EXPOSE 5558
# Prometheus ports
EXPOSE 2114
EXPOSE 2115
EXPOSE 2116
EXPOSE 2117

ENV HOME=/home/kaspa
USER kaspa

ENTRYPOINT [ "/sbin/tini", "--" ]
CMD [ "./stratum-bridge", "--config", "/app/config.yaml", "--node-mode", "external" ]

