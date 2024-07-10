resource "digitalocean_project" "malachite-testnet" {
  name = "malachite-testnet"
  description = "A project to test the Malachite codebase."
  resources = concat([
    for node in concat(digitalocean_droplet.small_a, digitalocean_droplet.large_a, digitalocean_droplet.small_b, digitalocean_droplet.large_b, digitalocean_droplet.small_c, digitalocean_droplet.large_c) :
    node.urn
  ], [digitalocean_droplet.cc.urn])
}
