# Deployment

## Topology

```
     GitHub Actions (cloud)              Home network
    ┌──────────────────────┐        ┌──────────────────────────────┐
    │ CI: test + build     │        │  Proxmox host                │
    │ Deploy: build binary │        │   └── LXC: friends-calendar  │
    └──────────┬───────────┘        │         ├── PostgreSQL       │
               │ artifact           │         └── API :8080        │
               ▼                    │                              │
    ┌──────────────────────┐  scp   │  self-hosted Actions runner  │
    │ self-hosted runner ──┼────────┼─▶ (same LAN)                 │
    └──────────────────────┘        └──────────────┬───────────────┘
                                                   │ cloudflared
                                                   ▼
                                          Cloudflare edge (TLS)
                                                   │
                                          api.<your-domain>
```

Two facts drive every decision below:

1. **The container is behind NAT.** It has no public IP and no forwarded
   ports. The only way in from the internet is the Cloudflare tunnel.
2. **A proxied Cloudflare record carries HTTP/S, not SSH.** So a
   GitHub-hosted runner has no route to the container at all - verified:
   port 443 to the tunnel hostname connects, 22 and 8006 are filtered.

Hence a **self-hosted runner on the LAN**: it connects *outbound* to
GitHub, so nothing needs exposing, and it can reach the container directly
over the local network.

### Why not Terraform

`terraform/` is Proxmox-specific and **does not match this setup**. Left in
the tree but not used by any of this:

- Its `remote-exec` provisioners SSH to the container's private IP, which
  is unreachable from any machine not already on that LAN.
- `ssl.tf` runs `certbot --nginx`. There is no public port 80 here to
  answer an HTTP-01 challenge, and the edge certificate is Cloudflare's.
  An origin certificate would never be used.
- `cloudflare.tf` points an A record at `ct_ip_address`, a private address.
- It builds Rust *on the container*; CI builds the binary instead.

Its Proxmox API URL would actually work through the tunnel
(`https://proxmox.<domain>/api2/json` answers 401, so the API is
reachable). If you ever want container *creation* automated, that is the
piece worth keeping - the provisioning half is what doesn't apply.

## One-time setup

### 1. Create the container

Proxmox UI → Create CT. Debian 12, 2 cores / 2GB / 20GB is enough (nothing
compiles on it). Note its LAN IP.

### 2. Provision it

Copy `scripts/provision-container.sh` onto the container and run it as
root. It installs PostgreSQL, creates the database and role, generates
`JWT_SECRET` and the DB password into `/opt/friends-calendar/.env` (mode
600), and installs the systemd unit.

It deliberately installs **no Rust and no nginx**, and leaves the service
enabled but **not started** - there is no binary until the first deploy, so
starting it early only crash-loops.

Then fill in the blanks it leaves in `/opt/friends-calendar/.env`:

| Variable | Value | Required? |
|---|---|---|
| `DISCORD_CLIENT_ID` | same as `backend/.env` | **yes** |
| `DISCORD_CLIENT_SECRET` | same as `backend/.env` | **yes** |
| `DISCORD_BOT_TOKEN` | same as `backend/.env` | no - bot/reminders/announcements/friend-sync off without it |
| `DISCORD_GUILD_ID` | same as `backend/.env` | no - friend sync 400s without it |
| `DISCORD_ANNOUNCEMENT_CHANNEL_ID` | same as `backend/.env` | no - and only a fallback; the `/server` page's DB config wins |
| `PUBLIC_API_URL` | `https://api.<your-domain>` | effectively yes |
| `FRONTEND_URL` | `https://calendar.<your-domain>` | effectively yes |

All five Discord values identify the *application*, not the machine, so
they are the same ones local development uses.

> **Blank is not the same as unset.** `env::var` returns `Ok("")` for
> `FOO=`, so a plain `.expect()` used to accept an unfilled placeholder and
> fail much later at Discord. The required values now refuse to start on an
> empty string, naming the file.

