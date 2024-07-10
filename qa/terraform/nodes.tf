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
  region    = var.region_a
  tags      = concat(var.instance_tags, ["cc"])
  size      = "g-4vcpu-16gb"
  ssh_keys  = var.ssh_keys
  user_data = templatefile("user-data/cc-data.txt", {
    malachite_dashboard = filebase64("../viewer/config-grafana/provisioning/dashboards-data/main.json")
  })
}

resource "digitalocean_droplet" "small_a" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.small_a
  name       = "node${count.index}"
  image      = "debian-12-x64"
  region     = var.region_a
  tags       = concat(var.instance_tags, ["small", var.region_a])
  size       = "g-2vcpu-8gb"
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

resource "digitalocean_droplet" "large_a" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.large_a
  name       = "node${var.small_a + count.index}"
  image      = "debian-12-x64"
  region     = var.region_a
  tags       = concat(var.instance_tags, ["large", var.region_a])
  size       = "g-4vcpu-16gb"
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = var.small_a + count.index
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
}

resource "digitalocean_droplet" "small_b" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.small_b
  name       = "node${var.small_a + var.large_a + count.index}"
  image      = "debian-12-x64"
  region     = var.region_b
  tags       = concat(var.instance_tags, ["small", var.region_b])
  size       = "g-2vcpu-8gb"
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = var.small_a + var.large_a + count.index
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
}

resource "digitalocean_droplet" "large_b" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.large_b
  name       = "node${var.small_a + var.large_a + var.small_b + count.index}"
  image      = "debian-12-x64"
  region     = var.region_b
  tags       = concat(var.instance_tags, ["large", var.region_b])
  size       = "g-4vcpu-16gb"
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = var.small_a + var.large_a + var.small_b + count.index
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
}

resource "digitalocean_droplet" "small_c" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.small_c
  name       = "node${var.small_a + var.large_a + var.small_b + var.large_b + count.index}"
  image      = "debian-12-x64"
  region     = var.region_c
  tags       = concat(var.instance_tags, ["small", var.region_c])
  size       = "g-2vcpu-8gb"
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = var.small_a + var.large_a + var.small_b + var.large_b + count.index
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
}

resource "digitalocean_droplet" "large_c" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.large_c
  name       = "node${var.small_a + var.large_a + var.small_b + var.large_b + var.small_c + count.index}"
  image      = "debian-12-x64"
  region     = var.region_c
  tags       = concat(var.instance_tags, ["large", var.region_c])
  size       = "g-4vcpu-16gb"
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = var.small_a + var.large_a + var.small_b + var.small_b + var.large_b + var.small_c + count.index
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
}
