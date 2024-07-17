variable "small_a" {
  type    = number
  default = 0
}

variable "large_a" {
  type    = number
  default = 0
}

variable "small_b" {
  type    = number
  default = 0
}

variable "large_b" {
  type    = number
  default = 0
}

variable "small_c" {
  type    = number
  default = 0
}

variable "large_c" {
  type    = number
  default = 0
}

# Regions list: https://docs.digitalocean.com/platform/regional-availability/

variable "region_a" {
  type    = string
  default = "tor1"
}

variable "region_b" {
  type    = string
  default = "nyc1"
}

variable "region_c" {
  type    = string
  default = "nyc3"
}

output "next_steps" {
  value = <<EOT
source commands.sh
ok_cc
cheat_sheet
EOT
}

/*
"regions": [
  "ams3",
  "blr1",
  "fra1",
  "lon1",
  "nyc1",
  "nyc3",
  "sfo2",
  "sfo3",
  "sgp1",
  "syd1",
  "tor1"
],
*/