resource "digitalocean_project" "malachite-testnet" {
  name        = "malachite-testnet"
  description = "A project to test the Malachite codebase."
  resources   = concat([for node in digitalocean_droplet.small: node.urn], [for node in digitalocean_droplet.large: node.urn], [digitalocean_droplet.cc.urn])
}
