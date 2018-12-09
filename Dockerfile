FROM rust:latest

WORKDIR /usr/src/rust_hyper
COPY . .

RUN cargo build -v --release

EXPOSE 8000
ENTRYPOINT ./target/release/rust_hyper ./config/config.json
