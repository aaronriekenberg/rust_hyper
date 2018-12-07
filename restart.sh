#!/bin/sh -x

CONFIG_FILE=config/$(hostname)-config.json

pkill rust_hyper

nohup ./target/release/rust_hyper $CONFIG_FILE 2>&1 | svlogd logs &
