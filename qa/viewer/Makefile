FILE?=prometheus.tgz

extract:
	rm -rf data-prometheus
	mkdir data-prometheus
	tar -xvzf $(FILE) -C data-prometheus

start: config-grafana/provisioning/alerting config-grafana/provisioning/notifiers config-grafana/provisioning/plugins data-prometheus data-grafana
	docker compose up -d

stop:
	docker compose down

# Suppress annoying Grafana log entries about missing folders
config-grafana/provisioning/alerting:
	mkdir config-grafana/provisioning/alerting
config-grafana/provisioning/notifiers:
	mkdir config-grafana/provisioning/notifiers
config-grafana/provisioning/plugins:
	mkdir config-grafana/provisioning/plugins
data-prometheus:
	mkdir data-prometheus
data-grafana:
	mkdir data-grafana

