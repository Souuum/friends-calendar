# Get zone info
data "cloudflare_zone" "main" {
  zone_id = var.cloudflare_zone_id
}

# Create DNS A record
resource "cloudflare_record" "api" {
  zone_id = var.cloudflare_zone_id
  name    = var.domain
  value   = split("/", var.ct_ip_address)[0]
  type    = "A"
  ttl     = 1
  proxied = true

  depends_on = [proxmox_lxc.friends_calendar]
}

# SSL/TLS settings
resource "cloudflare_zone_settings_override" "main" {
  zone_id = var.cloudflare_zone_id

  settings {
    ssl                      = "flexible"
    always_use_https         = "on"
    min_tls_version          = "1.2"
    automatic_https_rewrites = "on"
  }
}