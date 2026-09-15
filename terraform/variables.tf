# Proxmox variables
variable "proxmox_api_url" {
  description = "Proxmox API URL"
  type        = string
  default     = "https://proxmox:8006/api2/json"
}

variable "proxmox_user" {
  description = "Proxmox user"
  type        = string
  default     = "root@pam"
}

variable "proxmox_password" {
  description = "Proxmox password"
  type        = string
  sensitive   = true
}

variable "proxmox_tls_insecure" {
  description = "Skip TLS verification"
  type        = bool
  default     = true
}

variable "proxmox_node" {
  description = "Proxmox node name"
  type        = string
  default     = "pve"
}

# Container variables
variable "ct_hostname" {
  description = "Container hostname"
  type        = string
  default     = "friends-calendar-api"
}

variable "ct_password" {
  description = "Container root password"
  type        = string
  sensitive   = true
}

variable "ct_template" {
  description = "Container template"
  type        = string
  default     = "local:vztmpl/debian-12-standard_12.7-1_amd64.tar.zst"
}

variable "ct_cores" {
  description = "Number of CPU cores"
  type        = number
  default     = 2
}

variable "ct_memory" {
  description = "Memory in MB"
  type        = number
  default     = 2048
}

variable "ct_swap" {
  description = "Swap in MB"
  type        = number
  default     = 512
}

variable "ct_disk_size" {
  description = "Disk size"
  type        = string
  default     = "20G"
}

variable "ct_storage" {
  description = "Storage pool"
  type        = string
  default     = "local-lvm"
}

# Network variables
variable "ct_ip_address" {
  description = "Container IP address with CIDR"
  type        = string
  default     = "192.168.1.100/24"
}

variable "ct_gateway" {
  description = "Network gateway"
  type        = string
  default     = "192.168.1.1"
}

variable "ct_nameserver" {
  description = "DNS nameserver"
  type        = string
  default     = "1.1.1.1"
}

variable "ct_bridge" {
  description = "Network bridge"
  type        = string
  default     = "vmbr0"
}

# Application variables
variable "domain" {
  description = "Domain name"
  type        = string
  default     = "api.yourdomain.com"
}

# Cloudflare variables
variable "cloudflare_api_token" {
  description = "Cloudflare API token"
  type        = string
  sensitive   = true
}

variable "cloudflare_zone_id" {
  description = "Cloudflare zone ID"
  type        = string
}

# Application secrets
variable "discord_client_id" {
  description = "Discord OAuth2 client ID"
  type        = string
  sensitive   = true
}

variable "discord_client_secret" {
  description = "Discord OAuth2 client secret"
  type        = string
  sensitive   = true
}

variable "discord_bot_token" {
  description = "Discord bot token"
  type        = string
  sensitive   = true
}

# The guild the bot lives in - required for friend sync
# (services::friends), /api/discord/config resolution, and the weekly
# digest job (services::digest). Without this, those features return a
# clear 400 rather than crashing (AppState reads it as Option<String>),
# but a real deploy should still set it.
variable "discord_guild_id" {
  description = "Discord guild (server) ID the bot lives in"
  type        = string
}

variable "discord_channel_id" {
  description = "Discord announcement channel ID"
  type        = string
}

variable "jwt_secret" {
  description = "JWT secret key"
  type        = string
  sensitive   = true
}

variable "database_url" {
  description = "PostgreSQL database URL"
  type        = string
  sensitive   = true
}

variable "frontend_url" {
  description = "Frontend URL"
  type        = string
  default     = "https://yourdomain.com"
}