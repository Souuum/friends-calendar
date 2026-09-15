# Deployment

Two separate things, deliberately not automated the same way:

- **Provisioning** (create the container, DNS, nginx, systemd, SSL) - rare,
  manual, via Terraform (`make apply`). Run by a person, on purpose.
- **Deploying** (ship a new build of the backend after a code change) -
  frequent, automatic, on every push to `master` that passes CI, via
  `.github/workflows/cd.yml`.

They're split because Terraform has no remote state backend configured
here (`terraform/provider.tf`) - running `terraform apply` unattended from
CI on every push would start from empty state each time and either fail on
a resource-name collision or try to create a second container next to the
real one. Fixing that (a real state backend) is possible later, but for a
single-VPS personal project the simpler and safer answer is: infra changes
are rare enough to just run locally and review before applying.

## One-time provisioning

1. `cd terraform && cp terraform.tfvars.example terraform.tfvars`, fill in
   real values (Proxmox credentials, Cloudflare token/zone, Discord app
   secrets, a real `domain`). Never commit `terraform.tfvars` -
   `.gitignore` already excludes it.
2. **Before running anything**, confirm whether the container already
   exists - SSH to the Proxmox host and run `pct list` (or check the web
   UI) for a container matching `ct_hostname` (`friends-calendar-api` by
   default). If it already exists, `terraform apply` should either import
   it (`terraform import proxmox_lxc.friends_calendar <vmid>`) or you're
   managing it outside Terraform from here on - don't run `apply` blind
   against a container that already has real data/config on it.
3. `make init && make plan` - review the plan output carefully, this
   creates a real LXC container, a real Cloudflare DNS record, and runs
   real provisioning commands over SSH (apt install, Rust toolchain, first
   `cargo build --release`, nginx, certbot).
4. `make apply` when the plan looks right.
5. `make output` to get the container IP; `make status` to confirm the
   service came up.

This also does the **first** deploy (provisioning.tf builds once on the
container itself). Everything after that goes through CI/CD below.

## Automatic deployment (`.github/workflows/cd.yml`)

On every push to `master`: `ci.yml` runs first (fmt, clippy, backend
tests, frontend tests, `cargo build --release` as a compile check). If it
passes, `cd.yml` triggers (`workflow_run`, gated on
`conclusion == 'success'`) and:

1. Builds the release binary in a `rust:1-bookworm` container (matches the
   container template's glibc - a binary built on plain `ubuntu-latest`
   would refuse to run on the deploy target, `GLIBC_x.y not found`).
2. Uploads it as a build artifact.
3. A second job downloads it, `scp`s it to the VPS as
   `rust-friends-calendar.new`, then over one SSH connection atomically
   `mv`s it into place and `systemctl restart friends-calendar`.
4. Curls the real domain a few times with backoff; fails the job loudly if
   the service doesn't come back up.

Nothing here touches Terraform, DNS, nginx config, or the systemd unit
file - those only change via a manual `terraform apply` (step above). The
`.env` file on the VPS is never touched by a deploy either - only the
binary changes. This also means: a change that needs a new env var
(new Discord scope, new config) needs a manual step on the VPS (edit
`/opt/friends-calendar/.env`, `systemctl restart friends-calendar`) or a
`terraform apply` if you've added it to `variables.tf`/`env.tpl` - a plain
git push won't pick it up on its own.

### Required setup (one-time, before the pipeline can deploy anything)

1. **Create the GitHub Environment**: repo Settings → Environments → New
   environment → name it exactly `production` (`cd.yml`'s `deploy` job
   references this - GitHub validates the name exists, it won't silently
   no-op).
2. **Generate a dedicated deploy key** - don't reuse your personal
   `~/.ssh/id_rsa` (that's what `terraform/main.tf`'s
   `ssh_public_keys = file("~/.ssh/id_rsa.pub")` line currently
   provisions the container with, for the *human* Terraform/`scripts/*.sh`
   path):
   ```
   ssh-keygen -t ed25519 -f ~/.ssh/friends_calendar_deploy -C "github-actions-deploy" -N ""
   ```
   Append `~/.ssh/friends_calendar_deploy.pub` to the container's
   `/root/.ssh/authorized_keys` (or whatever user you intend `DEPLOY_SSH_USER`
   to be - see the note in `cd.yml` about that user needing to write to
   `/opt/friends-calendar` and restart the systemd unit without a sudo
   password prompt, which is only guaranteed out of the box for `root`).
3. **Add repo secrets** (Settings → Secrets and variables → Actions,
   scoped to the `production` environment or repo-wide, your call):
   - `DEPLOY_SSH_HOST` - the container's IP or domain
   - `DEPLOY_SSH_USER` - almost certainly `root`, matching how
     provisioning already operates
   - `DEPLOY_SSH_KEY` - the **private** half of the key from step 2, full
     contents including the `-----BEGIN...-----`/`-----END...-----` lines
   - `PROD_DOMAIN` - the real domain (no `https://` prefix - the workflow
     adds that), used only for the post-deploy health check

## Known gaps (not built here, flagging rather than guessing at scope)

- **No rollback.** A bad deploy replaces the only copy of the binary in
  place; there's no "previous version" kept on the VPS or in CI to roll
  back to. If this becomes a real problem, the cheapest fix is keeping the
  last N binaries in `/opt/friends-calendar/releases/` and symlinking
  `current` to the active one - not built now because nothing asked for
  it yet.
- **No staging environment.** Dropped deliberately (see CLAUDE.md) - this
  is a single personal VPS, and nothing indicated a staging deploy target
  was actually in use.
- **DB migrations run at process startup** (`sqlx::migrate!` in
  `AppState::new()`, `backend/src/config.rs`), not as a separate deploy
  step - a migration that needs to run before the new binary can safely
  serve traffic will run automatically on restart, but there's no
  pre-flight check or maintenance-window handling here. For this app's
  scale that's an acceptable tradeoff, not an oversight.
