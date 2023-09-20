#!/bin/bash
set -e
set -x

cargo build --release

for i in {15..16}; do
    server=web${i}
    work_dir=/etc/av1-cloud
    service_name="av1-cloud"
    bin_name="av1-cloud"

    # init environment
    ssh ${server} "mkdir -p ${work_dir}/configs || true"
    ssh ${server} "systemctl stop ${service_name} || true"

    # sync bin && configs
    scp target/release/${bin_name} ${server}:/usr/local/bin/${bin_name} >/dev/null 2>&1
    scp -r configs/* ${server}:${work_dir}/configs >/dev/null 2>&1

    # sync service
    scp ./scripts/${service_name}.service ${server}:/etc/systemd/system/${service_name}.service >/dev/null 2>&1
    ssh ${server} "systemctl daemon-reload"
    ssh ${server} "systemctl enable ${service_name}"
    ssh ${server} "systemctl restart ${service_name}"
done
