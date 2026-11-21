# Presentation and Demo Script (P3: Networked Producer and Consumer)

Presenters: Meg and Pat. The slide deck comes first (about 7 to 8 minutes), then the live demo.
Two machines: the consumer (`CONSUMER_IP`) and the producer. Setup: `deploy/SETUP.md`.

| Part | Slides | Speaker |
|---|---|---|
| Opening | 1 to 2 | Meg |
| Producer and duplicate detection | 3 to 6 | Meg |
| Consumer ingest and queue | 7 to 14 | Pat |
| Media processing and compression | 15 to 21 | Meg |
| GUI and deployment | 22 to 23 | Pat |
| Live demo | none, screen share | Meg types the producer commands, Pat runs the consumer and the browser |

Each slide number below is the slide to have on screen while that part is spoken.

## Before the demo

- [ ] Both machines built (`cargo build --release`), `deploy/.env` filled in on both
- [ ] Fresh consumer data: `rm -rf data/uploads/* data/previews/* data/thumbnails/* data/catalog.json`
- [ ] The producer machine has ffmpeg (the e2e script generates its own test videos)
- [ ] Pat has the consumer and GUI running (`C=2 Q=10 ./deploy/run-consumer.sh`, `./deploy/run-gui.sh`) and the browser open at `http://CONSUMER_IP:8080`

---

## Opening

**Slide 1 (Meg)**
> "Hi everyone. We're presenting Problem Set 3, a producer and consumer video upload system. Our group is Wesner, Patrick, Jose and me, Megan, and Patrick and I will walk you through it."

**Slide 2 (Meg)**
> "It has four parts: the producer and duplicate detection, the consumer's ingest and queue, media processing and compression, and the GUI and integration. I'll take the producer and the media processing. Patrick takes the consumer, the GUI and the deployment."

## Producer and duplicate detection (Meg)

**Slide 3**
> "The producer and consumer talk over gRPC. Each video is one client-streaming call, sent in chunks, because a video is bigger than gRPC's 4 MiB message limit. The header goes first and carries the hash, so the consumer can answer before any video bytes arrive. And ACCEPTED means done: the consumer replies after the worker has saved and processed the video, and the reply carries the video's ID."

**Slide 4**
> "The producer runs one OS thread per folder. Each thread builds its own single-threaded tokio runtime and opens its own gRPC channel, so the producer needs no locks. A failed connection ends only that thread, and inside a thread the files go one after another."

**Slide 5**
> "On the wire it's a header, then chunks. The file is read in pieces and pushed into a channel that holds only 4 messages. If the consumer answers early, for example with QUEUE_FULL, the receiver is dropped, the next send fails and the sender stops. So a rejected video wastes almost no bandwidth, and memory stays small."

**Slide 6**
> "For duplicate detection, the key is the file's SHA-256. The check and the insert happen under one mutex, so two identical uploads can't both pass: the first wins and the other gets DUPLICATE. Release lets a dropped upload retry, and once a video is saved, its hash stays reserved. The consumer applies this same logic in its own storage, and reloads the saved hashes from the catalog on startup. Patrick will show you what the consumer does with the header."

## Consumer ingest and queue (Pat)

**Slide 7**
> "The consumer has four pieces: a gRPC server on tonic and tokio, a bounded queue, c worker tasks, and a graceful shutdown."

**Slide 8**
> "Each upload is its own task, so the server handles many at once on port 50051. It reads only the header, then checks the queue first, then duplicates, and streams the rest to a worker. QUEUE_FULL and DUPLICATE are sent right after the header. ACCEPTED or ERROR comes after the worker finishes. The server only routes. Saving and processing happen in the workers."

**Slide 9**
> "Two settings control the consumer: c is the number of workers, and q is the maximum queue length. For example, `C=2 Q=10` with the run script."

**Slide 10**
> "The queue is a bounded channel of q waiting jobs. We use try_send, which never waits. If the queue is full, the upload is dropped at once. That's the leaky bucket."

**Slide 11**
> "The queue check happens right after the header. If it's full, the consumer replies QUEUE_FULL, and no video is saved and no temp file is made. There's a small race, because the queue can fill between the check and the enqueue. In that case we release the hash and still send QUEUE_FULL, so a retry isn't flagged as a duplicate."

**Slide 12**
> "A dispatcher hands jobs to the c workers round-robin, and each worker takes one job at a time. Per job it receives the bytes, checks the SHA-256 and the size, runs the media pipeline and adds the video to the catalog. A bad file ends one job, and the worker keeps running."

**Slide 13**
> "The bytes stream into a temp file in the uploads folder, and the hash and size must match the header. The video ends up at `uploads/<id>.mp4`, or with its original extension if compression fails. If anything goes wrong, the temp file is deleted and the hash is released, so a retry is allowed."

**Slide 14**
> "On Ctrl+C or SIGTERM, one cancel token tells the server and the workers to stop. The server stops accepting uploads, each worker finishes the job it's on, and the consumer waits up to 30 seconds before it exits. The GUI keeps running, because it's a separate process. Meg will explain what a worker does with each video."

## Media processing and compression (Meg)

