FROM lukemathwalker/cargo-chef:latest-rust-bullseye AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

# We do not need the Rust toolchain to run the binary!
FROM debian:bullseye-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/mail-list-rss /usr/local/bin
CMD [ "/usr/local/bin/mail-list-rss" ]