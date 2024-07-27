resource "digitalocean_droplet" "ams3" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.ams3
  name       = "ams3-${count.index}"
  image      = "debian-12-x64"
  region     = "ams3"
  tags       = concat(var.instance_tags, ["ams3", "ams3-${count.index}"])
  size       = var.ams3_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "ams3-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "blr1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.blr1
  name       = "blr1-${count.index}"
  image      = "debian-12-x64"
  region     = "blr1"
  tags       = concat(var.instance_tags, ["blr1", "blr1-${count.index}"])
  size       = var.blr1_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "blr1-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "fra1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.fra1
  name       = "fra1-${count.index}"
  image      = "debian-12-x64"
  region     = "fra1"
  tags       = concat(var.instance_tags, ["fra1", "fra1-${count.index}"])
  size       = var.fra1_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "fra1-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "lon1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.lon1
  name       = "lon1-${count.index}"
  image      = "debian-12-x64"
  region     = "lon1"
  tags       = concat(var.instance_tags, ["lon1", "lon1-${count.index}"])
  size       = var.lon1_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "lon1-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "nyc1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.nyc1
  name       = "nyc1-${count.index}"
  image      = "debian-12-x64"
  region     = "nyc1"
  tags       = concat(var.instance_tags, ["nyc1", "nyc1-${count.index}"])
  size       = var.nyc1_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "nyc1-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "nyc3" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.nyc3
  name       = "nyc3-${count.index}"
  image      = "debian-12-x64"
  region     = "nyc3"
  tags       = concat(var.instance_tags, ["nyc3", "nyc3-${count.index}"])
  size       = var.nyc3_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "nyc3-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "sfo2" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.sfo2
  name       = "sfo2-${count.index}"
  image      = "debian-12-x64"
  region     = "sfo2"
  tags       = concat(var.instance_tags, ["sfo2", "sfo2-${count.index}"])
  size       = var.sfo2_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "sfo2-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "sfo3" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.sfo3
  name       = "sfo3-${count.index}"
  image      = "debian-12-x64"
  region     = "sfo3"
  tags       = concat(var.instance_tags, ["sfo3", "sfo3-${count.index}"])
  size       = var.sfo3_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "sfo3-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "sgp1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.sgp1
  name       = "sgp1-${count.index}"
  image      = "debian-12-x64"
  region     = "sgp1"
  tags       = concat(var.instance_tags, ["sgp1", "sgp1-${count.index}"])
  size       = var.sgp1_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "sgp1-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "syd1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.syd1
  name       = "syd1-${count.index}"
  image      = "debian-12-x64"
  region     = "syd1"
  tags       = concat(var.instance_tags, ["syd1", "syd1-${count.index}"])
  size       = var.syd1_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "syd1-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}

resource "digitalocean_droplet" "tor1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.tor1
  name       = "tor1-${count.index}"
  image      = "debian-12-x64"
  region     = "tor1"
  tags       = concat(var.instance_tags, ["tor1", "tor1-${count.index}"])
  size       = var.tor1_size
  ssh_keys   = var.ssh_keys
  user_data  = templatefile("user-data/user-data.txt", {
    id = "tor1-${count.index}"
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
    elastic_password = random_string.elastic_password.result
  })
}
