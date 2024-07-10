resource "local_file" "commands" {
  content = templatefile("templates/commands.tmpl", {
    path     = abspath(path.root),
    region_a = var.region_a,
    region_b = var.region_b,
    region_c = var.region_c,
    ips      = [
      for node in concat(digitalocean_droplet.small_a, digitalocean_droplet.large_a, digitalocean_droplet.small_b, digitalocean_droplet.large_b, digitalocean_droplet.small_c, digitalocean_droplet.large_c) :
      node.ipv4_address
    ],
    nodes = [
      for node in concat(digitalocean_droplet.small_a, digitalocean_droplet.large_a, digitalocean_droplet.small_b, digitalocean_droplet.large_b, digitalocean_droplet.small_c, digitalocean_droplet.large_c) :
      {
        name        = node.name,
        ip          = node.ipv4_address,
        internal_ip = node.ipv4_address_private
      }
    ],
    cc = {
      name        = digitalocean_droplet.cc.name
      ip          = digitalocean_droplet.cc.ipv4_address
      internal_ip = digitalocean_droplet.cc.ipv4_address_private
    }
  })
  filename        = "commands.sh"
  file_permission = "0644"
}
