SCRIPTPATH="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"
echo "script location in $SCRIPTPATH"

######################## keydb ######################################
keydb_compose=${SCRIPTPATH}/keydb-compose.yml

# generate docker-compose file
cat <<EOF > ${keydb_compose}
version: '3'
services:
EOF

keydb_worker_dir=$SCRIPTPATH/../target/dev-keydb
for i in {1..6}; do
    config_path=${keydb_worker_dir}/keydb${i}.conf
    data_path=${keydb_worker_dir}/data/keydb${i}
    # append docker-compose container
    cat <<EOF >> ${keydb_compose}
  keydb${i}:
    image: eqalpha/keydb
    volumes:
      - ${data_path}:/data
      - ${config_path}:/etc/keydb/keydb.conf
    network_mode: "host"
EOF
done
docker-compose --file ${keydb_compose} down || true

sudo rm -rf ${keydb_worker_dir}
mkdir -p ${keydb_worker_dir}

for i in {1..6}; do
    port=4637${i}
    config_path=${keydb_worker_dir}/keydb${i}.conf

    # generate config file for keydb node
    cat <<EOF > ${config_path}
port ${port}
cluster-enabled yes
cluster-config-file nodes.conf
cluster-node-timeout 5000
appendonly yes
protected-mode no
loglevel notice
save 900 1
save 300 10
bind 0.0.0.0
save 60 10000
EOF
done


# compose up
docker-compose --file ${keydb_compose} up -d
rm ${keydb_compose}

sleep 0.5

create keydb cluster
docker run --network=host -it --rm eqalpha/keydb keydb-cli  --cluster create \
127.0.0.1:46371 \
127.0.0.1:46372 \
127.0.0.1:46373 \
127.0.0.1:46374 \
127.0.0.1:46375 \
127.0.0.1:46376 \
--cluster-replicas 1

######################## session store #########################################
docker rm -f df-session-store
docker run -d --name df-session-store --network=host --ulimit memlock=-1 docker.dragonflydb.io/dragonflydb/dragonfly --bind "0.0.0.0" --port 16379

######################## postgres #########################################
# TODO: use yugabyte
docker rm -f pg-av1-cloud
docker run -d \
    -e POSTGRES_USER=postgres \
    -e POSTGRES_PASSWORD=postgres \
    -e POSTGRES_DB=av1-cloud \
    -p 54333:5432 \
    --name pg-av1-cloud \
    postgres -N 20

sleep 1.5

echo "resetting pg migrations"
diesel database reset --database-url postgres://postgres:postgres@127.0.0.1:54333/av1-cloud