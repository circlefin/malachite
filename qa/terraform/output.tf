output "ssh-cc" {
  value = "root@${digitalocean_droplet.cc.ipv4_address}"
}

output "next_steps" {
  value = <<EOT
source commands.sh
ok_cc
deploy_cc
cheat_sheet
EOT
}

output grafana_url {
  value = "http://${digitalocean_droplet.cc.ipv4_address}:3000"
}

output elastic_url {
  value = "http://${digitalocean_droplet.cc.ipv4_address}:5601"
}

output elastic_user_password {
  value = random_string.elastic_password.result
}
