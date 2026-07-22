FROM rust:1 AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY protocol ./protocol
COPY server ./server

RUN cargo build --release --bin dreg_server

FROM debian:bookworm-slim AS runtime

RUN useradd --create-home --shell /usr/sbin/nologin app

COPY --from=builder /app/target/release/dreg_server /usr/local/bin/dreg_server

USER app
EXPOSE 6767

CMD ["dreg_server"]
