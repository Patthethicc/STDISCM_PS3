# Demo Script for Two Presenters (P3: Networked Producer and Consumer)

Target length: about 9 minutes, plus questions.

| Presenter | Machine | Screen | Owns |
|---|---|---|---|
| **A: Consumer side** | Consumer VM | Consumer terminal, GUI terminal, and the browser at `http://CONSUMER_IP:8080` | Architecture, the consumer, the GUI, the queue and the bonuses on the consumer side |
| **B: Producer side** | Producer VM | One terminal | The producer, the test commands, and the results on the producer side |

Swap in your names. Whoever built a part should present that part. **Rule of thumb:** B types every command that sends videos; A types every command that starts or restarts the consumer, and does everything in the browser.

Nothing here has been run on two VMs yet. It is written from the code, so do one full rehearsal and write down the real timings.

---

## Doing it remotely (you are in different places)

**1. Get the machines on one network (Tailscale).** B's machine must reach A's ports 50051 (gRPC) and 8080 (GUI). `deploy/tailscale-setup.sh` installs Tailscale, connects the machine, and wires up the producer for you.

*Once, by hand:* both of you need to be in the same Tailscale network. Either A invites B at `login.tailscale.com/admin/users` (B then signs in with the invited account), or A creates a short-lived, ephemeral auth key at `login.tailscale.com/admin/settings/keys` and sends it to B privately. Never commit a key, and revoke it after the demo.

Then:
```bash
./deploy/tailscale-setup.sh consumer     # A first: installs, connects, prints the address and ports to allow
./deploy/tailscale-setup.sh producer     # B next: finds A's machine, sets CONSUMER_HOST in deploy/.env, pings, runs the preflight
```
If B joins with an auth key instead of an invite: `TS_AUTHKEY=tskey-auth-... ./deploy/tailscale-setup.sh producer`. If B's script can't find A's machine by name, it asks for A's Tailscale address (`100.x.y.z`), or B passes it as an argument.

**2. Run the preflight on each machine before the call.**
```bash
./deploy/preflight.sh consumer   # A
./deploy/preflight.sh producer   # B
```
It checks `.env`, ffmpeg, the builds, and that B can reach A's gRPC port and GUI. Both must print `READY`.

**3. Decide who shares what.** The GUI is the main thing the audience watches, so A shares their screen for most of the demo (the browser plus the consumer terminal). B shares only while running a command, and stops sharing after. If your call tool lets two people share at once, keep both up and skip the hand-offs.

**4. Hand-off cues.** Say these out loud so nobody has to guess whose turn it is:

| Who says it | Cue | Meaning |
|---|---|---|
| Either | "Over to you, [name]" | I'm done; you start and share your screen |
| A | "Consumer restarted" | B can start the queue-full test |
| B | "Done, results are in" | A can show the GUI result |

If audio fails, type `ready` or `done` in the call chat instead.

**5. B runs the guided script.** Instead of typing each command, B runs:
```bash
./deploy/demo-producer.sh        # from the start
./deploy/demo-producer.sh 5      # resume at step 5 after a hiccup
```
For each step it prints who speaks and the short line to say, then waits for Enter before running anything. B stays in control of the pace. It follows the same order as the sections below, with a preflight step first and the queue-full test and its retry as two steps at the end.

**6. Record the rehearsal.** Record one full run on the call. If the live demo breaks, you have a backup to show.

---

## Before the demo (both)

