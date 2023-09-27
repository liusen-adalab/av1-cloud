#!/bin/bash
server=web15
if [ -n "$1" ]; then
    server=web$1
fi
set -x
ssh $server 'journalctl --output cat  -f -u av1-cloud.service'
