resource "local_file" "commands" {
  depends_on = [
    digitalocean_droplet.cc,
    digitalocean_droplet.small,
    digitalocean_droplet.large,
  ]
  content = templatefile("templates/commands.tmpl", {
    path   = abspath(path.root),
    region = var.region,
    ips    = [
      for node in concat(digitalocean_droplet.small, digitalocean_droplet.large) : node.ipv4_address
    ],
    small = [
      for node in digitalocean_droplet.small : {
        name        = node.name,
        ip          = node.ipv4_address,
        internal_ip = node.ipv4_address_private
      }
    ],
    large = [
      for node in digitalocean_droplet.large : {
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
