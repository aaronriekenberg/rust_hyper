FROM rust:latest

WORKDIR /usr/src/rust_hyper
COPY . .

RUN cargo build -v --release
