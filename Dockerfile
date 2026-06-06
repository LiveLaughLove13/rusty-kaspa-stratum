# ---------------------------------------- Chef image -------------------------------------------
  FROM rust:1.91-alpine AS chef
  # clang-dev (+ clang-static): librocksdb-sys/bindgen need libclang (was clang15-dev on Alpine 3.19).
  RUN apk --no-cache add \
    musl-dev \
    protobuf-dev \
    g++ \
    clang \
    clang-dev \
    clang-static \
    linux-headers \
    wasm-pack \
    openssl-dev \
  && CLANG_LIB_DIR="$(dirname "$(find /usr/lib -name 'libclang.so' -print -quit 2>/dev/null)")" \
  && test -n "$CLANG_LIB_DIR" \
  && ln -sf "$CLANG_LIB_DIR" /usr/lib/libclang-bindgen
  RUN cargo install cargo-chef --locked
  WORKDIR /app
  
  # ---------------------------------------- Planner image ----------------------------------------
  FROM chef AS planner
  COPY . .
  RUN cargo chef prepare --recipe-path recipe.json
  
  # ---------------------------------------- Builder image ----------------------------------------
  FROM chef AS builder
  COPY --from=planner /app/recipe.json recipe.json
  
  ENV RUSTFLAGS="-C target-feature=-crt-static" \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse" \
    LIBCLANG_PATH=/usr/lib/libclang-bindgen \
    LIBCLANG_STATIC_PATH=/usr/lib/libclang-bindgen \
    CARGO_BUILD_JOBS=4
  
  # Build dependencies - this is the caching Docker layer
  RUN cargo chef cook --release --recipe-path recipe.json -p kaspa-stratum-bridge --bin stratum-bridge
  
  COPY . .
  RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
      --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
      --mount=type=cache,id=cargo-target,target=/app/target,sharing=locked \
      cargo build --release --bin stratum-bridge \
      && cp /app/target/release/stratum-bridge /app/stratum-bridge   # <-- outside the mount, persists
  
  
  # ---------------------------------------- Runtime image ----------------------------------------
  FROM alpine AS runtime
  WORKDIR /app
  
  RUN apk --no-cache add \
    libgcc \
    libstdc++ \
    tini \
    ca-certificates \
    && addgroup -S kaspa \
    && adduser -S -G kaspa -h /home/kaspa -s /sbin/nologin kaspa \
    && mkdir -p /home/kaspa /app \
    && chown -R kaspa:kaspa /home/kaspa /app
  
  COPY --from=builder --chown=kaspa:kaspa /app/stratum-bridge .
  
  ENV HOME=/home/kaspa
  USER kaspa
  ENTRYPOINT [ "/sbin/tini", "--" ]
  CMD [ "/app/stratum-bridge" ]