# syntax=docker/dockerfile:1.4
FROM rust:1.68-slim AS build-env

WORKDIR /app

RUN --mount=target=. \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/cache \
    --mount=type=cache,target=/usr/local/cargo/registry/index \
    cargo fetch --manifest-path server/Cargo.toml

COPY --link . .

WORKDIR /app/server

# TODO: cargo build の --out-dir オプションが stable に落ちてきたらコメントの内容に置き換える
# RUN --mount=type=cache,target=/usr/local/cargo/git/db \
#     --mount=type=cache,target=/usr/local/cargo/registry/cache \
#     --mount=type=cache,target=/usr/local/cargo/registry/index \
#     --mount=type=cache,target=/app/server/target \
#     cargo build --release --out-dir .

RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/cache \
    --mount=type=cache,target=/usr/local/cargo/registry/index \
    --mount=type=cache,target=/app/server/target \
    cargo build --release && \
    cp /app/server/target/release/entrypoint /seichi-portal-backend

FROM gcr.io/distroless/cc
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend
# TODO: cargo build の --out-dir オプションが stable に落ちてきたらコメントの内容に置き換える
# COPY --from=build-env --link /app/server/entrypoint /seichi-portal-backend
COPY --from=build-env --link /seichi-portal-backend /seichi-portal-backend
CMD ["/seichi-portal-backend"]
