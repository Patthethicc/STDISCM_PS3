# Demo Script (P3: Networked Producer and Consumer)

Target length: ~8 minutes. Record the GUI (browser), the consumer terminal and the producer terminal.
Two VMs: consumer (`CONSUMER_IP`) and producer. Setup: `deploy/SETUP.md`.

## Before recording

- [ ] Both VMs built (`cargo build --release`), `deploy/.env` filled in on both
- [ ] Fresh consumer data: `rm -rf data/uploads/* data/previews/* data/thumbnails/* data/catalog.json`
- [ ] Producer VM has ffmpeg (the test script generates its own test videos)
- [ ] Browser open at `http://CONSUMER_IP:8080`, terminals side by side

## 0. Architecture (30 s)

> "The producer and consumer run on different VMs and talk over gRPC. Each producer thread reads its
> own folder and streams each video in chunks. The consumer checks the header first: if the queue is
> full or the video is a duplicate, it rejects it before any bytes are sent. Accepted videos go into a
> bounded queue, and c worker threads save them to one folder, compress them with ffmpeg, and make a
> 10-second preview and a thumbnail. The GUI is a separate web server on the consumer VM that reads
> the same folder."

## 1. Multiple producers (1.5 min)

Consumer VM: `C=2 Q=10 ./deploy/run-consumer.sh` and `./deploy/run-gui.sh`

Producer VM:
```bash
./tests/e2e/e2e.sh multi
```
- Producer log: 9 lines `ACCEPTED <id>` from producer-1, -2, -3.
- GUI: the videos pop in on their own (no reload) as workers finish.
- Consumer VM: `ls data/uploads` shows all of them in the one folder.

> "ACCEPTED means a worker has saved and processed the video. The consumer only replies once the job is done, so it shows in the GUI right around then."

## 2. GUI (1 min)

- Hover a video: its first 10 seconds play from the start; move away and it stops.
- Click a video: the full video plays; Esc closes it.

## 3. Queue full: leaky bucket + Bonus 1 (1.5 min)

Restart the consumer with a tiny queue:
```bash
C=1 Q=1 ./deploy/run-consumer.sh
```
Producer VM:
```bash
./tests/e2e/e2e.sh queue-full
```
- 8 producers upload at once. Producer log: several `QUEUE_FULL` lines; the rest ACCEPTED.

> "Bonus 1: the consumer tells the producer the queue is full. It reserves the queue slot when the
> header arrives, so a dropped video never wastes bandwidth uploading its bytes."

Then retry the dropped ones:
```bash
./tests/e2e/e2e.sh retry
```
> "A dropped video releases its duplicate-hash reservation, so retrying it later uploads normally
> instead of being flagged as a duplicate."

## 4. Duplicate detection: Bonus 2 (1 min)

Consumer back to normal: `C=2 Q=10 ./deploy/run-consumer.sh`
```bash
./tests/e2e/e2e.sh duplicate
```
- Same file in two folders, uploaded at the same time: one `ACCEPTED`, one `DUPLICATE`.

> "The hash is reserved atomically, so two identical uploads arriving together can't both get through."

## 5. Short and corrupt videos (1 min)

```bash
./tests/e2e/e2e.sh short-corrupt
```
- The 5-second video shows in the GUI and its preview plays (hover it).
- Consumer terminal: an error line for `zz_corrupt.mp4`; the next video is still processed.

> "A bad file only fails that one job; the worker keeps running."

## 6. Compression: Bonus 3 (30 s)

```bash
./tests/e2e/e2e.sh compression
```
- Before/after size for every video. The GUI cards also show `original → compressed`.

> "Videos are re-encoded to H.264/AAC MP4, which also makes every upload playable in the browser."

## 7. Synchronization summary (30 s, can be a slide)

| Shared thing | How it's protected |
|---|---|
| Queue (size q) | Bounded queue; slot reserved when the header arrives, released if the upload fails |
| Duplicate hashes | Hash registry with atomic check-and-reserve, released when a video is dropped or fails |
| Catalog (list the GUI reads) | Mutex around add + atomic file replace, so the GUI never reads a half-written file |
| Output folder | Each video gets its own file name (hash ID), so workers never write the same file |

## Closing line

> "p producer threads on one VM, c consumer workers on another, one bounded queue between them,
> and a GUI that updates by itself as videos finish."
