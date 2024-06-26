variable "ssh_keys" {
  type = list(string)
}

variable "instance_tags" {
  type    = list(string)
  default = ["Malachite"]
}

resource "digitalocean_droplet" "cc" {
  name      = "cc"
  image     = "debian-12-x64"
  region    = var.region
  tags = concat(var.instance_tags, ["cc"])
  # Build takes about 4.5 minutes on a 4-core Digital Ocean server
  size      = "s-4vcpu-8gb"
  # Build takes about 2.5 minutes on an 8-core Digital Ocean server
  #size      = "s-8vcpu-16gb"
  ssh_keys  = var.ssh_keys
  user_data = templatefile("user-data/cc-data.txt", {
    malachite_dashboard = filebase64("../viewer/config-grafana/provisioning/dashboards-data/main.json")
    node_dashboard      = filebase64("../viewer/config-grafana/provisioning/dashboards-data/node-exporter-full.json")
  })
}

resource "digitalocean_droplet" "small" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.small_nodes
  name       = "small${count.index}"
  image      = "debian-12-x64"
  region     = var.region
  tags       = concat(var.instance_tags, ["small"])
  size       = "s-4vcpu-8gb"
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = count.index
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
}

resource "digitalocean_droplet" "large" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.large_nodes
  name       = "large${count.index}"
  image      = "debian-12-x64"
  region     = var.region
  tags       = concat(var.instance_tags, ["large"])
  size       = "s-8vcpu-16gb"
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = var.small_nodes + count.index
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
}
