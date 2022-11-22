FROM rust:1.65.0

RUN apt-get update && apt-get install -y \
    default-mysql-client \
    openssl \
    && apt-get clean
RUN cargo install diesel_cli --no-default-features --features mysql --force

WORKDIR /work
COPY ../ /work
