# ⚠️ Not used by the current deployment

This directory has **never been applied** - there is no state file, no
`terraform.tfvars`, and no `.terraform/`. It was written speculatively and
audited once, but it does not describe how this app is actually deployed.
See `docs/deployment.md` for what is real.

Do not run `terraform apply` here expecting it to work. Concretely, against
the real setup (home Proxmox behind NAT, exposed via a Cloudflare tunnel):

| File | Why it doesn't apply |
|---|---|
| `provisioning.tf` | `remote-exec`/`file` provisioners SSH to the container's **private** IP. Unreachable from anywhere but that LAN, so this can't run from a laptop elsewhere. |
| `provisioning.tf` | `source = "../backend"` has no trailing slash, so it lands in `/opt/friends-calendar/backend/` while the build step does `cd /opt/friends-calendar` - the build would fail. It would also upload ~1.1GB of local macOS `target/` and the real `backend/.env`. |
| `provisioning.tf` | Installs Rust and builds **on** the container. CI builds the binary now; the container needs no toolchain. |
| `ssl.tf` | `certbot --nginx` needs a public port 80 for the HTTP-01 challenge. There isn't one. The edge certificate is Cloudflare's, and an origin cert would never be used. |
| `cloudflare.tf` | Points an A record at `ct_ip_address` - a private address, unroutable. The tunnel provides the public hostname instead. |
| `cloudflare.tf` | `ssl = "flexible"` contradicts `certbot`: flexible means Cloudflare reaches the origin over plain HTTP, so the origin certificate is unused. |
| `main.tf` | `ssh_public_keys = file("~/.ssh/id_rsa.pub")` fails at plan time on a machine that only has an ed25519 key. |

## The part that is still worth something

The Proxmox API **is** reachable through the tunnel -
`https://proxmox.<domain>/api2/json/version` answers `401`, meaning it
responds and only wants credentials. So `proxmox_lxc` resource *creation*
could be driven remotely with:

```hcl
proxmox_api_url = "https://proxmox.<domain>/api2/json"
```

If container creation is ever worth automating, that is the piece to keep.
Everything downstream of creation - provisioning, SSL, DNS - is handled
differently now and should be deleted rather than fixed.
