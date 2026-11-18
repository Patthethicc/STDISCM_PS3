#!/usr/bin/env bash
# Fills data/ with processed videos so the GUI can be tested before the consumer exists.
#   ./deploy/mock-data.sh                 # generates 4 test videos (one under 10s)
#   ./deploy/mock-data.sh path/to/videos  # processes your own videos instead
# Needs ffmpeg and ffprobe. Mirrors what the consumer workers will produce.
set -euo pipefail
DATA="${DATA_DIR:-data}"
mkdir -p "$DATA/uploads" "$DATA/previews" "$DATA/thumbnails"
SRC="${1:-}"

if [[ -z "$SRC" ]]; then
  SRC="$(mktemp -d)"
  i=0
  for spec in "testsrc2:20" "mandelbrot:15" "smptebars:25" "rgbtestsrc:6"; do
    i=$((i+1)); pat="${spec%%:*}"; dur="${spec##*:}"
    ffmpeg -loglevel error -y -f lavfi -i "$pat=size=640x360:rate=30" -f lavfi -i "sine=frequency=$((300+i*100))" \
      -t "$dur" -c:v libx264 -pix_fmt yuv420p -c:a aac "$SRC/sample_${i}_${pat}_${dur}s.mp4"
  done
fi

entries=()
for f in "$SRC"/*; do
  [[ -f "$f" ]] || continue
  sha=$(sha256sum "$f" | cut -d' ' -f1); id=${sha:0:16}
  name=$(basename "$f")
  dur=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$f" || true)
  if [[ -z "$dur" ]]; then echo "skip (not a video): $name"; continue; fi
  orig=$(stat -c %s "$f")
  ffmpeg -loglevel error -y -i "$f" -c:v libx264 -crf 28 -preset veryfast -c:a aac -movflags +faststart "$DATA/uploads/$id.mp4"
  ffmpeg -loglevel error -y -i "$DATA/uploads/$id.mp4" -t 10 -c:v libx264 -preset veryfast -an -movflags +faststart "$DATA/previews/$id.mp4"
  ffmpeg -loglevel error -y -ss 1 -i "$DATA/uploads/$id.mp4" -frames:v 1 "$DATA/thumbnails/$id.jpg" 2>/dev/null \
    || ffmpeg -loglevel error -y -i "$DATA/uploads/$id.mp4" -frames:v 1 "$DATA/thumbnails/$id.jpg"
  comp=$(stat -c %s "$DATA/uploads/$id.mp4")
  now=$(($(date +%s%N)/1000000))
  entries+=("{\"id\":\"$id\",\"filename\":\"$name\",\"sha256\":\"$sha\",\"duration_secs\":$dur,\"original_size\":$orig,\"compressed_size\":$comp,\"video_path\":\"uploads/$id.mp4\",\"preview_path\":\"previews/$id.mp4\",\"thumbnail_path\":\"thumbnails/$id.jpg\",\"added_at_ms\":$now}")
  echo "added: $name ($dur s)"
done
(IFS=,; echo "[${entries[*]}]") > "$DATA/catalog.json.tmp" && mv "$DATA/catalog.json.tmp" "$DATA/catalog.json"
echo "wrote $DATA/catalog.json with ${#entries[@]} videos"
