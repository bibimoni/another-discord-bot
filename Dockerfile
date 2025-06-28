FROM rust AS builder
WORKDIR /app

RUN echo "fn main() {}" > dummy.rs

COPY Cargo.lock /app
COPY Cargo.toml /app
RUN sed -i 's#src/main.rs#dummy.rs#' Cargo.toml

RUN cargo build --release

RUN sed -i 's#dummy.rs#src/main.rs#' Cargo.toml
COPY . .
RUN cargo build --release

RUN strip target/release/codeforces-trainer-bot
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
 && apt-get install -y --no-install-recommends libssl3 ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY .env /app/.env
COPY --from=builder /app/target/release/codeforces-trainer-bot /usr/local/bin/

CMD ["codeforces-trainer-bot"]


