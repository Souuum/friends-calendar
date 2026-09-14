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