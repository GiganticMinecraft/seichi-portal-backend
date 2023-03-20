# syntax=docker/dockerfile:1.4
FROM rust:1.68-slim AS build-env

WORKDIR /app

COPY --link . .

RUN --mount=target=. \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/cache \
    --mount=type=cache,target=/usr/local/cargo/registry/index \
    cargo fetch --manifest-path Cargo.toml

RUN rustup target add x86_64-unknown-linux-musl && \
    apt update && apt-get install -y musl-tools build-essential

# TODO: cargo build の --out-dir オプションが stable に落ちてきたらコメントの内容に置き換える
# RUN --mount=type=cache,target=/usr/local/cargo/git/db \
#     --mount=type=cache,target=/usr/local/cargo/registry/cache \
#     --mount=type=cache,target=/usr/local/cargo/registry/index \
#     --mount=type=cache,target=/app/server/target \
#     cargo build --release --out-dir .

RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/cache \
    --mount=type=cache,target=/usr/local/cargo/registry/index \
    --mount=type=cache,target=target \
    cargo build --release && \
    cp target/x86_64-unknown-linux-musl/release/entrypoint /seichi-portal-backend

FROM gcr.io/distroless/cc
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend
# TODO: cargo build の --out-dir オプションが stable に落ちてきたらコメントの内容に置き換える
# COPY --from=build-env --link /app/server/entrypoint /seichi-portal-backend
COPY --from=build-env --link /seichi-portal-backend /seichi-portal-backend
CMD ["/seichi-portal-backend"]
