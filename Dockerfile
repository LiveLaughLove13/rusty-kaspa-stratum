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

# Workspace root + bridge crate (matches kaspanet/rusty-kaspa `bridge/` layout).
COPY Cargo.toml Cargo.lock ./
COPY bridge ./bridge

RUN cargo build --release -p kaspa-stratum-bridge --features rkstratum_cpu_miner \
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

COPY --from=builder --chown=kaspa:kaspa /stratum-bridge .
COPY --from=builder --chown=kaspa:kaspa /usr/src/rustbridge/bridge/config.yaml ./config.yaml
COPY LICENSE .

EXPOSE 5555
EXPOSE 5556
EXPOSE 5557
EXPOSE 5558
EXPOSE 2114
EXPOSE 2115
EXPOSE 2116
EXPOSE 2117

ENV HOME=/home/kaspa
USER kaspa

ENTRYPOINT [ "/sbin/tini", "--" ]
CMD [ "./stratum-bridge", "--config", "/app/config.yaml", "--node-mode", "external" ]
