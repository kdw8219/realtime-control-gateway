#!/bin/sh
set -e

: "${self_ip:=localhost}"
: "${self_port:=8080}"
: "${to_ip:=localhost}"
: "${to_port:=50051}"

mkdir -p /app/config

cat > /app/config/default.toml <<EOF
[websocket_server]
self_ip = "${self_ip}"
self_port = "${self_port}"

[grpc_client]
to_ip = "${to_ip}"
to_port = "${to_port}"
EOF

exec /usr/local/bin/realtime-control-gateway
