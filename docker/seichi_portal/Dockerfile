FROM rust:1.65.0

RUN apt update -y && apt upgrade -y
RUN apt install default-mysql-client -y  && apt install openssl -y
RUN cargo install diesel_cli --no-default-features --features mysql --force

WORKDIR /work
COPY ../ /work
