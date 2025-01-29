terraform {
  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"  # ✅ Correct provider source
      version = "~> 2.0"  # Use the latest compatible version
    }
  }
}

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
  value = jsonencode([for droplet in digitalocean_droplet.nodes : droplet.ipv4_address])
}