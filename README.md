# STDISCM_PS3 — Networked Producer and Consumer (Media Upload)

Distributed media upload system: producers on one node stream videos to a consumer on another node over gRPC; the consumer saves and processes videos with ffmpeg; a GUI shows them. Nodes run independently, so when one node is down only its features stop working.

## Architecture (nodes)

| Node | Runs | Ports open to the network | Example IP |
|---|---|---|---|
| Consumer VM | `consumer` (gRPC upload service: queue + C workers + storage) + `gui` (web page) | 50051 (gRPC), 8080 (GUI) | 192.168.56.20 |
| Producer VM | `producer` (P threads, one folder each) | none | 192.168.56.21 |

The GUI is its own process on the consumer VM; it reads the consumer's data folder, so it keeps showing videos even if the consumer process stops.

Inter-node communication: **gRPC** — client-streaming `media.MediaUpload.Upload` (`proto/media_upload.proto`). The producer sends one `Header` (filename, size, SHA-256) then `Chunk`s of 256 KiB. Statuses: `ACCEPTED`, `QUEUE_FULL`, `DUPLICATE`, `ERROR`.

## Components

- **consumer** (`consumer/`): gRPC server on `0.0.0.0:50051`. Bounded leaky-bucket queue of length Q (slot reserved when the header arrives, so dropped uploads never waste bandwidth — Bonus 1). C worker threads: verify hash/size, save to `data/uploads/<id>.mp4`, run the media pipeline, append to the catalog. Writes via `catalog` (atomic `catalog.json` replace under a mutex, so the GUI never reads a half-written file).
- **gui** (`gui/`): Leptos SSR web page on `0.0.0.0:8080`. Reads `catalog.json` on every request, re-fetches the grid every 3 s (`/api/videos`, `/fragments/grid`), serves media files with HTTP ranges (`/media/...`). Features: hover previews, click-to-play modal (Esc closes), graceful refresh.
- **producer** (`producer/`): P threads, each scanning its folder for video files, hashing them (SHA-256, streaming), and streaming them to the consumer. Logs one line per file: `ACCEPTED`, `QUEUE_FULL`, `DUPLICATE`, or `ERROR`.
- **crates**: `catalog` (video catalog + atomic writer), `media` (ffmpeg probe, thumbnail, 10 s preview, H.264/AAC compression — Bonus 3), `registry` (SHA-256 registry utility for duplicate detection; the consumer's `Storage` carries an equivalent in-memory set).

## How a video moves through the system

1. Producer hashes the file, sends `Header` then the bytes.
2. Consumer answers early on `QUEUE_FULL` or `DUPLICATE` (before most bytes are sent).
3. Job enters the bounded queue; a worker runs the media pipeline: `ffprobe` → thumbnail → 10 s preview → compress to H.264/AAC MP4 (Bonus 3).
4. Worker saves the video, adds it to the catalog (newest first), releases the hash reservation.
5. The GUI picks it up automatically within ~3 s.

## Setup

### Prerequisites

Rust (stable; see `rust-toolchain.toml`), protoc (needed to build the gRPC code), ffmpeg + ffprobe (consumer VM; also used by the test scripts), git.

Windows: install Rust from rustup.rs, then `winget install Google.Protobuf Gyan.FFmpeg` (or `choco install protoc ffmpeg`), and run the `.sh` scripts in **Git Bash**.

### Build

```bash
git clone https://github.com/Patthethicc/STDISCM_PS3.git
cd STDISCM_PS3
cp deploy/.env.example deploy/.env
cargo build --release   # first time takes a few minutes
```

### Config

Edit `deploy/.env` on both VMs:
- both: `CONSUMER_HOST` = consumer VM's IP
- consumer: `C` (worker threads), `Q` (queue capacity)
- producer: `FOLDERS` (one folder per producer thread, `P` of them)

Values can be overridden per run: `C=1 Q=1 ./deploy/run-consumer.sh`.

### Output folder (consumer VM)

```
data/
  catalog.json          list of processed videos (the GUI reads this)
  uploads/<id>.mp4      saved videos (H.264/AAC after compression)
  previews/<id>.mp4     first 10 seconds
  thumbnails/<id>.jpg
  tmp/                  consumer-side temp dir (run scripts create it; uploads are staged inside uploads/)
```

Start fresh: `rm -rf data/uploads/* data/previews/* data/thumbnails/* data/catalog.json` (generate videos into `smoke/` and `sample-videos/` manually too).

## Run

**Consumer VM**, two terminals:

```bash
./deploy/run-consumer.sh   # gRPC on :50051, C workers, queue length Q
./deploy/run-gui.sh        # GUI on :8080
```

Open `http://<consumer IP>:8080` in a browser.

**Producer VM:**

```bash
./deploy/run-producer.sh                            # uses P, FOLDERS from deploy/.env
./deploy/run-producer.sh videos/a videos/b videos/c # or pass folders directly (P = number of folders)
```

## Testing

- **Unit/integration**: `cargo test --workspace` — consumer (queue, server, storage, worker, shutdown), producer (upload against a mock consumer), catalog, registry; media pipeline tests need ffmpeg + ffprobe on PATH.
- **Media pipeline example**: `cargo run -p media --example process -- <data_dir> <video>...` — prints what a worker would log for each file.
- **End-to-end**: `./tests/e2e/e2e.sh <scenario>` (on the producer VM, needs ffmpeg + curl; consumer + GUI must be running with the settings in the scenario table):

  | Scenario | Consumer settings | Expects |
  |---|---|---|
  | `multi` | C=2 Q=10 | p=3, three folders × 3 videos → 9 × `ACCEPTED`, all videos appear in the GUI |
  | `queue-full` | C=1 Q=1 | 8 producers at once → some `QUEUE_FULL`, some `ACCEPTED`, nothing `DUPLICATE` |
  | `retry` | same as queue-full | retry dropped videos → `ACCEPTED`, none `DUPLICATE` |
  | `duplicate` | — | same video in two folders → 1 `ACCEPTED`, 1 `DUPLICATE` |
  | `short-corrupt` | — | 5 s video fine (preview plays); corrupt file errors; next video still processed |
  | `compression` | — | before/after sizes show `original → compressed` for every video |
  | `gui` | — | manual GUI checks printed (hover, click, live refresh) |
  | `all` | — | multi + duplicate + short-corrupt + compression + gui |

- **GUI without producer**: `./deploy/mock-data.sh` (generates 4 test videos into `data/`), then `./deploy/run-gui.sh`.

See `deploy/SETUP.md` for the detailed two-VM network setup, and `DEMO.md` for the 8-minute demo script.

## Fault behavior

- **Consumer down:** the GUI keeps serving (it reads `data/` on each request); the producer logs `cannot connect to consumer` and stops that thread. If the consumer drops mid-run, the producer logs an error per affected file and continues with the next one; when the consumer returns, subsequent uploads succeed again.
- **Producer down:** the consumer is unaffected; it just stops receiving uploads.
- **GUI down:** videos remain in `data/`; the consumer keeps processing and appending to the catalog; reopen the GUI and they appear automatically.

## Status

| Piece | Status |
|---|---|
| Consumer (gRPC service, leaky-bucket queue, workers, storage) | Implemented and tested |
| Producer (scanner, hasher, uploader, worker) | Implemented and tested |
| Media pipeline (probe, thumbnail, preview, compression — Bonus 3) | Implemented |
| GUI (Leptos SSR, live refresh, hover preview, player modal) | Implemented |
| Unit/integration tests (consumer, producer, catalog, registry) | Implemented |
| End-to-end tests (`tests/e2e/e2e.sh`) | Implemented |
| Demo script (`DEMO.md`) | Implemented |

## Notes

- Duplicate detection (Bonus 2): the producer hashes the file and sends the SHA-256; the consumer reserves the hash when the header arrives (atomic check-and-reserve). Dropped/failed uploads release the reservation so a retry uploads normally; once saved, the hash stays taken (reseeded from the catalog on consumer restart).
- Timeouts: probe 30 s, encode 600 s (`MediaConfig` defaults).
- Consumer CLI: `-c` (worker threads, default 4), `-q` (queue capacity, default 16), `--bind 0.0.0.0:50051`, `--data-dir ./data`. GUI: `--bind 0.0.0.0:8080`, `--data-dir data`.
- Stats: `CompressionStats` records `original_bytes`, `compressed_bytes`, `ratio`, `saved_percent`; GUI cards show `original → compressed`.
- Producer chunk size: 256 KiB (`CHUNK_SIZE`), well under gRPC's 4 MiB message limit.
