#!/usr/bin/env bash
#
# Provision a fresh Debian 12 LXC container to run the Friends Calendar API.
#
# Run this ON the container, as root:
#     bash provision-container.sh
#
# What it deliberately does NOT install:
#   - Rust. The release binary is built by GitHub Actions and shipped in;
#     building on a 2-core/2GB container is slow and can OOM on this
#     dependency set (that is why cd.yml builds in CI in the first place).
#   - nginx. Cloudflare terminates TLS at the edge and `cloudflared`
#     forwards straight to the app's port, so a local reverse proxy would
#     be a third hop that adds nothing.
#   - certbot. There is no public port 80 on this host to answer an
#     HTTP-01 challenge, and the edge certificate is Cloudflare's.
#
# It is safe to re-run: every step is idempotent, and an existing .env is
# never overwritten.

set -euo pipefail

# Overridable only so the script can be smoke-tested against stubs; in
# real use these are always the defaults, and cd.yml / the systemd unit
# both hard-code the same paths.
APP_DIR="${APP_DIR:-/opt/friends-calendar}"
SYSTEMD_DIR="${SYSTEMD_DIR:-/etc/systemd/system}"
DB_NAME=friends_calendar
DB_USER=friends_calendar
ENV_FILE="$APP_DIR/.env"

log() { printf '\n\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[!]\033[0m %s\n' "$*"; }

if [ "$(id -u)" -ne 0 ] && [ -z "${PROVISION_SMOKE_TEST:-}" ]; then
  echo "Run as root (this installs packages and writes to /opt and /etc)." >&2
  exit 1
fi

log "Installing packages"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
# ca-certificates is not optional: every Discord API call is HTTPS, and a
# minimal container image has no root store, which fails at runtime rather
# than at install time.
apt-get install -y -qq postgresql ca-certificates curl >/dev/null

log "Starting PostgreSQL"
systemctl enable --now postgresql

log "Creating database and role"
# Generated here and never printed in full: the only copy that matters is
# the one written into .env below.
DB_PASSWORD="$(openssl rand -hex 24)"

if su - postgres -c "psql -tAc \"SELECT 1 FROM pg_roles WHERE rolname='$DB_USER'\"" | grep -q 1; then
  warn "Role $DB_USER already exists - leaving its password alone."
  DB_PASSWORD=""
else
  su - postgres -c "psql -q -c \"CREATE ROLE $DB_USER LOGIN PASSWORD '$DB_PASSWORD'\""
fi

if su - postgres -c "psql -tAc \"SELECT 1 FROM pg_database WHERE datname='$DB_NAME'\"" | grep -q 1; then
  warn "Database $DB_NAME already exists - not recreating it."
else
  su - postgres -c "createdb -O $DB_USER $DB_NAME"
fi

log "Creating $APP_DIR"
# The binary lands in target/release because that is the path the systemd
# unit and cd.yml's scp both use - it mirrors a cargo layout even though
# nothing is ever built here.
mkdir -p "$APP_DIR/target/release"

log "Writing $ENV_FILE"
if [ -f "$ENV_FILE" ]; then
  warn "$ENV_FILE exists - leaving it untouched."
  warn "Delete it first if you want this script to regenerate it."
else
  if [ -z "$DB_PASSWORD" ]; then
    echo "The role existed but .env did not, so its password is unknown." >&2
    echo "Reset it, then write DATABASE_URL by hand:" >&2
    echo "  su - postgres -c \"psql -c \\\"ALTER ROLE $DB_USER PASSWORD 'new'\\\"\"" >&2
    exit 1
  fi
  # Scoped, and restored below: this file holds a database password and a
  # JWT signing key, so it must never exist even briefly as world-readable.
  # Leaving the umask set would silently apply to everything after it.
  previous_umask="$(umask)"
  umask 077
  cat > "$ENV_FILE" <<EOF
DATABASE_URL=postgres://$DB_USER:$DB_PASSWORD@localhost:5432/$DB_NAME

# Generated here, not reused from any other environment.
JWT_SECRET=$(openssl rand -hex 32)

# 0.0.0.0 only if cloudflared runs somewhere other than this container.
# Loopback is the safer default; see the note in backend/src/main.rs.
BIND_ADDR=127.0.0.1:8080

# --- Fill these in before starting the service -----------------------
#
# All five Discord values are the SAME ones as local development - copy them
# from backend/.env. They identify the Discord application, not the machine.
#
# Blank is not "unset": the server refuses to start on an empty required
# value rather than failing later at Discord with an unrelated-looking
# error. DISCORD_CLIENT_ID and DISCORD_CLIENT_SECRET are required; the
# other three degrade gracefully (no bot, no friend sync, no announcements).
DISCORD_CLIENT_ID=
DISCORD_CLIENT_SECRET=
DISCORD_BOT_TOKEN=
DISCORD_GUILD_ID=
DISCORD_ANNOUNCEMENT_CHANNEL_ID=

# This API's own public URL. The OAuth callback is built from it and sent to
# Discord as redirect_uri, so "<this>/api/auth/callback" must ALSO be
# registered under OAuth2 -> Redirects in the Discord developer portal, or
# Discord rejects the login before it starts.
#   e.g. https://api.example.com
PUBLIC_API_URL=

# Where the browser lands after authorising, and the origin allowed through
# CORS. This is the *frontend's* URL, not this API's.
#   e.g. https://calendar.example.com
FRONTEND_URL=
EOF
  chmod 600 "$ENV_FILE"
  umask "$previous_umask"
fi

log "Installing the systemd unit"
cat > "$SYSTEMD_DIR/friends-calendar.service" <<'EOF'
[Unit]
Description=Friends Calendar API
After=network.target postgresql.service
# The app runs sqlx migrations at startup, so a database that isn't up yet
# means a crash-loop until it is. Restart=always would recover eventually,
# but ordering it properly makes the first boot clean.
Requires=postgresql.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/friends-calendar
Environment="RUST_LOG=info"
# .env is read by dotenvy relative to WorkingDirectory above.
ExecStart=/opt/friends-calendar/target/release/rust-friends-calendar
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# Explicit rather than umask-dependent: unit files are conventionally 644,
# and this one holds no secrets.
chmod 644 "$SYSTEMD_DIR/friends-calendar.service"

systemctl daemon-reload
systemctl enable friends-calendar >/dev/null

log "Done"
cat <<EOF

Next steps, in order:

  1. Fill in the Discord values and FRONTEND_URL in:
         $ENV_FILE

  2. Add this container's SSH key access for the deploy runner, then push
     to master - CI builds the binary and the runner ships it here.

  3. The service is enabled but NOT started: there is no binary yet, so
     starting now would only crash-loop. The first deploy starts it.

     Once deployed, check it with:
         systemctl status friends-calendar
         journalctl -u friends-calendar -f

  4. Point a Cloudflare tunnel route at this container. If cloudflared runs
     on the Proxmox host rather than in here, set BIND_ADDR=0.0.0.0:8080
     in .env first, or it will only ever listen on loopback.

EOF
