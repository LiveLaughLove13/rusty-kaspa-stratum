FROM rust:1.92-alpine AS builder

RUN apk add --no-cache build-base protoc

WORKDIR /usr/src/rustbridge

COPY Cargo.toml Cargo.lock ./

# Create a dummy src/main.rs to build only the dependencies.
# This avoids rebuilding all dependencies when only the source code changes.
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# Dependencies are now cached, we can remove the dummy source and build the real one
RUN rm -f target/release/deps/rustbridge*
COPY src ./src

RUN cargo build --release

FROM alpine:latest

WORKDIR /app

LABEL org.opencontainers.image.title="Kaspa Rust Stratum Bridge" \
    org.opencontainers.image.description="A high-performance Rust implementation of the Kaspa Stratum Bridge, providing seamless mining pool connectivity for Kaspa ASIC miners." \
    org.opencontainers.image.url="https://github.com/LiveLaughLove13/rusty-kaspa-stratum" \
    org.opencontainers.image.source="https://github.com/LiveLaughLove13/rusty-kaspa-stratum" \
    org.opencontainers.image.vendor="Kluster" \
    org.opencontainers.image.licenses="ISC"

# Copy the binary from the builder stage
COPY --from=builder /usr/src/rustbridge/target/release/rustbridge .
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

# Set the entrypoint to run the bridge
ENTRYPOINT ["./rustbridge"]
