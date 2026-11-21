#!/usr/bin/env bash
# Producer VM: P threads, each uploading every video in its own folder to the consumer.
#   ./deploy/run-producer.sh                                  # uses P and FOLDERS from deploy/.env
#   ./deploy/run-producer.sh videos/a videos/b                # P = number of folders given
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f deploy/.env ]] && set -a && source deploy/.env && set +a
: "${CONSUMER_HOST:?set CONSUMER_HOST in deploy/.env}"

if [[ $# -gt 0 ]]; then
  folders=("$@")
else
  read -r -a folders <<< "${FOLDERS:?set FOLDERS in deploy/.env}"
fi
P=${#folders[@]}

args=()
for f in "${folders[@]}"; do args+=(--folder "$f"); done

exec cargo run --release -q -p producer -- -p "$P" "${args[@]}" \
  --consumer "${CONSUMER_HOST}:${CONSUMER_PORT:-50051}"
