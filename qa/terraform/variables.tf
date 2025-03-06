variable "do_token" {}

variable "ssh_keys" {
  type = list(string)
  default = []
}

# The project name in Digital Ocean.
variable project_name {
  type = string
  default = "malachite-testnet"
}

# CC server region
variable cc_region {
  type = string
  default = "fra1"
}

# Regions and number of servers to deploy there
# Regions list: https://docs.digitalocean.com/platform/regional-availability/
# ams3 - Amsterdam
# blr1 - Bangalore
# fra1 - Frankfurt
# lon1 - London
# nyc1 - New York City
# nyc3 - New York City
# sfo2 - San Francisco
# sfo3 - San Francisco
# sgp1 - Singapore
# syd1 - Sydney
# tor1 - Toronto
variable "ams3" {
  type    = number
  default = 0
}
variable "blr1" {
  type    = number
  default = 0
}
variable "fra1" {
  type    = number
  default = 0
}
variable "lon1" {
  type    = number
  default = 0
}
variable "nyc1" {
  type    = number
  default = 0
}
variable "nyc3" {
  type    = number
  default = 0
}
variable "sfo2" {
  type    = number
  default = 0
}
variable "sfo3" {
  type    = number
  default = 0
}
variable "sgp1" {
  type    = number
  default = 0
}
variable "syd1" {
  type    = number
  default = 0
}
variable "tor1" {
  type    = number
  default = 0
}

# Cheapest droplet size
variable shared {
  type = string
  default = "s-4vcpu-8gb"
}

# Small droplet size
variable small {
  type = string
  default = "g-2vcpu-8gb"
}

# Large droplet size
variable large {
  type = string
  default = "g-4vcpu-16gb"
}

# Type of servers to deploy into each region
variable cc_size {
  type = string
  default = "so-4vcpu-32gb-intel"
}
variable "ams3_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "blr1_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "fra1_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "lon1_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "nyc1_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "nyc3_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "sfo2_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "sfo3_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "sgp1_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "syd1_size" {
  type    = string
  default = "g-2vcpu-8gb"
}
variable "tor1_size" {
  type    = string
  default = "g-2vcpu-8gb"
}

variable "instance_tags" {
  type    = list(string)
  default = ["Malachite"]
}

variable "ssh_timeout" {
  type = string
  default = "60s"
}
