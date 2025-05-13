#!/usr/bin/env bash

export MALACHITE__CONSENSUS__P2P__PROTOCOL__TYPE="gossipsub"
export MALACHITE__CONSENSUS__TIMEOUT_PROPOSE="5s"
export MALACHITE__CONSENSUS__TIMEOUT_PREVOTE="3s"
export MALACHITE__CONSENSUS__TIMEOUT_PRECOMMIT="3s"
export MALACHITE__CONSENSUS__TIMEOUT_COMMIT="0s"
export MALACHITE__CONSENSUS__TIMEOUT_STEP="2s"
export MALACHITE__CONSENSUS__VOTE_SYNC__MODE="request-response"

export MALACHITE__SYNC__ENABLED=true
export MALACHITE__SYNC__REQUEST_TIMEOUT="30s"

export MALACHITE__MEMPOOL__MAX_TX_COUNT=10000
export MALACHITE__MEMPOOL__GOSSIP_BATCH_SIZE=0
export MALACHITE__MEMPOOL__LOAD__LOAD_TYPE="uniform_load"
export MALACHITE__MEMPOOL__LOAD__INTERVAL="1ms"
# export MALACHITE__MEMPOOL__LOAD__COUNT=1000 # For some reason this fails to parse?
export MALACHITE__MEMPOOL__LOAD__SIZE="1 KiB"

export MALACHITE__TEST__MAX_BLOCK_SIZE="5 MiB"
export MALACHITE__TEST__TXS_PER_PART=1024
export MALACHITE__TEST__TIME_ALLOWANCE_FACTOR=0.5
export MALACHITE__TEST__EXEC_TIME_PER_TX="0ms"
export MALACHITE__TEST__MAX_RETAIN_BLOCKS=100
export MALACHITE__TEST__VOTE_EXTENSIONS__ENABLED=false
export MALACHITE__TEST__VOTE_EXTENSIONS__SIZE="1KiB"

export MALACHITE__VALUE_SYNC__ENABLED="true"
export MALACHITE__VALUE_SYNC__STATUS_UPDATE_INTERVAL="10s"
export MALACHITE__VALUE_SYNC__REQUEST_TIMEOUT="10s"

# Check if tmux is available
if ! command -v tmux &> /dev/null; then
    echo "Error: tmux is not installed or not in PATH"
    echo "Please install tmux first. For example:"
    echo "  Ubuntu/Debian: sudo apt install tmux"
    echo "  MacOS: brew install tmux"
    echo "  CentOS/RHEL: sudo yum install tmux"
    exit 1
fi

help() {
    echo "Usage: spawn.sh [--help] --nodes NODES_COUNT --home NODES_HOME [--app APP_BINARY] [--no-reset] [--profile=PROFILE|--debug] [--lldb]"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --help)
            help
            exit 0
            ;;
        --nodes)
            NODES_COUNT="$2"
            shift 2
            ;;
        --home)
            NODES_HOME="$2"
            shift 2
            ;;
        --app)
            app_name="$2"
            shift 2
            ;;
        --no-reset)
            no_reset=true
            shift
            ;;
        --profile=*)
            profile=true
            profile_template="${1#*=}"
            [ -z "$profile_template" ] && profile_template="time"
            build_profile="profiling"
            build_folder="profiling"
            shift
            ;;
        --debug)
            debug=true
            build_profile="dev"
            build_folder="debug"
            shift
            ;;
        --lldb)
            lldb=true
            debug=true
            build_profile="dev"
            build_folder="debug"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            help
            exit 1
            ;;
    esac
done

# Check required arguments
if [ -z "$NODES_COUNT" ] || [ -z "$NODES_HOME" ]; then
    help
    exit 1
fi

# Set defaults
app_name=${app_name:-"informalsystems-malachitebft-starknet-app"}
profile=${profile:-false}
debug=${debug:-false}
lldb=${lldb:-false}
build_profile=${build_profile:-"release"}
build_folder=${build_folder:-"release"}
no_reset=${no_reset:-false}

echo "Compiling '$app_name'..."
cargo build --bin "$app_name" --profile "$build_profile"

if [ $? -ne 0 ]; then
    echo "Error: Compilation failed"
    exit 1
fi

session="malachite"
tmux kill-session -t "$session" 2>/dev/null
tmux new-session -s "$session" -n main -d

for ((NODE=0; NODE<NODES_COUNT; NODE++)); do
    NODE_HOME="$NODES_HOME/$NODE"

    rm -rf "$NODE_HOME/logs"
    mkdir -p "$NODE_HOME/logs"

    rm -rf "$NODE_HOME/traces"
    mkdir -p "$NODE_HOME/traces"

    if [ "$no_reset" != true ]; then
        echo "[Node $NODE] Resetting state"
        rm -rf "$NODE_HOME/db"
        mkdir -p "$NODE_HOME/db"
        rm -rf "$NODE_HOME/wal"
        mkdir -p "$NODE_HOME/wal"
    fi

    pane=$(tmux new-window -P -n "node-$NODE" "$SHELL")

    echo "[Node $NODE] Spawning node..."

    if [ "$lldb" = true ]; then
        lldb_script="
            b $app_name::main
            run
            script with open('$NODE_HOME/node.pid', 'w') as f: f.write(str(lldb.debugger.GetSelectedTarget().process.id))
            continue
        "
        cmd_prefix="rust-lldb --source =(echo \"$lldb_script\") ./target/$build_folder/$app_name -- "
        tmux send-keys -t "$pane" "$cmd_prefix start --home '$NODE_HOME'" Enter
    elif [ "$profile" = true ] && [ "$NODE" -eq 0 ]; then
        cmd_prefix="cargo instruments --bin $app_name --profile $build_profile --template $profile_template --time-limit 60000 --output '$NODE_HOME/traces/' --"
        tmux send-keys -t "$pane" "sleep $NODE" Enter
        tmux send-keys -t "$pane" "unbuffer $cmd_prefix start --home '$NODE_HOME' 2>&1 | tee '$NODE_HOME/logs/node.log'" Enter
    else
        cmd_prefix="./target/$build_folder/$app_name"
        tmux send-keys -t "$pane" "unbuffer $cmd_prefix start --home '$NODE_HOME' 2>&1 | tee '$NODE_HOME/logs/node.log'" Enter
    fi
done

echo "Spawned $NODES_COUNT nodes."
echo

tmux attach -t "$session"

echo

read -p "Press Enter to kill the tmux session... " quit
tmux kill-session -t "$session"
