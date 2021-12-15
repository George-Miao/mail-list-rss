FROM rustlang/rust:nightly as planner
WORKDIR /app

RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM rustlang/rust:nightly as cacher
WORKDIR /app

RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rustlang/rust:nightly as builder
WORKDIR /app

COPY . .
COPY --from=cacher /app/target target
RUN cargo build --release

FROM rustlang/rust:nightly as runtime
WORKDIR /app
COPY --from=builder /app/target/release/mail-list-rss .
