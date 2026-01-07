FROM rust:1.92-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    libsasl2-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./

COPY src ./src
RUN cargo build --release --bin front


FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y ca-certificates bash \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/front /usr/local/bin/front

EXPOSE 8080

HEALTHCHECK --interval=5s --timeout=2s --retries=3 \
  CMD bash -c 'echo > /dev/tcp/127.0.0.1/8080' || exit 1

CMD ["front"]
