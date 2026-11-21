#!/usr/bin/env bash
# End-to-end tests for the doc's verification table. Run on the PRODUCER VM (needs ffmpeg + curl):
#
#   ./tests/e2e/e2e.sh <scenario>
#
# Each scenario says which consumer settings it needs; start the consumer with them first
# (on the consumer VM), together with ./deploy/run-gui.sh.
#
#   multi          p=3 with 3 folders: every video ends up in the folder and the GUI
#                  consumer: normal, e.g. C=2 Q=10
#   queue-full     small queue + slow workers: extra videos dropped, producers log QUEUE_FULL
#                  consumer: C=1 Q=2 WORKER_DELAY_MS=3000
#   retry          re-upload the videos dropped by queue-full: accepted, NOT flagged duplicate
#                  consumer: same as queue-full (run right after it)
#   duplicate      same video in two folders, uploaded at once: one saved, the other DUPLICATE
#   short-corrupt  a 5 s video previews fine; a corrupt file errors and workers keep running
#   compression    sizes before/after compression for every video in the GUI
#   gui            prints the manual hover/click checks
#   all            multi, duplicate, short-corrupt, compression (one consumer setting)
#
# Test videos are generated into tests/e2e/.work with a unique run ID, so reruns don't collide
# with earlier uploads as duplicates.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
[[ -f deploy/.env ]] && set -a && source deploy/.env && set +a
: "${CONSUMER_HOST:?set CONSUMER_HOST in deploy/.env}"
API="http://${CONSUMER_HOST}:${GUI_PORT:-8080}/api/videos"
WORK="tests/e2e/.work"
RUN_ID="$(date +%s)-$$"
PASS=0; FAIL=0

pass() { PASS=$((PASS+1)); echo "PASS  $*"; }
fail() { FAIL=$((FAIL+1)); echo "FAIL  $*"; }
check() { local name=$1; shift; if "$@"; then pass "$name"; else fail "$name"; fi; }
info() { echo "      $*"; }

catalog() { curl -fsS --max-time 5 "$API" 2>/dev/null || echo "[]"; }
count_videos() { catalog | grep -o '"id":' | wc -l | tr -d ' '; }
in_catalog() { catalog | grep -q "\"filename\":\"$1\""; }

# make_video <path> <seconds> <tone Hz>; unique bytes per run via metadata
make_video() {
  mkdir -p "$(dirname "$1")"
  ffmpeg -loglevel error -y -f lavfi -i "testsrc2=size=320x240:rate=25" -f lavfi -i "sine=frequency=$3" \
    -t "$2" -c:v libx264 -pix_fmt yuv420p -c:a aac -metadata comment="e2e-$RUN_ID-$1" "$1"
}

# producer <folder...>  -> output in $WORK/producer.log
producer() {
  mkdir -p "$WORK"
  ./deploy/run-producer.sh "$@" 2>&1 | tee "$WORK/producer.log"
}
n_label() { grep -c ": $1" "$WORK/producer.log" || true; }

# wait_for <count> <seconds>: until the GUI lists at least <count> videos
wait_for() {
  local want=$1 secs=$2 i
  for ((i = 0; i < secs; i++)); do
    [[ $(count_videos) -ge $want ]] && return 0
    sleep 1
  done
  return 1
}

# wait until the GUI count stops changing for 10 s (queue drained)
wait_idle() {
  local last=-1 same=0 now
  info "waiting for the consumer to finish its queue..."
  while ((same < 10)); do
    now=$(count_videos)
    if [[ $now == "$last" ]]; then same=$((same+1)); else same=0; last=$now; fi
    sleep 1
  done
}

reachable() {
  if ! curl -fsS --max-time 5 "$API" >/dev/null; then
    echo "Can't reach the GUI at $API. Start ./deploy/run-consumer.sh and ./deploy/run-gui.sh on the consumer VM."
    exit 2
  fi
}

# ---------------------------------------------------------------- scenarios

multi() {
  echo "-- p=3, three folders, 3 videos each"
  local d="$WORK/multi-$RUN_ID" before
  for f in 1 2 3; do for v in 1 2 3; do make_video "$d/p$f/multi_${f}_${v}.mp4" 12 $((200 + f*100 + v*10)); done; done
  before=$(count_videos)
  producer "$d/p1" "$d/p2" "$d/p3"
  check "all 9 uploads ACCEPTED" [ "$(n_label ACCEPTED)" -eq 9 ]
  check "all 9 appear in the GUI (folder + catalog)" wait_for $((before + 9)) 180
}

queue_full() {
  echo "-- small queue with slow workers (consumer: C=1 Q=2 WORKER_DELAY_MS=3000)"
  local d="$WORK/qfull-$RUN_ID"
  for f in 1 2 3; do for v in 1 2 3 4; do make_video "$d/p$f/qfull_${f}_${v}.mp4" 6 $((900 + f*100 + v*10)); done; done
  producer "$d/p1" "$d/p2" "$d/p3"
  local full; full=$(n_label QUEUE_FULL)
  check "some uploads dropped with QUEUE_FULL ($full of 12)" [ "$full" -ge 1 ]
  check "some uploads still ACCEPTED" [ "$(n_label ACCEPTED)" -ge 1 ]
  check "nothing flagged DUPLICATE" [ "$(n_label DUPLICATE)" -eq 0 ]
  # remember dropped files for the retry scenario
  mkdir -p "$WORK/dropped"; rm -f "$WORK/dropped"/*
  grep ": QUEUE_FULL" "$WORK/producer.log" | sed -E 's/^\[producer-[0-9]+\] ([^:]+): .*/\1/' | while read -r name; do
    find "$d" -name "$name" -exec cp {} "$WORK/dropped/" \;
  done
  info "dropped files saved to $WORK/dropped for: ./tests/e2e/e2e.sh retry"
}

