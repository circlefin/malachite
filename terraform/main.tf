terraform {
  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = "~> 2.0"
    }
  }
}

variable "do_token" {}
variable "node_count" {
  default = 3
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
  value = [for droplet in digitalocean_droplet.nodes : droplet.ipv4_address]
}
