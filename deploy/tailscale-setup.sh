#!/usr/bin/env bash
# Puts the consumer and producer machines on one Tailscale network, so they can reach each other
# from different places. Run it once on each machine.
#
#   ./deploy/tailscale-setup.sh consumer                 # on the consumer machine, first
#   ./deploy/tailscale-setup.sh producer [CONSUMER_IP]   # on the producer machine
#
# What it does: installs Tailscale if missing (asks first), connects this machine, and then
#   consumer: prints this machine's Tailscale address and the ports to allow
#   producer: finds the consumer, writes CONSUMER_HOST into deploy/.env, pings it, runs the preflight
#
# Both machines must be in the SAME tailnet. Do ONE of these before running (once):
#   a) Owner invites the other person: https://login.tailscale.com/admin/users -> Invite users.
#      The other person then signs in with the invited account when this script opens the login link.
#   b) Owner creates an auth key at https://login.tailscale.com/admin/settings/keys (short expiry,
#      ephemeral) and sends it privately. The other person runs:
#         TS_AUTHKEY=tskey-auth-... ./deploy/tailscale-setup.sh producer
#      Do not paste the key into a file or commit it. Revoke it after the demo.
#
# The default machine names are stdiscm-consumer and stdiscm-producer, so the producer can find the
# consumer without being told its address.
set -uo pipefail
cd "$(dirname "$0")/.."

role="${1:-}"
[[ $role == consumer || $role == producer ]] || { sed -n '5,6p' "$0"; exit 2; }
name="stdiscm-$role"
CONSUMER_NAME="stdiscm-consumer"

case "$(uname -s)" in
  Linux*) os=linux ;;
  MINGW*|MSYS*|CYGWIN*) os=windows ;;
  Darwin*) os=mac ;;
  *) os=other ;;
esac

die()  { echo "ERROR: $*" >&2; exit 1; }
info() { echo ">> $*"; }

# ---------------------------------------------------------------- 1. find or install Tailscale
TS=""
find_ts() {
  if command -v tailscale >/dev/null 2>&1; then TS=tailscale; return 0; fi
  local p
  for p in "/c/Program Files/Tailscale/tailscale.exe" "/Applications/Tailscale.app/Contents/MacOS/Tailscale"; do
    if [[ -x $p ]]; then TS=$p; return 0; fi
  done
  return 1
}

if ! find_ts; then
  case $os in
    linux)   what="run Tailscale's official install script (curl -fsSL https://tailscale.com/install.sh | sh, uses sudo)" ;;
    windows) what="run: winget install --id Tailscale.Tailscale -e (Windows will ask for permission)" ;;
    mac)     die "Install Tailscale from https://tailscale.com/download/mac, open it once, then re-run this script." ;;
    *)       die "Unsupported system. Install Tailscale from https://tailscale.com/download, then re-run this script." ;;
  esac
  info "Tailscale is not installed. This will $what"
  read -r -p "Install it now? [y/N] " yn
  [[ $yn == [yY]* ]] || die "Not installed. Install Tailscale, then re-run."
  case $os in
    linux)   curl -fsSL https://tailscale.com/install.sh | sh || die "install failed" ;;
    windows) winget install --id Tailscale.Tailscale -e --accept-source-agreements --accept-package-agreements || die "winget install failed (try installing from https://tailscale.com/download/windows)" ;;
  esac
  find_ts || die "Installed, but the tailscale command was not found. Open a new terminal and re-run."
fi
info "Using: $TS"

SUDO=""
if [[ $os == linux && ${EUID:-$(id -u)} -ne 0 ]]; then SUDO="sudo"; fi

# ---------------------------------------------------------------- 2. connect
my_ip() { "$TS" ip -4 2>/dev/null | head -n1; }

ip="$(my_ip)"
if [[ -n $ip ]]; then
  info "Already connected to Tailscale as $ip"
else
  up_args=(--hostname "$name")
  if [[ -n ${TS_AUTHKEY:-} ]]; then
    up_args+=(--authkey "$TS_AUTHKEY")
    info "Connecting with the auth key (no login page needed)..."
  else
    info "Connecting. A login link will appear: open it and sign in to the SAME tailnet as your partner."
  fi
  $SUDO "$TS" up "${up_args[@]}" || die "tailscale up failed"
  ip="$(my_ip)"
  [[ -n $ip ]] || die "Connected, but no Tailscale address yet. Wait a few seconds and re-run."
fi
info "This machine's Tailscale address: $ip"

# ---------------------------------------------------------------- 3. role-specific
if [[ $role == consumer ]]; then
  echo
  echo "Done. Next, on this machine:"
  echo "  1. Allow ports 50051 (gRPC) and 8080 (GUI) through this machine's firewall for Tailscale."
  case $os in
    linux)   echo "     e.g.  sudo ufw allow in on tailscale0      (if ufw is on)" ;;
    windows) echo "     Windows will ask the first time consumer / gui start: allow it (Private and Public)." ;;
  esac
  echo "  2. Start the consumer and GUI (see DEMO_SCRIPT.md)."
  echo "  3. Tell your partner to run:  ./deploy/tailscale-setup.sh producer"
  echo "     If that can't find this machine by name ($CONSUMER_NAME), give them this address: $ip"
  exit 0
fi

# producer
target="${2:-}"
if [[ -z $target ]]; then
  target="$("$TS" ip -4 "$CONSUMER_NAME" 2>/dev/null | head -n1)"
  [[ -n $target ]] && info "Found the consumer as $CONSUMER_NAME at $target"
fi
if [[ -z $target ]]; then
  echo "Could not find '$CONSUMER_NAME' in your tailnet (is the consumer connected, in the same tailnet?)."
  read -r -p "Consumer's Tailscale address (100.x.y.z), or Enter to quit: " target
  [[ -n $target ]] || exit 1
fi

# Write CONSUMER_HOST into deploy/.env (create it from the example if needed).
[[ -f deploy/.env ]] || { cp deploy/.env.example deploy/.env && info "Created deploy/.env from the example"; }
if grep -q '^CONSUMER_HOST=' deploy/.env; then
  sed -i.bak "s|^CONSUMER_HOST=.*|CONSUMER_HOST=$target|" deploy/.env && rm -f deploy/.env.bak
else
  echo "CONSUMER_HOST=$target" >> deploy/.env
fi
info "Set CONSUMER_HOST=$target in deploy/.env"

info "Pinging the consumer over Tailscale..."
"$TS" ping -c 3 --timeout 5s "$target" || echo "(no reply: is the consumer machine connected and awake?)"

echo
info "Running the preflight (it will fail on the ports until the consumer and GUI are started)"
./deploy/preflight.sh producer
