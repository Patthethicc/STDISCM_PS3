# Setup Guide (P3: Networked Producer and Consumer)

Two VMs talk over gRPC:

| VM | Runs | Port open to the network | Example IP |
|---|---|---|---|
| Consumer | `consumer` (gRPC upload service) + `gui` (web page) | 50051 (gRPC), 8080 (GUI) | 192.168.56.20 |
| Producer | `producer` (p threads, one folder each) | none | 192.168.56.21 |

The GUI is its own process on the consumer VM. It reads the consumer's data folder, so it keeps
showing videos even if the consumer process stops.

---

## 1. Network

- **VirtualBox:** give both VMs a second adapter set to **Host-only Adapter** on the same network
  (e.g. 192.168.56.x). Note each VM's IP with `ip a`.
- **Two laptops:** same Wi-Fi or hotspot; use `ipconfig` (Windows) / `ip a` (Linux) for the IPs.

From the producer VM: `ping 192.168.56.20`.

Open the ports on the consumer VM, e.g. Ubuntu: `sudo ufw allow 50051/tcp && sudo ufw allow 8080/tcp`.
On Windows, allow the programs through the firewall when asked.

## 2. Install on both VMs

```bash
# Rust (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# protoc (needed to build the gRPC code) and ffmpeg (consumer VM; also used by the test scripts)
sudo apt install -y protobuf-compiler ffmpeg git
```

Windows: install Rust from rustup.rs, then `winget install Google.Protobuf Gyan.FFmpeg` (or
`choco install protoc ffmpeg`), and run the `.sh` scripts in **Git Bash**.

```bash
git clone https://github.com/Patthethicc/STDISCM_PS3.git
cd STDISCM_PS3
cp deploy/.env.example deploy/.env
```

Edit `deploy/.env` on both VMs: set `CONSUMER_HOST` to the consumer VM's IP. On the consumer VM,
set `C` and `Q`. On the producer VM, set `FOLDERS` (one folder per producer thread).

## 3. Build (both VMs, first time takes a few minutes)

```bash
cargo build --release
```

## 4. Run

**Consumer VM**, two terminals:

```bash
./deploy/run-consumer.sh      # gRPC on :50051, c workers, queue length q
./deploy/run-gui.sh           # GUI on :8080
```

Open `http://<consumer IP>:8080` in a browser.

**Producer VM:**

```bash
./deploy/run-producer.sh                        # P and FOLDERS from deploy/.env
./deploy/run-producer.sh videos/a videos/b      # or pass folders directly (p = number of folders)
```

Each file logs one line: `ACCEPTED (queued, not yet saved)`, `QUEUE_FULL`, `DUPLICATE`, or `ERROR`.
ACCEPTED means the video got a queue slot; it appears in the GUI once a worker has saved and processed it.

## 5. Inputs

| Input | Where | Meaning |
|---|---|---|
| `p` | producer (number of folders) | producer threads, each reading its own folder |
| `c` | `C` in `.env` on the consumer | consumer worker threads |
| `q` | `Q` in `.env` on the consumer | max queue length; uploads beyond it are dropped (leaky bucket) |
| worker delay | `WORKER_DELAY_MS` on the consumer | slows workers so the queue fills, to demo drops |

Override any of them for one run: `C=1 Q=2 WORKER_DELAY_MS=3000 ./deploy/run-consumer.sh`.

## Output folder (consumer VM)

```
data/
  catalog.json          list of processed videos (the GUI reads this)
  uploads/<id>.mp4      saved videos (H.264/AAC after compression)
  previews/<id>.mp4     first 10 seconds
  thumbnails/<id>.jpg
```

Start fresh: stop the consumer and GUI, then `rm -rf data/uploads/* data/previews/* data/thumbnails/* data/catalog.json`.

## Testing the GUI without the producer

On the consumer VM: `./deploy/mock-data.sh` (generates 4 test videos into `data/`), then `./deploy/run-gui.sh`.

## Troubleshooting

| Problem | Fix |
|---|---|
| Producer: `cannot connect to consumer` | Check `CONSUMER_HOST`, `ping`, firewall port 50051, and that `run-consumer.sh` is running |
| GUI page doesn't open from another machine | Firewall port 8080; `run-gui.sh` binds 0.0.0.0 already |
| Build error about `protoc` | Install protobuf-compiler (step 2) |
| Videos show "processing" or no preview | ffmpeg missing on the consumer VM |
| No drops even with a small queue | Set `WORKER_DELAY_MS` (e.g. 3000) and use more videos than `Q` |
