# QA

This is an opinionated QA environment with a human developer in mind. It focuses on logical blocks of a QA setup
using custom commands to simplify the language used to describe the process of running the nodes.

## Prerequisites

* [pssh](https://linux.die.net/man/1/pssh)(Mac) or [parallel-ssh](https://manpages.org/parallel-ssh)(Linux) on your
  local machine.
* If you use parallel-ssh, create a symlink to `pssh` in your path.

* Usually, `ln /usr/bin/parallel-ssh /usr/bin/pssh` will do the trick.

## The command & control server

A `cc` server is deployed along with the QA nodes. It helps manage the servers, and it is closer than a developer
machine.

The developer can build the Docker image for testing locally and push it to the Docker Registry on the `cc` server,
using the `deploy_cc` custom command. The QA nodes can then pull the image from the registry and run it.

The developer can create the testnet configuration remotely on the `cc` server using the `setup_config` custom command.
The configuration is stored in the `/data` folder on the server which is shared as over NFS with the QA nodes.

The `cc` server also hosts a Prometheus server with Grafana for monitoring the nodes. The data can be downloaded using
the `get_prometheus_data` custom command. Then it can be imported to a local Grafana/Prometheus viewer for further
analysis.

Finally, the `cc` server also works as the DNS server for the QA nodes. All node IPs can be resolved by simple names on
the servers. This is especially useful when configuring persistent peers.

## Set up the hosts in Digital Ocean

After creating your DO access (see the CometBFT QA infra
[steps](https://github.com/cometbft/qa-infra/blob/main/README.md#setup)), run

```bash
cd terraform
terraform init
terraform apply -var small_nodes=0 # optional. This will create the cc server only.
terraform apply -var small_nodes=4 -var large_nodes=3 # the cc server will not be deleted if you scale the nodes.
```

By running terraform with zero nodes first, you create the `cc` server ahead of time. You can skip that step and create
the `cc` server with the QA nodes in one go.

The above will create a 7-node Digital Ocean QA environment a `hosts` file and a `commands.sh` file with the custom
commands.

Most of the node setup is done automatically in cloud-init. When terraform finishes, the servers are still installing
packages and setting up their environment. One of the first commands we will run will check if the servers have
finished building.

## Post-terraform tasks

There are a few custom commands to make managing the nodes easier. They are explained in the `commands.sh` file.

Note: most of these commands require SSH authentication. If you use a Yubikey for SSH authentication, you can
saturate your machine's SSH connection with the default settings. Use a key file and `ssh-agent` or change
connection settings.

### 0. TL;DR

You start execution on your local machine and move over to the `cc` server when it is ready. You can also keep working
from your local machine if you feel the servers are close enough and the network is fast.

```bash
source commands.sh # do this in all new terminal window on your machine. No need to do this on the CC server.

ok_cc # make sure the CC server has finished initial setup.
deploy_cc # Takes 4-5 minutes. Continue in a different window while this is running.
          # You can run it on cc server as well, but you have to manually put the source code at /root/malachite.

ssh-cc # (optional) move to the CC server and run the rest of the commands closer to the QA nodes.
setup_config # depends on deploy_cc, only run it if that finished.

ok_all # make sure all QA servers have finished initial setup
dnode-run all # run malachite on all QA servers

# Wait some time to generate data

dnode-stop all # stop all malachite nodes. It does not remove the docker container so the logs can be viewed.

get_prometheus_data # this has to run on the machine where you want the data to end up. Usually, your local machine.
fetch_log all # fetch he logs of malachite-cli from each QA node

dnode-rm all # remove the docker container "node" from the servers so the application can be re-run
```

### 1. Import custom commands

```bash
source commands.sh
```

Make the custom commands available on your local machine. You do not need to run this on the CC server, as it gets
invoked automatically when you SSH into the server.

### 2. Make sure CC works

```bash
ok_cc
```

This loads the SSH key into your known_hosts and checks if the cloud-init execution has finished on the CC server. It
also sets up the DNS service with the created hosts and copies the `commands.sh` over for easy execution.

It will print a date if the server successfully finished the setup.

You have to run this every time you create or destroy new servers with Terraform. It copies the server IPs and the
correct custom commands to the CC server.

### 4. Build your node and deploy it to the cc server.

```bash
deploy_cc
```

Builds the application using Docker and deploys it into the CC server Docker Registry.

This will take a few minutes. (4.5 minutes in Lausanne, connecting to a 4vCPU/8GB fra1 server in Digital Ocean.)

You can continue executing the rest of the setup commands, until you want to configure the network with `setup_config`.
You will need the application for the correct generation of the application configuration.

You can also run this command on the `cc` server (see the `ssh-cc` command below). Caveat: you need to copy the source
code over to the server

### 4.5 (optional) Connect to the CC server

```bash
ssh-cc
```

It is encouraged to run the rest of the commands from the CC server as it is closer to the QA servers and the commands
run faster.

The custom commands are automatically available on the CC server. No need to `source commands.sh` there.

You can keep running on your local machine, though, if that is more convenient.

### 5. Make sure all servers finished cloud-init installations

```bash
ok_all
```

Similar to `ok_cc` but all deployed servers are taken into account. Your `known_hosts` file will be updated with the
server keys and prints the date each server finished installing cloud-init. Run this multiple times until all servers
return successfully.

### 6. Create the configuration data on the cc server

```bash
setup_config
```

The configuration data is stored on the CC server under `/data`. This path is also shared with the QA nodes over NFS.

Depends on an up-to-date host count. Re-run it after `ok_cc` if you changed the number of servers.

### 7. Start the nodes

```bash
dnode-run 0 2 3
RUST_LOG=debug cnode-run 1
```

You can also use the `all` keyword to start or stop all nodes at once.

```bash
dnode-stop all
```

You can use `dnode`, `dnode-run`, `dnode-log` and `dnode-stop` to manage the docker container.
`dnode` is a generic command to run docker commands remotely.

### 8. Get the data from Prometheus

```bash
get_prometheus_data
```

This will copy the compressed prometheus database from the `cc` server to your local machine as `prometheus.tgz`.

# Created files

## hosts file

Terraform creates a [hosts](terraform/hosts) file that can be added to any server (including your local dev machine)
for easier access to the servers. The file is
deployed onto the cc server and it is used as part of the DNS service there.

## commands.sh file

Terraform also creates a [commands.sh](terraform/commands.sh) file with suggested commands for CLI-based configuration
and node
management. You can run `source commands.sh` and use the functions in your shell. The descriptions of commands are
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

The commands that start with `make` will need to be run from the `viewer` directory or you can use `-C` to point make
to the directory.

## 1. Download the data

This command is part of the terraform-created `commands.sh` file.

```bash
download_data
```

This will download compressed `prometheus.tgz` file from the `cc` server.

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
