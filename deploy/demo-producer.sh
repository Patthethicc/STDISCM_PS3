#!/usr/bin/env bash
# Producer VM: guided walk-through of the demo, for a call with the consumer-side presenter.
# Each step prints who speaks and what is said, then waits for Enter before running anything,
# so you control the pace. Full wording is in DEMO_SCRIPT.md.
#   ./deploy/demo-producer.sh        # from the start
#   ./deploy/demo-producer.sh 5      # resume at step 5 (after a hiccup)
set -uo pipefail
cd "$(dirname "$0")/.."
START="${1:-1}"
TOTAL=10

b=$'\033[1m'; d=$'\033[2m'; o=$'\033[0m'

pause() { read -r -p "${d}>> press Enter ${1:-to continue}${o} " _ || true; echo; }
head_() { echo; echo "${b}=== STEP $1/$TOTAL: $2 ===${o}"; }
say()   { echo "  YOU SAY:  $*"; }
cue()   { echo "  PARTNER:  $*"; }
# Run a scenario; a failed check should not stop the demo.
run()   { "$@" || echo "${b}(some checks failed: say \"let's look at the log\" and move on)${o}"; }

step() {
  case $1 in
  1) head_ 1 "Preflight"
     echo "  Check this machine, and have your partner run ./deploy/preflight.sh consumer."
     ./deploy/preflight.sh producer || { echo "Fix the FAIL lines first."; exit 1; }
     cue "consumer running as C=2 Q=10, GUI open and empty"
     pause "when both of you say READY" ;;
  2) head_ 2 "Introduction"
     say "p is the number of producer threads, one folder each. c and q are set on the consumer."
     pause ;;
  3) head_ 3 "Three producers at once"
     say "Three producer threads, each with its own folder, upload at the same time."
     cue "narrates the GUI filling in by itself; shows 'ls data/uploads'"
     pause "to start: ./tests/e2e/e2e.sh multi"
     run ./tests/e2e/e2e.sh multi ;;
  4) head_ 4 "The GUI"
     cue "hovers (10 s preview) and clicks (full video, Esc closes). You stay quiet."
     pause "when your partner finishes" ;;
  5) head_ 5 "Duplicate detection (Bonus 2)"
     say "One thread gets ACCEPTED, the other DUPLICATE. The hash is reserved atomically."
     cue "shows only one copy in the GUI"
     pause "to start: ./tests/e2e/e2e.sh duplicate"
     run ./tests/e2e/e2e.sh duplicate ;;
  6) head_ 6 "Short video and corrupt file"
     say "The corrupt file shows ERROR here. The video after it is still processed."
     cue "hovers the 5 s video; says a bad file fails only its own job"
     pause "to start: ./tests/e2e/e2e.sh short-corrupt"
     run ./tests/e2e/e2e.sh short-corrupt ;;
  7) head_ 7 "Compression (Bonus 3)"
     cue "points at original -> compressed on the GUI cards"
     pause "to start: ./tests/e2e/e2e.sh compression"
     run ./tests/e2e/e2e.sh compression ;;
  8) head_ 8 "Hand-off: tiny queue"
     cue "restarts the consumer as: C=1 Q=1 ./deploy/run-consumer.sh   (the GUI keeps running)"
     pause "when your partner says the consumer is restarted" ;;
  9) head_ 9 "Queue full (Bonus 1)"
     say "Several uploads get QUEUE_FULL, the rest ACCEPTED, none DUPLICATE."
     cue "explains the leaky bucket: the queue is checked when the header arrives, so no bytes are wasted"
     echo "  This takes a while (one worker encodes the accepted videos one at a time): keep talking."
     pause "to start: ./tests/e2e/e2e.sh queue-full"
     run ./tests/e2e/e2e.sh queue-full ;;
  10) head_ 10 "Retry the dropped videos"
     say "All previously dropped videos are ACCEPTED, none flagged DUPLICATE."
     pause "to start: ./tests/e2e/e2e.sh retry"
     run ./tests/e2e/e2e.sh retry
     echo
     say "p producer threads on one VM, c consumer workers on another, one bounded queue between them."
     cue "closing line, then questions" ;;
  esac
}

for ((n = START; n <= TOTAL; n++)); do step "$n"; done
echo; echo "${b}Demo finished.${o}"
