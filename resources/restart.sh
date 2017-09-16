#!/bin/sh -x

KILL_CMD=killall
CONFIG_FILE=raspberrypi-config.yml

$KILL_CMD rust_hyper

if [ -f output ]; then
  mv output output.1
fi

nohup ./rust_hyper $CONFIG_FILE > output 2>&1 &
