# QA

This is an opinionated QA environment with a human developer in mind. It focuses on logical blocks of a QA setup
using custom commands to simplify the language used to describe the process of running the nodes.

# The command & control server

A `cc` server is deployed along with the QA nodes. It helps manage the servers and it's closer than a developer machine.

The developer can build the Docker image for testing locally and push it to the Docker Registry on the `cc` server,
using the `deploy_cc` custom command. The QA nodes can then pull the image from the registry and run it.

The developer can create the testnet configuration remotely on the `cc` server using the `setup_config` custom command.
The configuration is stored in the `/data` folder on the server which is shared as over NFS with the QA nodes.

The `cc` server also hosts a Prometheus server with Grafana for monitoring the nodes.

Finally, the `cc` server also works as the DNS server for the QA nodes. All node IPs can be resolved by simple names on
the servers. This is especially useful when configuring persistent peers.

# Set up the hosts in Digital Ocean

After creating your DO access (see the CometBFT QA infra
[steps](https://github.com/cometbft/qa-infra/blob/main/README.md#setup)), run

```bash
cd terraform
terraform init
terraform apply -var small_nodes=0
terraform apply -var small_nodes=4
```

By running terraform with zero nodes first, you create the `cc` server ahead of time. You can skip that step and create
the `cc` server with the QA nodes in one go.

This will create a 4-node Digital Ocean QA environment a `hosts` file and a `commands.sh` file with the custom commands.

The servers are called `small0`, `small1`, `small2`, and `small3`, respectively but they are also available under the
names `node0`, `node1`, `node2`, and `node3`. If large servers are also created, they will be listed after the small
servers when referencing them as node<number>.

Most of the node setup is done automatically in cloud-init. When terraform finishes, the servers are still installing
packages and setting up their environment. One of the first commands we will run will check if the servers have
finished building.

# Post-terraform tasks

There are a few custom commands to make managing the nodes easier. They are self-explanatory in the `commands.sh` file.

Note: most of these commands require SSH authentication and if you use a Yubikey for SSH authentication, you ca
saturate your machine's SSH connection with the default settings. You a key file and the `ssh-agent` or change
connection settings.

## -1. TL;DR

You start execution on your local machine and move over to the `cc` server when it is ready:

```bash
source commands.sh

ok_cc
update_cc
deploy_cc # Takes minutes.

ssh-cc
xssh cat /etc/done
xssh mount /data # only needed if some servers came up earlier than the CC server

setup_config # depends on deploy_cc

dnode-run all
```

For local execution, you can use the following commands:

```bash
source commands.sh

keyscan_all_servers
xssh cat /etc/done
xssh mount /data # only needed if some servers came up earlier than the CC server

update_cc
deploy_cc  # Takes minutes.

setup_config

dnode-run all
```

## 0. Import custom commands

```bash
source commands.sh
```

## 1. Install all server SSH keys

```bash
❯ keyscan_all_servers
```

## 2. Check if server setup finished (print dates of when it finished)

```bash
❯ xssh cat /etc/done
```

Troubleshooting tip: if any of the servers finished earlier than the CC server, then it might have not mounted the NFS
volume from CC. The volume will be mounted on restart, but if you want to speed things up, you can mount the volume
manually:

```bash
❯ xssh mount /data
```

Ignore the error on the cc server, it is expected.

## 3. Install DNS entries on cc server.

```bash
❯ update_cc
```

## 4. Build your node and deploy it to the cc server.

```bash
❯ deploy_cc
```

This will take a few minutes. (4.5 minutes in Lausanne, connecting to a 4vCPU/8GB fra1 server in Digital Ocean.)

## 5. Create the configuration data on the cc server

```bash
setup_config
```

The configuration data is stored on the CC server under `/data`. This path is also shared with the QA nodes over NFS.

## 8. Start the nodes

```bash
dnode-run 0 2 3
RUST_LOG=debug cnode-run 1
```

```bash
dnode-stop all
```

You can use `dnode-log`, `dnode-stop` to manage the docker container. `dnode` is a generic command to run docker
commands remotely.

# hosts file

Terraform creates a [hosts](terraform/hosts) file that can be added to any server (including your local dev machine)
for easier access to the servers. The file is
deployed onto the cc server and it is used as part of the DNS service there.

# commands.sh file

A [commands.sh](terraform/comands.sh) file is created with suggested commands for CLI-based configuration and node
management. You can run `source commands.sh` and use the functions in your shell. The descriptions of commands are
listed in the top comment of the file. The file is copied over to `cc` during `update_cc` and invoked automatically
when you SSH into the server.
