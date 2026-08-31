# syntax=docker/dockerfile:1
FROM rust:1.98-bookworm AS build

WORKDIR /src
COPY . .
RUN cargo build --release --locked --package redmine-importer

FROM ubuntu:26.04
LABEL org.opencontainers.image.source=https://github.com/GiganticMinecraft/seichi-portal-backend

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /etc/seichi-portal

COPY --from=build /src/target/release/redmine-importer /redmine-importer
COPY server/redmine-importer/config/redmine-import.json /etc/seichi-portal/redmine-import.json

ENV REDMINE_IMPORT_CONFIG=/etc/seichi-portal/redmine-import.json

USER ubuntu

ENTRYPOINT ["/redmine-importer"]
