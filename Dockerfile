FROM debian:bookworm-slim AS builder

RUN apt-get update && apt-get install -y \
  curl \
  build-essential \
  pkg-config \
  libssl-dev \
  ca-certificates \
  && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y \
  libssl-dev \
  ca-certificates \
  libncursesw5-dev \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/load-balancer /app/load-balancer

ENTRYPOINT [ "/app/load-balancer" ]