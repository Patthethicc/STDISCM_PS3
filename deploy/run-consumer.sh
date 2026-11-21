#!/usr/bin/env bash
# Consumer VM: receives uploads over gRPC, queues them, and workers save + process them.
#   ./deploy/run-consumer.sh                       # uses deploy/.env
#   C=1 Q=2 WORKER_DELAY_MS=3000 ./deploy/run-consumer.sh   # small queue + slow workers (drop demo)
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f deploy/.env ]] && set -a && source deploy/.env && set +a
: "${C:?set C}" "${Q:?set Q}"
mkdir -p "${DATA_DIR:=data}"/{uploads,previews,thumbnails,tmp}

exec cargo run --release -q -p consumer -- \
  -c "$C" -q "$Q" \
  --listen "0.0.0.0:${CONSUMER_PORT:-50051}" \
  --data-dir "$DATA_DIR" \
  --worker-delay-ms "${WORKER_DELAY_MS:-0}"
