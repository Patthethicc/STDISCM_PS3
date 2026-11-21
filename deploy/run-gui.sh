#!/usr/bin/env bash
# Consumer VM: web GUI that shows the uploaded videos. Runs next to the consumer and reads
# the same data folder, so it keeps serving videos even if the consumer process stops.
#   ./deploy/run-gui.sh      -> open http://CONSUMER_HOST:GUI_PORT
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f deploy/.env ]] && set -a && source deploy/.env && set +a
mkdir -p "${DATA_DIR:=data}"

exec cargo run --release -q -p gui -- \
  --data-dir "$DATA_DIR" \
  --bind "0.0.0.0:${GUI_PORT:-8080}"
