resource "null_resource" "setup_ssl" {
  depends_on = [
    null_resource.setup_container,
    cloudflare_record.api
  ]

  connection {
    type     = "ssh"
    user     = "root"
    password = var.ct_password
    host     = split("/", var.ct_ip_address)[0]
  }

  # Wait for DNS propagation
  provisioner "local-exec" {
    command = "sleep 30"
  }

  # Get SSL certificate
  provisioner "remote-exec" {
    inline = [
      "certbot --nginx -d ${var.domain} --non-interactive --agree-tos --email admin@${data.cloudflare_zone.main.name}",
      "systemctl enable certbot.timer",
      "systemctl start certbot.timer",
    ]
  }
}