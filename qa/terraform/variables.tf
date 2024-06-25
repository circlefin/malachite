variable "small_nodes" {
  type    = number
  default = 2
}

variable "large_nodes" {
  type    = number
  default = 0
}

variable "region" {
  type    = string
  default = "fra1"
}

output "next_steps" {
  value = <<EOT
source commands.sh
ok_cc
cheat_sheet
EOT
}
