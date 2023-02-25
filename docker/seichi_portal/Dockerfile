FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /work

FROM chef AS planner
COPY ../../server /work
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /work/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY ../../server /work
RUN cargo build

ENTRYPOINT ["cargo"]
CMD ["run", "-p", "app"]
