#!/usr/bin/env bash
# Consumer VM: receives uploads over gRPC, queues them, and workers save + process them.
#   ./deploy/run-consumer.sh                       # uses deploy/.env
#   C=1 Q=1 ./deploy/run-consumer.sh   # tiny queue (drop demo with 8 producers)
set -euo pipefail
cd "$(dirname "$0")/.."
# Load deploy/.env; variables already set on the command line (e.g. Q=2 ./...) win.
if [[ -f deploy/.env ]]; then _env=$(export -p); set -a; source deploy/.env; set +a; eval "$_env" 2>/dev/null || true; fi
: "${C:?set C}" "${Q:?set Q}"
mkdir -p "${DATA_DIR:=data}"/{uploads,previews,thumbnails,tmp}

args=(-c "$C" -q "$Q" --bind "0.0.0.0:${CONSUMER_PORT:-50051}" --data-dir "$DATA_DIR")

exec cargo run --release -q -p consumer -- "${args[@]}"
