resource "digitalocean_droplet" "ams3" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.ams3
  name       = "ams3-${count.index}"
  image      = "debian-12-x64"
  region     = "ams3"
  tags       = concat(var.instance_tags, ["ams3", "ams3-${count.index}"])
  size       = var.ams3_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data ams3-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.ams3[*].id
  ]

  count = length(digitalocean_droplet.ams3)

  connection {
    host = digitalocean_droplet.ams3[count.index].ipv4_address
    timeout = "120s"
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "blr1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.blr1
  name       = "blr1-${count.index}"
  image      = "debian-12-x64"
  region     = "blr1"
  tags       = concat(var.instance_tags, ["blr1", "blr1-${count.index}"])
  size       = var.blr1_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data blr1-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.blr1[*].id
  ]

  count = length(digitalocean_droplet.blr1)

  connection {
    host = digitalocean_droplet.blr1[count.index].ipv4_address
    timeout = "120s"
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "fra1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.fra1
  name       = "fra1-${count.index}"
  image      = "debian-12-x64"
  region     = "fra1"
  tags       = concat(var.instance_tags, ["fra1", "fra1-${count.index}"])
  size       = var.fra1_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data fra1-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.fra1[*].id
  ]

  count = length(digitalocean_droplet.fra1)

  connection {
    host = digitalocean_droplet.fra1[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "lon1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.lon1
  name       = "lon1-${count.index}"
  image      = "debian-12-x64"
  region     = "lon1"
  tags       = concat(var.instance_tags, ["lon1", "lon1-${count.index}"])
  size       = var.lon1_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data lon1-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.lon1[*].id
  ]

  count = length(digitalocean_droplet.lon1)

  connection {
    host = digitalocean_droplet.lon1[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "nyc1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.nyc1
  name       = "nyc1-${count.index}"
  image      = "debian-12-x64"
  region     = "nyc1"
  tags       = concat(var.instance_tags, ["nyc1", "nyc1-${count.index}"])
  size       = var.nyc1_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data nyc1-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.nyc1[*].id
  ]

  count = length(digitalocean_droplet.nyc1)

  connection {
    host = digitalocean_droplet.nyc1[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "nyc3" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.nyc3
  name       = "nyc3-${count.index}"
  image      = "debian-12-x64"
  region     = "nyc3"
  tags       = concat(var.instance_tags, ["nyc3", "nyc3-${count.index}"])
  size       = var.nyc3_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data nyc3-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.nyc3[*].id
  ]

  count = length(digitalocean_droplet.nyc3)

  connection {
    host = digitalocean_droplet.nyc3[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "sfo2" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.sfo2
  name       = "sfo2-${count.index}"
  image      = "debian-12-x64"
  region     = "sfo2"
  tags       = concat(var.instance_tags, ["sfo2", "sfo2-${count.index}"])
  size       = var.sfo2_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data sfo2-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.sfo2[*].id
  ]

  count = length(digitalocean_droplet.sfo2)

  connection {
    host = digitalocean_droplet.sfo2[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "sfo3" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.sfo3
  name       = "sfo3-${count.index}"
  image      = "debian-12-x64"
  region     = "sfo3"
  tags       = concat(var.instance_tags, ["sfo3", "sfo3-${count.index}"])
  size       = var.sfo3_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data sfo3-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.sfo3[*].id
  ]

  count = length(digitalocean_droplet.sfo3)

  connection {
    host = digitalocean_droplet.sfo3[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "sgp1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.sgp1
  name       = "sgp1-${count.index}"
  image      = "debian-12-x64"
  region     = "sgp1"
  tags       = concat(var.instance_tags, ["sgp1", "sgp1-${count.index}"])
  size       = var.sgp1_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data sgp1-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.sgp1[*].id
  ]

  count = length(digitalocean_droplet.sgp1)

  connection {
    host = digitalocean_droplet.sgp1[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "syd1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.syd1
  name       = "syd1-${count.index}"
  image      = "debian-12-x64"
  region     = "syd1"
  tags       = concat(var.instance_tags, ["syd1", "syd1-${count.index}"])
  size       = var.syd1_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data syd1-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.syd1[*].id
  ]

  count = length(digitalocean_droplet.syd1)

  connection {
    host = digitalocean_droplet.syd1[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}

resource "digitalocean_droplet" "tor1" {
  depends_on = [digitalocean_droplet.cc]
  count      = var.tor1
  name       = "tor1-${count.index}"
  image      = "debian-12-x64"
  region     = "tor1"
  tags       = concat(var.instance_tags, ["tor1", "tor1-${count.index}"])
  size       = var.tor1_size
  ssh_keys   = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
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

resource terraform_data tor1-done {
  triggers_replace = [
    terraform_data.cc-done.id,
    digitalocean_droplet.tor1[*].id
  ]

  count = length(digitalocean_droplet.tor1)

  connection {
    host = digitalocean_droplet.tor1[count.index].ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "remote-exec" {
    script = "scripts/node-done.sh"
  }
}
