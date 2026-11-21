#!/usr/bin/env bash
# Run on each machine before the demo. Checks that it is ready for its role.
#   ./deploy/preflight.sh consumer     # on the consumer VM
#   ./deploy/preflight.sh producer     # on the producer VM
set -uo pipefail
cd "$(dirname "$0")/.."
role="${1:-}"
[[ $role == producer || $role == consumer ]] || { sed -n '2,4p' "$0"; exit 2; }
# Load deploy/.env; variables already set on the command line win.
if [[ -f deploy/.env ]]; then _env=$(export -p); set -a; source deploy/.env; set +a; eval "$_env" 2>/dev/null || true; fi

fails=0
ok()   { echo "  ok    $*"; }
bad()  { echo "  FAIL  $*"; fails=$((fails+1)); }
note() { echo "  note  $*"; }
have() { command -v "$1" >/dev/null 2>&1; }
built() { [[ -x target/release/$1 || -x target/release/$1.exe ]]; }
port_open() { timeout 3 bash -c "exec 3<>/dev/tcp/$1/$2" 2>/dev/null; }
videos_in() { curl -fsS --max-time 5 "$1" 2>/dev/null | grep -o '"id":' | wc -l | tr -d ' '; }

echo "Preflight ($role)"

[[ -f deploy/.env ]] && ok "deploy/.env exists" || bad "deploy/.env missing (cp deploy/.env.example deploy/.env)"
have ffmpeg  && ok "ffmpeg installed"  || bad "ffmpeg not installed"
have ffprobe && ok "ffprobe installed" || bad "ffprobe not installed"
have curl    && ok "curl installed"    || bad "curl not installed"

if [[ $role == consumer ]]; then
  built consumer && ok "consumer is built (cargo build --release)" || bad "consumer not built: cargo build --release"
  built gui      && ok "gui is built"      || bad "gui not built: cargo build --release"
  gui="http://127.0.0.1:${GUI_PORT:-8080}/api/videos"
  if curl -fsS --max-time 5 "$gui" >/dev/null 2>&1; then
    ok "GUI answers on port ${GUI_PORT:-8080}"
    n=$(videos_in "$gui")
    [[ $n -eq 0 ]] && ok "no videos yet (clean start)" || note "$n video(s) already in the GUI; clear data/ first for a clean demo"
  else
    bad "GUI not answering: start ./deploy/run-gui.sh"
  fi
  port_open 127.0.0.1 "${CONSUMER_PORT:-50051}" \
    && ok "consumer is listening on port ${CONSUMER_PORT:-50051}" \
    || bad "consumer not listening: start C=2 Q=10 ./deploy/run-consumer.sh"
  note "the consumer's c and q can't be checked from here; start it as C=2 Q=10 for the first half"
else
  built producer && ok "producer is built (cargo build --release)" || bad "producer not built: cargo build --release"
  if [[ -z ${CONSUMER_HOST:-} ]]; then
    bad "CONSUMER_HOST not set in deploy/.env"
  else
    note "CONSUMER_HOST=${CONSUMER_HOST}"
    port_open "$CONSUMER_HOST" "${CONSUMER_PORT:-50051}" \
      && ok "can reach the consumer's gRPC port ${CONSUMER_PORT:-50051}" \
      || bad "cannot reach ${CONSUMER_HOST}:${CONSUMER_PORT:-50051} (wrong IP, consumer not running, firewall, or not on the same network/VPN)"
    gui="http://${CONSUMER_HOST}:${GUI_PORT:-8080}/api/videos"
    if curl -fsS --max-time 5 "$gui" >/dev/null 2>&1; then
      ok "can reach the GUI on port ${GUI_PORT:-8080}"
      n=$(videos_in "$gui")
      [[ $n -eq 0 ]] && ok "no videos yet (clean start)" || note "$n video(s) already in the GUI; ask the consumer side to clear data/ for a clean demo"
    else
      bad "cannot reach the GUI at $gui (the e2e tests need it)"
    fi
  fi
fi

echo
if [[ $fails -eq 0 ]]; then echo "READY"; else echo "$fails problem(s) above"; exit 1; fi