> `FRONTEND_URL` feeds **both** the post-login redirect and the CORS
> allow-list, so it must be the origin the client actually loads from. The
> packaged Tauri origins (`tauri://localhost`, `https://tauri.localhost`)
> and the dev origin are always allowed on top of it.

> `PUBLIC_API_URL` builds the OAuth callback that gets sent to Discord as
> `redirect_uri`. **`<PUBLIC_API_URL>/api/auth/callback` must also be
> registered** in the Discord developer portal under OAuth2 → Redirects, or
> Discord rejects the login before it even starts. It was hard-coded to
> `http://localhost:8080/api/auth/callback` until 2026-09-16, which would
> have sent every production login to the user's own machine.

### 3. Point the tunnel at it

Add a route to the existing `cloudflared`: `api.<your-domain>` →
`http://<container-ip>:8080`.

> If `cloudflared` runs on the **Proxmox host** rather than inside the
> container, set `BIND_ADDR=0.0.0.0:8080` in `.env` first. The app defaults
> to loopback, so a tunnel on another machine cannot reach it. Loopback is
> the default deliberately: widening it should be a decision, not an
> accident.

### 3b. Deploy the frontend to Cloudflare Pages

`desktop/` builds to a static site (`adapter-static`, `fallback:
index.html`), so Pages serves it directly.

Cloudflare dash → **Workers & Pages → Create → Pages → Connect to Git**,
pick this repo, then:

| Setting | Value |
|---|---|
| Production branch | `master` |
| Framework preset | SvelteKit (or None - the fields below are what matter) |
| Build command | `yarn build` |
| Build output directory | `build` |
| Root directory | `desktop` |

**Build output directory is relative to the root directory**, so with root
`desktop` it is `build`, *not* `desktop/build`. Getting this pair wrong is
the usual "build succeeded, site is blank" cause.

Environment variables (Settings → Environment variables, Production):

| Variable | Value | Why |
|---|---|---|
| `VITE_API_URL` | `https://api.<your-domain>` | baked in at **build** time |
| `NODE_VERSION` | `22` | see below |

Then **Custom domains → Set up a custom domain** →
`calendar.<your-domain>`. Pages creates the DNS record itself; the tunnel
is not involved, since Pages hosts these files at the edge.

Finally set `FRONTEND_URL` on the container to that URL and
`systemctl restart friends-calendar`, so the post-login redirect and the
CORS allow-list both point at it.

Three things this repo had to fix before a Pages build could work, all of
which will silently bite again if undone:

- **`NODE_VERSION=22`.** `vitest` declares `engines.node "^22.12.0 || ..."`
  and **yarn v1 treats an incompatible `engines` field as a hard error**
  (npm only warns). A Pages default of Node 18/20 fails at
  `yarn install`, not at build - the same failure that broke CI. There is
  also a `.node-version` at the repo root, but the dashboard variable is
  the one that reliably wins.
- **`desktop/_redirects`** (committed as `desktop/static/_redirects`,
  copied verbatim into `build/`). Nothing is prerendered - `build/`
  contains exactly one HTML file - so without `/* /index.html 200` every
  route except `/` returns Pages' own 404 on a direct link or refresh.
- **`desktop/package-lock.json` deleted.** Both lockfiles were committed;
  Pages picks its package manager by sniffing lockfiles, and the npm one
  was stale. `yarn.lock` is authoritative here.

`VITE_API_URL` being build-time matters: a Pages build without it points
the deployed frontend at `http://localhost:8080`, i.e. each visitor's own
machine. That was also what `LoginScreen` hard-coded until 2026-09-16,
independently of this variable.

The Tauri desktop app keeps working alongside this: it is allowed through
CORS by its own origin, and its login flow can still use the
paste-the-token box.

### 4. Install the self-hosted runner

