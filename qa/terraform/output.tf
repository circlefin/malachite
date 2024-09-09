output "ssh-cc" {
  value = "root@${digitalocean_droplet.cc.ipv4_address}"
}

output "next_steps" {
  value = <<EOT
source commands.sh
deploy_cc
ssh-cc
cheat_sheet
EOT
}

output grafana_url {
  value = "http://${digitalocean_droplet.cc.ipv4_address}:3000"
}

output elastic_url {
  value = "http://${digitalocean_droplet.cc.ipv4_address}:5601"
}
