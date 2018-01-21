#!/bin/sh -x

CONFIG_FILE=raspberrypi-config.yml

killall rust_hyper

nohup ./rust_hyper $CONFIG_FILE 2>&1 | svlogd logs &
