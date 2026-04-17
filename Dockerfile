# syntax=docker/dockerfile:1.4
# Builder: Debian (glibc) so bindgen/librocksdb-sys can dlopen libclang. Static musl binary via
# kaspanet `musl-toolchain` (CI parity). Quoted heredoc avoids Docker RUN `$` expansion eating RUSTFLAGS.
# ---------------------------------------- Builder image ----------------------------------------
FROM rust:1.90-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    protobuf-compiler \
    pkg-config \
    libssl-dev \
    cmake \
    ninja-build \
    zlib1g-dev \
    curl \
    ca-certificates \
    zstd \
    clang \
    libclang-dev \
  && rm -rf /var/lib/apt/lists/*

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"
ENV GITHUB_WORKSPACE=/usr/src/rustbridge
# buildx/GitHub runners are RAM-tight while compiling rocksdb; avoid OOM (often surfaces as exit 101).
ENV CARGO_BUILD_JOBS=4
ENV CMAKE_BUILD_PARALLEL_LEVEL=4

WORKDIR /usr/src/rustbridge

COPY musl-toolchain ./musl-toolchain
COPY Cargo.toml Cargo.lock ./
COPY .cargo/config.toml .cargo/config.toml
COPY bridge ./bridge
COPY bridge-tauri/src-tauri ./bridge-tauri/src-tauri

RUN <<'EOS'
#!/usr/bin/env bash
set -euo pipefail
source musl-toolchain/build.sh
cd "${GITHUB_WORKSPACE}"
export RUSTFLAGS="${RUSTFLAGS} -C link-arg=-Wl,--allow-multiple-definition"
cargo build --locked --release --target x86_64-unknown-linux-musl --features rkstratum_cpu_miner -p kaspa-stratum-bridge
cp target/x86_64-unknown-linux-musl/release/stratum-bridge /stratum-bridge
EOS

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