retry() {
  echo "-- retry the videos dropped by queue-full"
  local n; n=$(ls "$WORK/dropped" 2>/dev/null | wc -l | tr -d ' ')
  [[ $n -gt 0 ]] || { echo "No dropped videos saved. Run queue-full first."; exit 2; }
  wait_idle
  # one at a time so the small queue isn't flooded again
  local ok=0 dup=0 f
  for f in "$WORK/dropped"/*; do
    mkdir -p "$WORK/one"; rm -f "$WORK/one"/*; cp "$f" "$WORK/one/"
    producer "$WORK/one" >/dev/null
    [[ $(n_label ACCEPTED) -eq 1 ]] && ok=$((ok+1))
    [[ $(n_label DUPLICATE) -eq 1 ]] && dup=$((dup+1))
    sleep 4
  done
  check "all $n previously dropped videos ACCEPTED on retry ($ok)" [ "$ok" -eq "$n" ]
  check "none flagged as DUPLICATE" [ "$dup" -eq 0 ]
}

duplicate() {
  echo "-- same video in two folders, uploaded at the same time"
  local d="$WORK/dup-$RUN_ID"
  make_video "$d/a/same_video.mp4" 8 777
  mkdir -p "$d/b"; cp "$d/a/same_video.mp4" "$d/b/same_video.mp4"
  producer "$d/a" "$d/b"
  check "exactly one ACCEPTED" [ "$(n_label ACCEPTED)" -eq 1 ]
  check "exactly one DUPLICATE" [ "$(n_label DUPLICATE)" -eq 1 ]
  sleep 5
  check "only one copy in the GUI" [ "$(catalog | grep -o '"filename":"same_video.mp4"' | wc -l | tr -d ' ')" -le 1 ]
}

short_corrupt() {
  echo "-- a 5-second video, a corrupt file, then a normal video"
  local d="$WORK/sc-$RUN_ID" before
  make_video "$d/p1/short_5s.mp4" 5 555
  head -c 300000 /dev/urandom > "$d/p1/zz_corrupt.mp4"
  make_video "$d/p2/after_corrupt.mp4" 12 666
  before=$(count_videos)
  producer "$d/p1" "$d/p2"
  check "short video appears in the GUI" bash -c "for i in \$(seq 90); do curl -fsS '$API' | grep -q '\"filename\":\"short_5s.mp4\"' && exit 0; sleep 1; done; exit 1"
  check "short video has a preview" bash -c "curl -fsS '$API' | grep -o '{[^{}]*\"filename\":\"short_5s.mp4\"[^{}]*}' | grep -q '\"preview_path\":\"'"
  check "video after the corrupt one still processed (workers kept running)" bash -c "for i in \$(seq 90); do curl -fsS '$API' | grep -q '\"filename\":\"after_corrupt.mp4\"' && exit 0; sleep 1; done; exit 1"
  check "corrupt file NOT in the GUI" bash -c "! curl -fsS '$API' | grep -q 'zz_corrupt.mp4'"
  info "Also show the consumer terminal: it should log an error for zz_corrupt.mp4 and keep going."
}

compression() {
  echo "-- size before and after compression"
  local rows; rows=$(catalog | grep -o '"filename":"[^"]*"\|"original_size":[0-9]*\|"compressed_size":[0-9null]*' | paste - - - 2>/dev/null)
  if [[ -z $rows ]]; then fail "no videos in the GUI yet"; return; fi
  local smaller=0 total=0
  while IFS=$'\t' read -r f o c; do
    f=${f#*:}; o=${o#*:}; c=${c#*:}
    total=$((total+1))
    if [[ $c != null && $c -lt $o ]]; then smaller=$((smaller+1)); fi
    printf '      %-40s %10s -> %s bytes\n' "${f//\"/}" "$o" "$c"
  done <<< "$rows"
  check "compression visible ($smaller of $total smaller after)" [ "$smaller" -ge 1 ]
}

gui() {
  cat <<MSG
Manual GUI checks at http://${CONSUMER_HOST}:${GUI_PORT:-8080}
  [ ] Hover a video: its first 10 seconds play from the start; moving the mouse away stops it
  [ ] Hover the 5-second video: the preview plays fine
  [ ] Click a video: the full video plays in the player; Esc closes it
  [ ] Upload while the page is open: the new video appears within ~3 s without reloading
MSG
}

# ---------------------------------------------------------------- main
case "${1:-}" in
  multi)         reachable; multi ;;
  queue-full)    reachable; queue_full ;;
  retry)         reachable; retry ;;
  duplicate)     reachable; duplicate ;;
  short-corrupt) reachable; short_corrupt ;;
  compression)   reachable; compression ;;
  gui)           gui; exit 0 ;;
  all)           reachable; multi; duplicate; short_corrupt; compression; gui ;;
  *) sed -n '2,24p' "$0"; exit 2 ;;
esac
echo; echo "$PASS passed, $FAIL failed"
[[ $FAIL -eq 0 ]]
