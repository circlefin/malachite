variable "do_token" {}
variable "node_count" {
  default = 3  # Number of nodes to create
}
variable "do_ssh_fingerprint" {}

provider "digitalocean" {
  token = var.do_token
}

resource "digitalocean_droplet" "nodes" {
  count  = var.node_count
  name   = "test-ssh-node-${count.index + 1}"
  region = "nyc1"
  size   = "s-1vcpu-1gb"
  image  = "ubuntu-24-04-x64"
  ssh_keys = [var.do_ssh_fingerprint]
}

output "droplet_ips" {
  value = digitalocean_droplet.nodes[*].ipv4_address
}
