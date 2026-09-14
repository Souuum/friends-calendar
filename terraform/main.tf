resource "proxmox_lxc" "friends_calendar" {
  target_node  = var.proxmox_node
  hostname     = var.ct_hostname
  ostemplate   = var.ct_template
  password     = var.ct_password
  unprivileged = true
  onboot       = true
  start        = true

  # Resources
  cores  = var.ct_cores
  memory = var.ct_memory
  swap   = var.ct_swap

  # Root filesystem
  rootfs {
    storage = var.ct_storage
    size    = var.ct_disk_size
  }

  # Network configuration
  network {
    name   = "eth0"
    bridge = var.ct_bridge
    ip     = var.ct_ip_address
    gw     = var.ct_gateway
  }

  nameserver = var.ct_nameserver

  # Enable nesting for Docker if needed
  features {
    nesting = true
  }

  # SSH keys (optional)
  ssh_public_keys = file("~/.ssh/id_rsa.pub")

  # Provisioning happens after creation
  connection {
    type     = "ssh"
    user     = "root"
    password = var.ct_password
    host     = split("/", var.ct_ip_address)[0]
  }

  # Wait for container to be ready
  provisioner "remote-exec" {
    inline = [
      "echo 'Container is ready'"
    ]
  }
}