**Slide 15**
> "Media processing uses ffmpeg and ffprobe, in four steps: probe, which rejects non-video files; preview, the first 10 seconds muted; thumbnail, one small JPEG frame; and compress, to H.264 and AAC."

**Slide 16**
> "A worker calls `process_video` after the upload finishes, and it returns either the processed media or an error. ffmpeg runs as a child process, so a crash or a hang stays in the child, and the arguments are passed without a shell. Probe and preview are required, while thumbnail and compression only warn. A failed required step deletes all of that video's outputs."

**Slide 17**
> "The preview is the first 10 seconds, muted, at most 480 pixels wide, and it starts playing at once. Browsers only autoplay on hover when it's muted. A 4-second upload gives a 4-second preview. The thumbnail is one frame, at most 320 pixels wide."

**Slide 18**
> "The probe rejects empty files, files ffprobe can't read, still images, and files with no real video stream. A panic would kill a worker and cut capacity to c minus 1, so every bad input returns an error instead. A compression timeout is only a warning, while a probe or preview timeout is an error."

**Slide 19**
> "Almost nothing is shared. The config is one read-only value, so it needs no lock. Output is written to a `.part` file and renamed, which is atomic, so the GUI never sees half a file. Two reader threads drain ffmpeg's output, because a full 64 KiB pipe would deadlock ffmpeg against the waiting worker. On a timeout we kill the process and then wait, so there are no zombies."

**Slide 20**
> "Every upload is re-encoded to H.264 and AAC in an MP4, so it plays in any browser. The compressed file replaces the original, and if compression fails we keep the original and log a warning. The catalog stores the compressed size, and the GUI cards show original to compressed."

**Slide 21**
> "Encoding is CPU-bound, and each ffmpeg uses every core by default. So we give each ffmpeg cores divided by c threads. Each worker runs the pipeline in spawn_blocking, so it never stalls the async runtime. Slow encodes keep workers busy, so the queue fills sooner and QUEUE_FULL shows up. Patrick will finish with the GUI and deployment."

## GUI and deployment (Pat)

**Slide 22**
> "The GUI is a separate process with its own web server. It only reads the shared data folder, so the consumer and the GUI never block each other, and it keeps serving videos even if the consumer crashes. The grid refreshes every 3 seconds. Hover plays the first 10 seconds, and click opens the full video in a player."

**Slide 23**
> "We run on two machines. The producer machine sends to the consumer over gRPC on port 50051, and the consumer machine runs the consumer and the GUI on port 8080. One env file per machine sets the addresses, ports, c and q. And each scenario in our verification table is one end-to-end command. Now let's show it running."

Keep slide 23 up until you switch to the live demo.

---

## Live demo

Pat starts and restarts the consumer and drives the browser. Meg types every command that sends videos. Say "Over to you" to hand off, and Pat says "Consumer restarted" before the queue-full step.

While Meg is sharing the terminal, flash a slide back up only if someone asks.

| Step | Meg types (producer machine) | Meg says | Pat says | Slide to flash if asked |
|---|---|---|---|---|
| Multiple producers | `./tests/e2e/e2e.sh multi` | "Three threads, each with its own folder, upload at once." | "The videos appear in the GUI as the workers finish." | 4, 22 |
| Duplicate | `./tests/e2e/e2e.sh duplicate` | "One gets ACCEPTED, the other DUPLICATE, because the hash is reserved atomically." | "Only one copy shows in the GUI." | 6 |
| Short and corrupt | `./tests/e2e/e2e.sh short-corrupt` | "The corrupt file shows ERROR on the producer." | "Each bad file fails its own job. The worker keeps running." | 18 |
| Compression | `./tests/e2e/e2e.sh compression` | "This prints size before and after." | "Every upload is re-encoded to H.264/AAC." | 20 |
| Queue full | `./tests/e2e/e2e.sh queue-full`, then `./tests/e2e/e2e.sh retry` | "Several uploads get QUEUE_FULL, and retrying them works." | "Consumer restarted with `C=1 Q=1`. Anything past q is dropped." | 10, 11 |

For the queue-full step, Pat restarts the consumer first: `C=1 Q=1 ./deploy/run-consumer.sh`. The GUI keeps showing videos while it restarts, because it's a separate process.

**GUI checks (Pat, in the browser):** hover a video to see the first 10 seconds play and stop when the mouse leaves. Click a video to play the full video, and press Esc to close it.

## Timing

About 7 to 8 minutes for the slides at about 20 seconds each, plus the demo. To stay near 10 minutes in total, speak slides 9, 13, 14, 17 and 18 in one sentence each. The queue-full scenario takes a while because the accepted videos are encoded one at a time, so Pat can use that wait to explain slides 10 and 11.

## If something goes wrong

| Problem | Do this |
|---|---|
| Producer shows `cannot connect to consumer` | Pat checks that the consumer is running, Meg checks `CONSUMER_HOST` in `deploy/.env` and the firewall on port 50051 |
| GUI is empty | Pat runs `./deploy/mock-data.sh` to fill `data/` with sample videos, then reloads |
| Nothing is dropped in `queue-full` | Pat confirms the consumer really was restarted with `C=1 Q=1` |
| A scenario takes too long | Skip to the next one. The producer log still shows the results. |
| The live demo fails | Show the recorded rehearsal |
