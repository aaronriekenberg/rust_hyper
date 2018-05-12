#!/bin/sh -x

CONFIG_FILE=$(hostname)-config.yml

pkill rust_hyper

nohup ./rust_hyper $CONFIG_FILE 2>&1 | svlogd logs &
