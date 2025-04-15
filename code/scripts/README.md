# Testnet scripts
This directory contains scripts that gives you three ways to run local testnet for development purposes.

In each case, we are going to assume that you run your commands from the `code` folder.

## Bash
The `spawn.bash` script is a generic script that can run using the Bash command language interpreter.

* How to use with mandatory parameters:
```bash
scripts/spawn.bash --nodes 5 --home $HOME/.malachite --app informalsystems-malachitebft-example-channel
```

The script will build the application (the `--app` parameter requires a Rust crate name) and run its `start` subcommand.

* Optional parameters:

`--no-reset`: this will prevent the script cleaning up the WAL and database of the nodes in their configuration  directory.

* The script will always build a release version of the application.

* The script will run the application in the foreground. When you press CTRL-C to stop the script, the application
instances will also be terminated.

* The application log can be found in the node's home directory in the `logs/node.log` file.

* The script overrides some of the configuration parameters for local testnet use:
```bash
export MALACHITE__CONSENSUS__P2P__PROTOCOL__TYPE="gossipsub"
export MALACHITE__CONSENSUS__TIMEOUT_PROPOSE="2s"
export MALACHITE__CONSENSUS__TIMEOUT_PROPOSE_DELTA="1s"
export MALACHITE__CONSENSUS__TIMEOUT_PREVOTE="1s"
export MALACHITE__CONSENSUS__TIMEOUT_PRECOMMIT="1s"
export MALACHITE__CONSENSUS__TIMEOUT_COMMIT="0s"
export MALACHITE__CONSENSUS__TIMEOUT_STEP="2s"
export MALACHITE__MEMPOOL__MAX_TX_COUNT="10000"
export MALACHITE__MEMPOOL__GOSSIP_BATCH_SIZE=0
export MALACHITE__TEST__MAX_BLOCK_SIZE="50KiB"
export MALACHITE__TEST__VALUE_PAYLOAD="proposal-and-parts"
export MALACHITE__TEST__TX_SIZE="1KiB"
export MALACHITE__TEST__TXS_PER_PART=256
export MALACHITE__TEST__TIME_ALLOWANCE_FACTOR=0.3
export MALACHITE__TEST__EXEC_TIME_PER_TX="0ms"
export MALACHITE__TEST__MAX_RETAIN_BLOCKS=10000
export MALACHITE__TEST__VOTE_EXTENSIONS__ENABLED="false"
export MALACHITE__TEST__VOTE_EXTENSIONS__SIZE="1KiB"
```

## Fish
The `spawn.fish` script is a generic script that can run using the [Fish](https://fishshell.com/) command language
interpreter.

* This script has additional functionality compared to the Bash script.
* How to use with mandatory parameters:
```fish
scripts/spawn.fish --nodes 5 --home $HOME/.malachite
```
The script will build the `informalsystems-malachitebft-example-channel` application by default, unless the name is
overwritten by the `--app` parameter. (the `--app` parameter requires a Rust crate name) Then it run the application's
`start` subcommand.

* Optional parameters:

`--no-reset`: this will prevent the script cleaning up the WAL and database of the nodes in their configuration directory.

`--app`: set the name of the application that will be built. The default is `informalsystems-malachitebft-example-channel`.

`--profile`: turn on profiling. The build profile will be set as `profiling` and the compiled binary will be read from
the `target/profiling` directory.

`--debug`: mutually exclusive to `--profile`. Sets the build profile to `dev` and the compiled binary directory path to
`target/debug`.

`--lldb`: enable the `rust-lldb` debugger before running the application and set a breakpoint on the `main` function.

* The script uses the `tmux` session manager, when creating the node processes.

* The script will build a release version of the application unless the `--profile` or `--debug` parameters are given.

* The script will run the application in the foreground. When you press CTRL-C to stop the script, the application
  instances will also be terminated.

* The application log can be found in the node's home directory in the `logs/node.log` file.

* The script overrides some of the configuration parameters for local testnet use:
```bash
set -x MALACHITE__CONSENSUS__P2P__PROTOCOL__TYPE "gossipsub"
set -x MALACHITE__CONSENSUS__TIMEOUT_PROPOSE "5s"
set -x MALACHITE__CONSENSUS__TIMEOUT_PREVOTE "3s"
set -x MALACHITE__CONSENSUS__TIMEOUT_PRECOMMIT "3s"
set -x MALACHITE__CONSENSUS__TIMEOUT_COMMIT "0s"
set -x MALACHITE__CONSENSUS__TIMEOUT_STEP "2s"

set -x MALACHITE__MEMPOOL__MAX_TX_COUNT 1000
set -x MALACHITE__MEMPOOL__GOSSIP_BATCH_SIZE 0
set -x MALACHITE__TEST__MAX_BLOCK_SIZE "2MiB"
set -x MALACHITE__TEST__TX_SIZE "1 KiB"
set -x MALACHITE__TEST__TXS_PER_PART 1024
set -x MALACHITE__TEST__TIME_ALLOWANCE_FACTOR 0.5
set -x MALACHITE__TEST__EXEC_TIME_PER_TX "1ms"
set -x MALACHITE__TEST__MAX_RETAIN_BLOCKS 50
set -x MALACHITE__TEST__VOTE_EXTENSIONS__ENABLED false
set -x MALACHITE__TEST__VOTE_EXTENSIONS__SIZE "1KiB"
set -x MALACHITE__SYNC__ENABLED true
set -x MALACHITE__SYNC__REQUEST_TIMEOUT "30s"
```

## make
The `Makefile` configuration is a generic set of targets that can run using the GNU `make` command.

* How to use with mandatory parameters:
```bash
make -C scripts
```
The `-C` parameter switches the execution folder for `make` to the directory specified after it.

The command will build the application and run its `start` subcommand. Contrary to the shell solutions the applications
are started using `nohup` and are running in the background as shell jobs.

* Optional parameters:

`NODES_COUNT=5 make -C scripts`: set the number of nodes to spawn. The default is 5.

`NODES_HOME=$HOME/.malachite make -C scripts`: set the home directory for the nodes. The default is `$HOME/.malachite`.

`APP_BINARY=informalsystems-malachitebft-example-channel make -C scripts`: set the name of the application that will be
built and executed. The default is `informalsystems-malachitebft-example-channel`.

`RELEASE=1 make -C scripts`: set the build profile to `release`. The default is application build is `debug`.

* Additional parameters that are less used:

`MALACHITE_CODE_DIR`: set the directory where the Malachite code is located. The default is `../../code`.

`CC`: the cargo binary with path. Default is `$(which cargo)`.

* The script will build a debug version of the application unless the `RELEASE=1` parameters is given.

* The script will run the application in the background. If you run the `make -C scripts stop` command, the application
will stop.

* The application log can be found in the node's home directory in the `logs/nohup.out` file. The `make -C scripts logs`
command will list all the log files.

* The application configuration, logs, databases and WAL can be removed with `make -C scripts clean`.

* Other `Makefile` targets with obvious purpose: `build`, `testnet`, `start`, `restart`. The default target (`all`) uses
the `build`, `testnet` and `start` targets as dependency.
*