- [ ] Both machines built with `cargo build --release`, and `deploy/.env` filled in on both (`CONSUMER_HOST` = consumer's address).
- [ ] Both preflights print `READY` (see above).
- [ ] A: fresh data. Stop the consumer and GUI, then `rm -rf data/uploads/* data/previews/* data/thumbnails/* data/catalog.json`.
- [ ] A starts the GUI: `./deploy/run-gui.sh`. Browser open and empty. Font size large enough for the audience.
- [ ] A starts the consumer: `C=2 Q=10 ./deploy/run-consumer.sh`.
- [ ] Optional: put three real videos in `sample-videos/p1`, `p2` and `p3` on B's machine for a better look than the test pattern.

---

## 1. Introduction and architecture (A, 45 s)

**A** (shows a slide or the README table):
> "This is a media upload service split across two machines that talk over gRPC. Producers read videos from folders and stream them in chunks. The consumer accepts uploads from several producers at once, saves them into one folder, compresses them, and a web page shows them. I'll show the consumer side and the GUI; [B] will drive the producers."

**B:**
> "The producer takes p, the number of threads. Each thread has its own folder, so three folders means three threads. The consumer takes c worker threads and q, the maximum queue length. Both the consumer and the GUI run on the other VM."

Show the inputs on screen: A shows `C=2 Q=10`, B shows `./deploy/run-producer.sh a b c` (three folders = p of 3).

---

## 2. Multiple producers upload at once (B drives, A narrates, 1.5 min)

**B** runs:
```bash
./tests/e2e/e2e.sh multi
```
This generates 3 folders of 3 videos, runs 3 producer threads, and checks that all 9 uploaded.

**B** (while it runs):
> "Three producer threads, each with its own folder, are uploading at the same time."

**A** (shows the browser and consumer terminal):
> "On the consumer, the videos appear on their own as the workers finish them. No page reload."

- **B:** point at the producer log. Each line is `[producer-N] file: ACCEPTED <id>`, from producers 1, 2 and 3.
- **A:** run `ls data/uploads` on the consumer. All videos are in the one folder.
- **A, important:** ACCEPTED arrives **after** the worker has saved and processed the video, because the consumer only replies once the job is done. Say this if asked.
- **A tip:** the grid doesn't refresh while a preview is playing or the player is open. Move the mouse off the cards to see new videos pop in.

---

## 3. The GUI (A, 1 min)

**A:**
- **Hover** a video: its first 10 seconds play from the start. Move away and it stops.
- **Click** a video: the full video plays in a player. **Esc** closes it.

> "The preview is a separate 10-second clip the worker made, so hovering never loads the full video."

**B** (while A does this): run `./tests/e2e/e2e.sh gui` to show the checklist, or keep quiet. Don't type commands during A's part.

---

## 4. Duplicate detection: Bonus 2 (B drives, 1 min)

**B** runs:
```bash
./tests/e2e/e2e.sh duplicate
```
The same file is in two folders and uploaded at the same moment.

**B:**
> "One thread gets ACCEPTED, the other gets DUPLICATE. The producer hashes the file with SHA-256, and the consumer reserves that hash the moment the header arrives, as one atomic step. Two identical uploads can't both get through."

**A:** point at the GUI. Only one copy appears.

---

## 5. A short video and a corrupt file (B drives, 1 min)

**B** runs:
```bash
./tests/e2e/e2e.sh short-corrupt
```

- **A:** hover the 5-second video. Its preview plays fine even though it's shorter than 10 seconds.
- **B:** point at the producer terminal. The corrupt file logs `zz_corrupt.mp4: ERROR rejected: ...`.
- **A:** the consumer terminal shows `job failed ... filename=zz_corrupt.mp4 reason=rejected: ...`. The video uploaded after the corrupt one still appears in the GUI.

> A: "A bad file fails only its own job. The worker keeps running."

*Verify in rehearsal:* the exact wording of the `reason` depends on the file, so read it once beforehand.

---

## 6. Compression: Bonus 3 (B drives, A shows, 30 s)

**B** runs:
```bash
./tests/e2e/e2e.sh compression
```
It prints the before and after size for every video.

**A:** point at the GUI cards, which show `original → compressed`.

> "Every upload is re-encoded to H.264 and AAC, which also makes every video playable in the browser."

---

## 7. Queue full: the leaky bucket and Bonus 1 (A restarts, B drives, 2 min)

**A:** stop the consumer and restart it with one worker and one queue slot. The GUI keeps running and keeps showing videos.
```bash
C=1 Q=1 ./deploy/run-consumer.sh
```
> A: "The GUI is a separate process, so it keeps showing videos while I restart the consumer."

**B** runs:
```bash
./tests/e2e/e2e.sh queue-full
```
8 producers upload 16 videos against a one-slot queue.

**B:**
> "Several uploads get QUEUE_FULL and the rest are ACCEPTED. Nothing is flagged DUPLICATE."

**A:**
> "That's the leaky bucket: anything beyond q is dropped. The consumer checks the queue when the header arrives, and tells the producer before the video's bytes are sent. So a dropped video wastes no bandwidth. That's Bonus 1."

*This scenario can take a while because the accepted 20-second videos are encoded one at a time. Fill the wait with the talking points above.*

Then, once it finishes:
```bash
./tests/e2e/e2e.sh retry
```
**B:**
> "Retrying the dropped videos: all ACCEPTED, none flagged DUPLICATE. A dropped upload releases its hash reservation, so retrying is safe."

---

## 8. Why it's safe to run in parallel (A, 30 s, can be a slide)

| Shared thing | How it's protected |
|---|---|
| Queue (size q) | Bounded queue; a slot is taken when the header arrives and released if the upload fails |
| Duplicate hashes | Atomic check-and-reserve; released when a video is dropped or fails |
| Catalog the GUI reads | Mutex around each add, and an atomic file replace, so the GUI never reads a half-written file |
| Output folder | Each video is named by its hash ID, so two workers never write the same file |

---

## Closing line (B, then A)

**B:**
> "p producer threads on one VM, c consumer workers on another, and one bounded queue between them."

**A:**
> "And a GUI that updates by itself, with hover previews and click-to-play. Questions?"

---

## Likely questions

| Question | Answer |
|---|---|
| What happens if the consumer goes down? | The GUI keeps serving because it reads the data folder. The producer logs `cannot connect to consumer` for new uploads. When the consumer returns, uploads work again. |
| What happens if the producer goes down? | The consumer is unaffected; it just stops receiving. |
| Why does ACCEPTED take a while? | The consumer replies only after a worker has saved, processed and cataloged the video. |
| Why check the header first? | So `QUEUE_FULL` and `DUPLICATE` are answered before the video bytes are sent. |
| What are p, c and q? | p = producer threads (one folder each), c = consumer worker threads, q = max queue length. |
| What if two producers send the same file? | One is accepted and the other gets DUPLICATE, because the hash is reserved atomically. |
| Is a restarted consumer still duplicate-safe? | Yes. It reseeds its known hashes from the catalog on startup. |

---

## If something goes wrong

| Problem | Do this |
|---|---|
| Producer: `cannot connect to consumer` | A checks that the consumer is running; B checks `CONSUMER_HOST` and the firewall on port 50051 |
| GUI is empty | A: `./deploy/mock-data.sh` fills `data/` with 4 sample videos, then reload |
| Nothing is dropped in `queue-full` | A confirms the consumer really is `C=1 Q=1` |
| A scenario takes too long | Skip to the next one and say it takes a while; the producer log still shows the results |
| Live demo fails | Show the last recorded run, or the passing output of `cargo test --workspace` |

## Known doc mismatch

The producer's log label was fixed in commit `bf10b00` ("correct ACCEPTED log label"), so the log reads `ACCEPTED <id>`. `DEMO.md` and `deploy/SETUP.md` were updated to match: the consumer replies only after the worker finishes, so ACCEPTED means saved and processed.
