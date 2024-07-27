resource "random_string" "elastic_password" {
  length           = 30
  special          = false
}

resource "digitalocean_droplet" "cc" {
  name      = "cc"
  image     = "debian-12-x64"
  region    = "tor1"
  tags      = concat(var.instance_tags, ["cc", "tor1"])
  size      = var.cc_size
  ssh_keys  = var.ssh_keys
  user_data = templatefile("user-data/cc-data.txt", {
    prometheus_config = filebase64("../viewer/config-prometheus/prometheus.yml")
    grafana_data_sources = filebase64("../viewer/config-grafana/provisioning/datasources/prometheus.yml")
    grafana_dashboards_config = filebase64("../viewer/config-grafana/provisioning/dashboards/malachite.yml")
    grafana_malachite_dashboard = filebase64("../viewer/config-grafana/provisioning/dashboards-data/main.json")
    elastic_password = random_string.elastic_password.result
  })
}
