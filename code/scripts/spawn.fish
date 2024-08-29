#!/usr/bin/env fish

# This script takes:
# - a number of nodes to run as an argument,
# - the home directory for the nodes configuration folders

function help
    echo "Usage: spawn.fish [--help] --nodes NODES_COUNT --home NODES_HOME [--profile=PROFILE|--debug]"
end

argparse -n spawn.fish help 'nodes=' 'home=' 'profile=?' debug -- $argv
or return

if set -ql _flag_help
    help
    return 0
end

if ! set -q _flag_nodes
    help
    return 1
end

if ! set -q _flag_home
    help
    return 1
end

set profile_template (string replace -r '^$' 'time' -- $_flag_profile)

if set -q _flag_profile
    echo "Profiling enabled."
    set profile true
    set debug false
    set build_profile profiling
    set build_folder profiling
else if set -q _flag_debug
    echo "Debugging enabled."
    set profile false
    set debug true
    set build_profile dev
    set build_folder debug
else
    set profile false
    set debug false
    set build_profile release
    set build_folder release
end

set -x MALACHITE__CONSENSUS__MAX_BLOCK_SIZE "1MiB"
set -x MALACHITE__CONSENSUS__TIMEOUT_PROPOSE "5s"
set -x MALACHITE__CONSENSUS__TIMEOUT_PREVOTE "3s"
set -x MALACHITE__CONSENSUS__TIMEOUT_PRECOMMIT "3s"
set -x MALACHITE__CONSENSUS__TIMEOUT_COMMIT "0s"
set -x MALACHITE__MEMPOOL__MAX_TX_COUNT "1000"
set -x MALACHITE__MEMPOOL__GOSSIP_BATCH_SIZE 0
set -x MALACHITE__TEST__TX_SIZE "1KB"
set -x MALACHITE__TEST__TXS_PER_PART 64
set -x MALACHITE__TEST__TIME_ALLOWANCE_FACTOR 0.5
set -x MALACHITE__TEST__EXEC_TIME_PER_TX "100us"
set -x MALACHITE__CONSENSUS__P2P__PROTOCOL "broadcast"

echo "Compiling Malachite..."
cargo build --profile $build_profile

set session malachite
tmux kill-session -t $session
tmux new-session  -s $session -n main -d

set NODES_COUNT $_flag_nodes
set NODES_HOME  $_flag_home

for NODE in (seq 0 $(math $NODES_COUNT - 1))
    set NODE_HOME "$NODES_HOME/$NODE"
    mkdir -p "$NODE_HOME/logs"
    mkdir -p "$NODE_HOME/traces"

    rm -f "$NODE_HOME/logs/*.log"

    set pane $(tmux new-window -P -n "node-$NODE" /bin/zsh)

    echo "[Node $NODE] Spawning node..."

    if $debug
        set lldb_script "
            b malachite_cli::main
            run
            script with open('$NODE_HOME/node.pid', 'w') as f: f.write(str(lldb.debugger.GetSelectedTarget().process.id))
            continue
        "

        set cmd_prefix "rust-lldb --source =(echo \"$lldb_script\") ./target/$build_folder/malachite-cli -- "

        tmux send -t "$pane" "$cmd_prefix start --home '$NODE_HOME'" Enter
    else if $profile; and [ $NODE = 0 ]
        set cmd_prefix "cargo instruments --profile $build_profile --template $profile_template --time-limit 60000 --output '$NODE_HOME/traces/' --"

        tmux send -t "$pane" "sleep $NODE" Enter
        tmux send -t "$pane" "$cmd_prefix start --home '$NODE_HOME' 2>&1 > '$NODE_HOME/logs/node.log' &" Enter
        tmux send -t "$pane" "echo \$! > '$NODE_HOME/node.pid'" Enter
        tmux send -t "$pane" "tail -f '$NODE_HOME/logs/node.log'" Enter
    else
        set cmd_prefix "./target/$build_folder/malachite-cli"

        tmux send -t "$pane" "$cmd_prefix start --home '$NODE_HOME' 2>&1 > '$NODE_HOME/logs/node.log' &" Enter
        tmux send -t "$pane" "echo \$! > '$NODE_HOME/node.pid'" Enter
        tmux send -t "$pane" "tail -f '$NODE_HOME/logs/node.log'" Enter
    end
end

echo "Spawned $NODES_COUNT nodes."
echo

read -l -P "Launch tmux? [Y/n] " launch_tmux
switch $launch_tmux
    case N n
        echo "To attach to the tmux session, run:"
        echo "  tmux attach -t $session"
    case '*'
        tmux attach -t $session
end

echo

read -l -P "Press Enter to stop the nodes... " done

echo "Stopping all nodes..."
for NODE in (seq 0 $(math $NODES_COUNT - 1))
    set NODE_PID (cat "$NODES_HOME/$NODE/node.pid")
    echo "[Node $NODE] Stopping node (PID: $NODE_PID)..."
    kill $NODE_PID
end
echo
read -l -P "Press Enter to kill the tmux session... " done

tmux kill-session -t $session
