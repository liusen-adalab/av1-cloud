#!/bin/bash
set -e

# stop
for i in {15..16}; do
    server=web${i}
    ssh ${server} "systemctl stop av1-cloud || true"
done

# clear cache
for i in {70..75}; do
    redis-cli -c -h 10.0.10.${i} flushall >/dev/null 2>&1
done

# clear  session
redis-cli -n 1 -h 10.0.10.59 flushdb

# clear db
p_cmd="drop table if exists __diesel_schema_migrations, employees, sys_files, sys_files_id_seq, user_files, users;"
PGPASSWORD=postgres psql -h 10.0.10.3 -p 30020 -U postgres -d av1_cloud -c "${p_cmd}"
/home/sen/.cargo/bin/diesel migration run --database-url postgres://postgres:postgres@10.0.10.85:5433/av1_cloud

# clear fs
ssh web15 "rm -rf /storage/dev-av1_cloud-root/"

# restart
for i in {15..16}; do
    server=web${i}
    ssh ${server} "systemctl restart av1-cloud"
done
