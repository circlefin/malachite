# QA

This is an opinionated QA environment with a human developer in mind. It focuses on logical blocks of a QA setup
using custom commands to simplify the language used to describe the process of running the nodes.

## Prerequisites

* [pssh](https://linux.die.net/man/1/pssh) (Mac) or [parallel-ssh](https://manpages.org/parallel-ssh) (Linux) on your
  local machine.

If you use `parallel-ssh`, create a symlink to `pssh` in your path. (Something like
`ln /usr/bin/parallel-ssh /usr/bin/pssh` will do the trick.)

## The command & control server

A `cc` server is deployed along with the QA nodes. It helps manage the servers, and it is closer than a developer
machine.

The developer can build the Docker image for testing locally and push it to the Docker Registry on the `cc` server,
using the `deploy_cc` custom command. The QA nodes can then pull the image from the registry and run it.

The developer can create the testnet configuration remotely on the `cc` server using the `setup_config` custom command.
The configuration is stored in the `/data` folder on the server which is shared over NFS with the QA nodes.

The `cc` server also hosts a Prometheus server with Grafana for monitoring the nodes and an ElasticSearch database with
Kibana to collect the logs. The services can be reached at their default port 3000 and port 5601.

Finally, the `cc` server also works as the DNS server for the QA nodes. All node IPs can be resolved by simple names on
the servers. This is especially useful when configuring persistent peers.

## Set up the hosts in Digital Ocean

After creating your DO access (see the CometBFT QA infra
[steps](https://github.com/cometbft/qa-infra/blob/main/README.md#setup)), run

```bash
cd terraform
terraform init
terraform apply # optional. This will create the cc server only.
terraform apply -var nyc1=4 -var nyc3=3 # the cc server will not be deleted if you scale the nodes.
```

By running terraform with zero nodes first, you create the `cc` server ahead of time. You can skip that step and create
the `cc` server with the QA nodes in one go.

The above will create a 7-node Digital Ocean QA environment and a `commands.sh` file with the custom commands.

Most of the node setup is done automatically in cloud-init. When terraform finishes, the CC server is still installing
services for ElasticSearch and Prometheus.

(Optional) Use the `ok_cc` command to check if all services are up and running. By the time you deploy a binary, all
services should be running.

## Post-terraform tasks

There are a few custom commands to make managing the nodes easier. They are explained in the `commands.sh` file.

### 0. TL;DR

You start execution on your local machine and move over to the `cc` server when it is ready. You can also keep working
from your local machine if you feel the servers are close enough and the network is fast.

```bash
source commands.sh # do this in all new terminal window on your machine. No need to do this on the CC server.

deploy_cc # Takes 4-5 minutes.
          # You can run it on cc server as well, but you have to manually put the source code at /root/malachite.
ssh-cc # (optional) move to the CC server and run the rest of the commands closer to the QA nodes.
setup_config # only run it after deploy_cc finished.

d_pull all # run `docker pull` on all QA servers
d_run all # run malachite on all QA servers

# Wait some time to generate data

d_rm all # stop and remove the docker container "node" from the servers
```

### 1. Import custom commands

```bash
source commands.sh
```

Make the custom commands available on your local machine. You do not need to run this on the CC server, as it gets
invoked automatically when you SSH into the server.

### 2. Build your node and deploy it to the cc server.

```bash
deploy_cc
```

Builds the application using Docker and deploys it into the CC server Docker Registry.

This will take a few minutes. (4.5 minutes in Lausanne, connecting to a 4vCPU/8GB fra1 server in Digital Ocean.)

You can also run this command on the `cc` server, but you need to copy or clone the source code over to the server.

### 3. Connect to the CC server

```bash
ssh-cc
```

It is encouraged to run the rest of the commands from the CC server as it is closer to the QA servers and the commands
run faster.

The custom commands are automatically available on the CC server. No need to `source commands.sh` there.

You can keep running on your local machine, though, if that is more convenient.

### 4. Create the configuration data on the cc server

```bash
setup_config
```

The configuration data is stored on the CC server under `/data`. This path is also shared with the QA nodes over NFS.

Depends on an up-to-date host count. Re-run it, if you changed the number of servers.

### 5. Start the nodes

```bash
d_run 0 2 3
RUST_LOG=debug d_run 1
```

You can also use the `all` keyword to start or stop all nodes at once.

```bash
d_run all
```

You can use `d_run`, `d_log`, `d_stop` and `d_rm` to manage the docker containers.

### (optional) Make sure CC installed all services

```bash
ok_cc
```

This checks if the Prometheus, Grafana, Docker Registry, ElasticSearch and Kibana installation has finished on the CC
server.

It will print a date if the server finished installation.

# Created files

## commands.sh file

Terraform creates a [commands.sh](terraform/commands.sh) file with suggested commands for CLI-based configuration and
node management. You can run `source commands.sh` and use the functions in your shell. The descriptions of commands are
listed in the top comment of the file. The file is copied over to `cc` during `ok_cc` and invoked automatically
when you SSH into the server.

## prometheus.tgz file

This file gets exported using the `get_prometheus_data` command. Import it in the viewer for further analysis.

# Viewer

The viewer allows you to view the metrics of a testnet on your local machine. You can export the Prometheus metrics
from the cloud and keep them on your local machine even after the testnet is destroyed. You can do additional analysis
and create custom Grafana dashboards.

## Prerequisites

* docker on your machine
* a running `cc` server from where you download the data
* `make` on your machine.

The `Makefile` is in the `viewer` directory.

## 1. Download the data

This command is part of the terraform-created `commands.sh` file.

```bash
get_prometheus_data
```

This will download a compressed `prometheus.tgz` file from the `cc` server.

## 2. Extract the data to its destination

```bash
make extract_data
```

You can give a different path to the command if you stored the file elsewhere with the `FILE` environment variable.

## 3. Start the viewer

```bash
make viewer-start
```

You can view the Grafana dashboard at `http://localhost:3000`. The default username and password are `admin`/`admin`.

## 4. Finish up

When you are done with the data, you can stop the viewer.

```bash
make viewer-stop
```

