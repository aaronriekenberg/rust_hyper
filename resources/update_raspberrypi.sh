#!/bin/bash -x

cargo build --target=armv7-unknown-linux-gnueabihf --release

sha256sum target/armv7-unknown-linux-gnueabihf/release/rust_hyper

ssh pi@raspberrypi 'killall rust_hyper'

scp target/armv7-unknown-linux-gnueabihf/release/rust_hyper pi@raspberrypi:rust_hyper_run/
