resource "digitalocean_project" "malachite-testnet" {
  name = "malachite-testnet"
  description = "A project to test the Malachite codebase."
  resources = concat([
    for node in concat(digitalocean_droplet.ams3, digitalocean_droplet.blr1, digitalocean_droplet.fra1, digitalocean_droplet.lon1, digitalocean_droplet.nyc1, digitalocean_droplet.nyc3, digitalocean_droplet.sfo2, digitalocean_droplet.sfo3, digitalocean_droplet.sgp1, digitalocean_droplet.syd1, digitalocean_droplet.tor1) :
    node.urn
  ], [digitalocean_droplet.cc.urn])
}