Repo → Settings → Actions → Runners → New self-hosted runner, and follow
the shown commands on whichever LAN machine will host it (the Proxmox host,
or a small separate container - **not** the app container, so a bad deploy
can't take the runner down with it).

Install it as a service so it survives reboots:

```
sudo ./svc.sh install && sudo ./svc.sh start
```

It needs `ssh`, `scp` and `curl` on PATH. A deploy only happens while it is
online - that is the trade for not exposing anything.

### 5. Give the runner SSH access to the container

On the runner:

```
ssh-keygen -t ed25519 -f ~/.ssh/friends_calendar_deploy -C "actions-deploy" -N ""
ssh-copy-id -i ~/.ssh/friends_calendar_deploy.pub root@<container-ip>
```

`root` because the deploy writes to `/opt/friends-calendar` and runs
`systemctl restart`. A non-root user needs passwordless sudo for exactly
those two commands.

### 6. Create the GitHub Environment and secrets

Settings → Environments → **`production`** (the name is referenced by
`cd.yml`).

| Secret | Value |
|---|---|
| `DEPLOY_SSH_HOST` | the container's **LAN** IP - the runner is local, so never the public hostname |
| `DEPLOY_SSH_USER` | `root` |
| `DEPLOY_SSH_KEY` | contents of `~/.ssh/friends_calendar_deploy` (the **private** half, including the BEGIN/END lines) |
| `PROD_DOMAIN` | `api.<your-domain>`, no scheme - the workflow adds `https://` |

## What a deploy does

Push to `master` → CI runs (fmt, clippy, backend tests, frontend tests,
layout tests, audit). If it passes, `cd.yml`:

1. Builds the release binary on a GitHub-hosted runner in `rust:1-bookworm`
   - glibc-matched to Debian 12, so the binary actually runs on the
   container. Building on plain `ubuntu-latest` produces
   `GLIBC_x.y not found`.
2. The **self-hosted** runner downloads that artifact, `scp`s it in as
   `.new`, then over one SSH connection renames it into place and restarts
   the unit. The rename is atomic, so a failed transfer can never leave a
   half-written binary in the live path.
3. Health-checks `https://$PROD_DOMAIN/` with backoff - that goes out to
   Cloudflare and back through the tunnel, so it exercises the whole path,
   not just the local port.

SSH material is written to `$RUNNER_TEMP`, never `~/.ssh`, and removed
afterwards. On a persistent runner the old `>> ~/.ssh/known_hosts` would
have appended the same host key on every deploy forever.

`.env` on the container is never touched by a deploy. A change needing a
new variable means editing `/opt/friends-calendar/.env` and restarting by
hand - a push alone won't pick it up.

## Before the first deploy

Migrations **010 through 014** will all run on first start
(`sqlx::migrate!` runs at startup, not as a separate step).

On a container provisioned by the script this is a fresh, empty database,
so they simply apply in order. The 014 guard only matters when migrating a
database that already has announced events - see CLAUDE.md. If you ever
restore a dump from elsewhere into this container, `pg_dump` first.

**After the first successful deploy, run a friend sync.** Migration 014
backfills `user_guilds` from `friendships`, and anyone who never went
through a sync won't be in it - under the publication-scoped visibility
rules, a user missing from `user_guilds` sees no published events.
`POST /api/friends/sync` repopulates it from Discord's live member list.

## Known gaps

- **No rollback.** A bad deploy overwrites the only binary. The cheapest
  fix, if this ever bites: keep the last N in
  `/opt/friends-calendar/releases/` and symlink `current`.
- **The runner is a single point of failure.** If it's offline, deploys
  queue rather than fail, and nothing ships until it's back.
- **No staging.** Single personal VPS, deliberately.
- **Migrations run at startup**, with no pre-flight check or maintenance
  window. Acceptable at this scale, but it is a real property: a migration
  that fails takes the service down until it's fixed.
- **The Proxmox UI is publicly reachable.** `proxmox.<domain>` serves its
  login page to anyone. Cloudflare hides the origin IP and absorbs DDoS,
  but the form itself is exposed - a Cloudflare Access policy in front of
  it is a few clicks and worth doing.
