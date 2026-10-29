#!/usr/bin/env bash
#
# Create a development container for the builder/verifier agents
# (.claude/agents/), with everything CI runs available locally.
#
# Run this ON THE PROXMOX HOST, as root:
#     bash scripts/provision-dev-container.sh
#
# Why a separate container, rather than working on the host or in CT 100:
#   - The host has no toolchain, so nothing can be compiled or tested before
#     it is pushed, and every backend change goes to CI blind.
#   - CT 100 runs the live API and CT 101 the live database. `#[sqlx::test]`
#     creates and drops scratch databases, and a Rust build takes gigabytes of
#     disk and every core, none of which belongs next to production.
#   - Agents should not run as root on the hypervisor, next to a saved GitHub
#     token that can push to the repo.
#
# What ends up inside (Debian 12, matching CI's Linux and production):
#   Rust stable + clippy + rustfmt, cargo-audit, Node 22 + Yarn 1, Postgres 15
#   with a CREATEDB role for #[sqlx::test], Playwright's Chromium, and the repo
#   cloned with dependencies installed, all owned by an unprivileged `dev` user.
#
# What it deliberately does NOT do:
#   - Store any GitHub credential. Add a fine-grained token yourself (see the
#     next steps it prints), scoped to this one repository.
#   - Install Claude Code. Use the installer from the official docs, as `dev`.
#   - Copy production secrets. backend/.env here holds only DATABASE_URL.
#
# Re-running is safe: an existing container is reused, and every step inside
# it skips work already done.

set -euo pipefail

CTID="${CTID:-$(pvesh get /cluster/nextid)}"
CT_HOSTNAME="${CT_HOSTNAME:-friends-calendar-dev}"
STORAGE="${STORAGE:-local-lvm}"
DISK_GB="${DISK_GB:-40}"        # Rust target dirs alone reach ~10 GB.
CORES="${CORES:-4}"
MEMORY_MB="${MEMORY_MB:-8192}"  # The backend's test build OOMs below ~4 GB.
SWAP_MB="${SWAP_MB:-2048}"
BRIDGE="${BRIDGE:-vmbr0}"
IP="${IP:-dhcp}"                # Or e.g. IP=192.168.1.110/24 GW=192.168.1.254
GW="${GW:-}"
REPO_URL="${REPO_URL:-https://github.com/souuum/friends-calendar.git}"
NODE_VERSION="${NODE_VERSION:-22.12.0}"
CARGO_AUDIT_VERSION="${CARGO_AUDIT_VERSION:-0.22.2}"

log() { printf '\n\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[!]\033[0m %s\n' "$*"; }

if [ "$(id -u)" -ne 0 ] || ! command -v pct >/dev/null; then
  echo "Run as root on the Proxmox host (needs pct)." >&2
  exit 1
fi

# Never reuse a production container by mistake.
case "$CTID" in
  100 | 101) echo "CTID $CTID is production. Pick another (or leave CTID unset)." >&2; exit 1 ;;
esac

if pct status "$CTID" >/dev/null 2>&1; then
  if [ "$(pct config "$CTID" | sed -n 's/^hostname: //p')" != "$CT_HOSTNAME" ]; then
    echo "CT $CTID exists and isn't $CT_HOSTNAME. Refusing to touch it." >&2
    exit 1
  fi
  log "Reusing existing CT $CTID ($CT_HOSTNAME)"
  pct status "$CTID" | grep -q running || pct start "$CTID"
else
  TEMPLATE="${TEMPLATE:-$(pveam list local | awk '/debian-12-standard/ {print $1}' | sort -V | tail -1)}"
  if [ -z "$TEMPLATE" ]; then
    echo "No Debian 12 template found. Run: pveam update && pveam download local debian-12-standard_12.7-1_amd64.tar.zst" >&2
    exit 1
  fi
  NET="name=eth0,bridge=$BRIDGE,firewall=1,ip=$IP"
  [ -n "$GW" ] && NET="$NET,gw=$GW"

  log "Creating CT $CTID ($CT_HOSTNAME) from $TEMPLATE"
  # Unprivileged, with nesting so systemd works on Debian 12. Not started on
  # boot: it's a workbench, not a service.
  pct create "$CTID" "$TEMPLATE" \
    --hostname "$CT_HOSTNAME" \
    --unprivileged 1 \
    --features nesting=1 \
    --cores "$CORES" --memory "$MEMORY_MB" --swap "$SWAP_MB" \
    --rootfs "$STORAGE:$DISK_GB" \
    --net0 "$NET" \
    --onboot 0 \
    --start 1
fi

log "Waiting for the network"
for _ in $(seq 1 30); do
  pct exec "$CTID" -- getent hosts deb.debian.org >/dev/null 2>&1 && break
  sleep 2
done
pct exec "$CTID" -- getent hosts deb.debian.org >/dev/null || { echo "No DNS inside CT $CTID." >&2; exit 1; }

log "Provisioning inside CT $CTID"
pct exec "$CTID" -- env \
  REPO_URL="$REPO_URL" NODE_VERSION="$NODE_VERSION" CARGO_AUDIT_VERSION="$CARGO_AUDIT_VERSION" \
  bash -s <<'INSIDE'
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
log() { printf '\n\033[1;34m  ->\033[0m %s\n' "$*"; }
as_dev() { runuser -l dev -c "$1"; }

log "System packages"
apt-get update -qq
# build-essential + pkg-config + libssl-dev: openssl-sys, ring and friends
# build C code. postgresql: the scratch databases #[sqlx::test] creates.
apt-get install -y -qq --no-install-recommends \
  build-essential pkg-config libssl-dev ca-certificates curl git xz-utils \
  postgresql python3 >/dev/null

