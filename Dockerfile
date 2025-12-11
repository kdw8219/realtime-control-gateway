FROM rust:1.91-slim as builder

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    clang \
    llvm \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/realtime-control-gateway
COPY . .
RUN cargo build --release

FROM rust:1.91-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# copy built binary
COPY --from=builder /usr/src/realtime-control-gateway/target/release/realtime-control-gateway /usr/local/bin/realtime-control-gateway
# copy entrypoint script
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 0
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
