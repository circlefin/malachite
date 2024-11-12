locals {
  nodes = [
    for node in concat(digitalocean_droplet.ams3, digitalocean_droplet.blr1, digitalocean_droplet.fra1, digitalocean_droplet.lon1, digitalocean_droplet.nyc1, digitalocean_droplet.nyc3, digitalocean_droplet.sfo2, digitalocean_droplet.sfo3, digitalocean_droplet.sgp1, digitalocean_droplet.syd1, digitalocean_droplet.tor1) :
    {
      name        = node.name,
      ip          = node.ipv4_address,
      internal_ip = node.ipv4_address_private
    }
  ]
  ips = [
    for node in local.nodes :
    node.ip
  ]
  cc = {
    name        = digitalocean_droplet.cc.name
    ip          = digitalocean_droplet.cc.ipv4_address
    internal_ip = digitalocean_droplet.cc.ipv4_address_private
  }
  etc-hosts = templatefile("templates/hosts.tmpl", {
    nodes = local.nodes,
    cc    = local.cc
  })
  commands-sh = templatefile("templates/commands.tmpl", {
    path      = abspath(path.root),
    ips       = local.ips,
    nodes     = local.nodes,
    cc        = local.cc
  })
}

resource "local_file" "commands" {
  content = local.commands-sh
  filename        = "commands.sh"
  file_permission = "0644"
}
