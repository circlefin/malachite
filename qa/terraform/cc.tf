resource "random_string" "elastic_password" {
  length           = 30
  special          = false
}

resource tls_private_key ssh {
  algorithm = "ED25519"
}

resource digitalocean_ssh_key cc {
  name = "autossh"
  public_key = tls_private_key.ssh.public_key_openssh
}

resource "digitalocean_droplet" "cc" {
  name      = "cc"
  image     = "debian-12-x64"
  region    = "tor1"
  tags      = concat(var.instance_tags, ["cc", "tor1"])
  size      = var.cc_size
  ssh_keys  = concat(var.ssh_keys, [digitalocean_ssh_key.cc.id])
  user_data = templatefile("user-data/cc-data.txt", {
    prometheus_config = filebase64("../viewer/config-prometheus/prometheus.yml")
    grafana_data_sources = filebase64("../viewer/config-grafana/provisioning/datasources/prometheus.yml")
    grafana_dashboards_config = filebase64("../viewer/config-grafana/provisioning/dashboards/malachite.yml")
    elastic_password = random_string.elastic_password.result
    #ssh_key = tls_private_key.ssh.private_key_openssh
  })
  connection {
    host = digitalocean_droplet.cc.ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }
  provisioner "file" {
    source = "../viewer/config-grafana/provisioning/dashboards-data"
    destination = "/root"
  }
}

resource terraform_data cc-done {
  triggers_replace = [
    local.commands-sh,
    local.etc-hosts,
    digitalocean_droplet.cc.id
  ]

  connection {
    host = digitalocean_droplet.cc.ipv4_address
    timeout = var.ssh_timeout
    private_key = tls_private_key.ssh.private_key_openssh
  }

  provisioner "file" {
    content = local.commands-sh
    destination = "/etc/profile.d/commands.sh"
  }
  provisioner "file" {
    content = local.etc-hosts
    destination = "/etc/hosts"
  }

  provisioner "remote-exec" {
    script = "scripts/cc-done.sh"
  }
}
