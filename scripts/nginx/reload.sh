set -x
set -e

for i in {13..14}; do
    server=web${i}
    scp scripts/nginx/nginx.conf ${server}:/etc/nginx/
    ssh ${server} "nginx -s reload"
done
