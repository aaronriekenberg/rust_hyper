#!/bin/sh

pgrep rust_hyper > /dev/null 2>&1
if [ $? -eq 1 ]; then
  cd ~/rust_hyper_run
  ./restart.sh > /dev/null 2>&1
fi