id dev >/dev/null 2>&1 || useradd --create-home --shell /bin/bash dev

log "Node $NODE_VERSION + Yarn 1"
# The official tarball, checksum-verified, rather than a curl-to-bash
# repository setup. Node 22 because yarn v1 hard-fails on vitest's engines field
# under anything older.
if ! /usr/local/bin/node --version 2>/dev/null | grep -qx "v$NODE_VERSION"; then
  tmp=$(mktemp -d)
  base="https://nodejs.org/dist/v$NODE_VERSION"
  file="node-v$NODE_VERSION-linux-x64.tar.xz"
  curl -fsSL "$base/$file" -o "$tmp/$file"
  curl -fsSL "$base/SHASUMS256.txt" | grep " $file\$" | (cd "$tmp" && sha256sum -c -) >/dev/null
  tar -xJf "$tmp/$file" -C /usr/local --strip-components=1
  rm -rf "$tmp"
fi
command -v yarn >/dev/null || npm install -g --silent yarn@1.22.22

log "Postgres role for tests"
# CREATEDB is what #[sqlx::test] needs: it creates a database per test.
# Not a superuser.
PGPASS_FILE=/home/dev/.dev-db-password
if [ ! -s "$PGPASS_FILE" ]; then
  head -c 32 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24 > "$PGPASS_FILE"
  chown dev:dev "$PGPASS_FILE"; chmod 600 "$PGPASS_FILE"
fi
PGPASS=$(cat "$PGPASS_FILE")
runuser -u postgres -- psql -qtAc "SELECT 1 FROM pg_roles WHERE rolname='dev'" | grep -q 1 \
  || runuser -u postgres -- psql -qc "CREATE ROLE dev LOGIN CREATEDB PASSWORD '$PGPASS'"
runuser -u postgres -- psql -qtAc "SELECT 1 FROM pg_database WHERE datname='friends_calendar_dev'" | grep -q 1 \
  || runuser -u postgres -- createdb -O dev friends_calendar_dev
DATABASE_URL="postgres://dev:$PGPASS@localhost:5432/friends_calendar_dev"
grep -q '^export DATABASE_URL=' /home/dev/.profile \
  || echo "export DATABASE_URL='$DATABASE_URL'" >> /home/dev/.profile

log "Rust (as dev)"
as_dev 'command -v cargo >/dev/null || [ -x ~/.cargo/bin/cargo ] ||
  (curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --component clippy,rustfmt >/dev/null)'

log "cargo-audit $CARGO_AUDIT_VERSION"
# The prebuilt release, not `cargo install`, which compiles for minutes.
if ! cargo-audit --version 2>/dev/null | grep -q "$CARGO_AUDIT_VERSION"; then
  tmp=$(mktemp -d)
  curl -fsSL "https://github.com/rustsec/rustsec/releases/download/cargo-audit/v$CARGO_AUDIT_VERSION/cargo-audit-x86_64-unknown-linux-musl-v$CARGO_AUDIT_VERSION.tgz" \
    | tar -xz -C "$tmp"
  install -m 755 "$(find "$tmp" -name cargo-audit -type f)" /usr/local/bin/cargo-audit
  rm -rf "$tmp"
fi

log "Repository"
as_dev "[ -d ~/friends-calendar/.git ] || git clone -q '$REPO_URL' ~/friends-calendar"
as_dev "cd ~/friends-calendar && git config user.name >/dev/null || git config user.name 'friends-calendar agents'"
# backend/.env: DATABASE_URL only. Tests need nothing else, and production
# secrets have no business on a workbench.
as_dev "[ -f ~/friends-calendar/backend/.env ] || echo 'DATABASE_URL=$DATABASE_URL' > ~/friends-calendar/backend/.env"

log "Frontend dependencies + Chromium"
as_dev 'cd ~/friends-calendar/desktop && yarn install --frozen-lockfile --silent'
# System libraries need root; the browser itself is the dev user's.
(cd /home/dev/friends-calendar/desktop && npx --yes playwright install-deps chromium >/dev/null)
as_dev 'cd ~/friends-calendar/desktop && npx playwright install chromium >/dev/null'

log "Prefetching Rust crates"
as_dev 'cd ~/friends-calendar && cargo fetch -q'
INSIDE

IP_NOW=$(pct exec "$CTID" -- hostname -I | awk '{print $1}')
cat <<EOF

$(log "Done: CT $CTID ($CT_HOSTNAME) at ${IP_NOW:-unknown IP}")

Next steps (as the dev user: pct enter $CTID, then su - dev):

  1. GitHub access, scoped to this repo only. Create a fine-grained token at
     https://github.com/settings/personal-access-tokens/new with repository
     souuum/friends-calendar and Contents + Pull requests (+ Workflows) write,
     then:
         cd ~/friends-calendar && git config credential.helper store && git push --dry-run
     and enter the token once when asked.
  2. Install Claude Code as dev, following the official docs, then start it in
     ~/friends-calendar. The agents are in .claude/agents/ (builder, verifier).
  3. First full check, to warm the build cache (takes a while once):
         cd ~/friends-calendar/backend && cargo test
         cd ~/friends-calendar/desktop && yarn test && yarn test:storybook

Worth doing: a Proxmox firewall rule so CT $CTID can't reach CT 100/101
(192.168.1.104 and the database). Nothing here needs production, and the
agents shouldn't be able to reach it by accident.
EOF
