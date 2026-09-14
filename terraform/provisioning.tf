resource "null_resource" "setup_container" {
  depends_on = [proxmox_lxc.friends_calendar]

  connection {
    type     = "ssh"
    user     = "root"
    password = var.ct_password
    host     = split("/", var.ct_ip_address)[0]
  }

  # Install dependencies
  provisioner "remote-exec" {
    inline = [
      "apt-get update",
      "apt-get upgrade -y",
      "apt-get install -y curl git build-essential pkg-config libssl-dev postgresql-client nginx certbot python3-certbot-nginx ca-certificates",
      
      # Install Rust
      "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
      "source $HOME/.cargo/env",
      
      # Create app directory
      "mkdir -p /opt/friends-calendar",
    ]
  }

  # Copy application files
  provisioner "file" {
    source      = "../backend"
    destination = "/opt/friends-calendar"
  }

  # Create .env file
  provisioner "file" {
    content = templatefile("${path.module}/templates/env.tpl", {
      database_url        = var.database_url
      discord_client_id   = var.discord_client_id
      discord_client_secret = var.discord_client_secret
      discord_bot_token   = var.discord_bot_token
      discord_channel_id  = var.discord_channel_id
      jwt_secret          = var.jwt_secret
      frontend_url        = var.frontend_url
    })
    destination = "/opt/friends-calendar/.env"
  }

  # Build application
  provisioner "remote-exec" {
    inline = [
      "cd /opt/friends-calendar",
      "source $HOME/.cargo/env",
      "cargo build --release",
    ]
  }

  # Create systemd service
  provisioner "file" {
    content     = file("${path.module}/templates/systemd.service")
    destination = "/etc/systemd/system/friends-calendar.service"
  }

  # Setup Nginx
  provisioner "file" {
    content = templatefile("${path.module}/templates/nginx.conf", {
      domain = var.domain
    })
    destination = "/etc/nginx/sites-available/friends-calendar"
  }

  # Enable and start services
  provisioner "remote-exec" {
    inline = [
      # Enable systemd service
      "systemctl daemon-reload",
      "systemctl enable friends-calendar",
      "systemctl start friends-calendar",
      
      # Setup Nginx
      "ln -sf /etc/nginx/sites-available/friends-calendar /etc/nginx/sites-enabled/",
      "nginx -t",
      "systemctl restart nginx",
    ]
  }
}