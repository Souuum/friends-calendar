output "container_id" {
  description = "Container ID"
  value       = proxmox_lxc.friends_calendar.id
}

output "container_ip" {
  description = "Container IP address"
  value       = split("/", var.ct_ip_address)[0]
}

output "hostname" {
  description = "Container hostname"
  value       = var.ct_hostname
}

output "api_url" {
  description = "API URL"
  value       = "https://${var.domain}"
}

output "cloudflare_dns" {
  description = "Cloudflare DNS record"
  value       = cloudflare_record.api.hostname
}

output "ssh_command" {
  description = "SSH command to connect to container"
  value       = "ssh root@${split("/", var.ct_ip_address)[0]}"
}

output "proxmox_host" {
  description = "Proxmox host, extracted from proxmox_api_url - used by scripts/backup.sh to SSH into the Proxmox host itself (not the container) for vzdump"
  value       = split(":", split("//", var.proxmox_api_url)[1])[0]
}