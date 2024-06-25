#!/usr/bin/env fish

# This script takes:
# - a number of nodes to run as an argument, 
# - the home directory for the nodes configuration folders

function help
    echo "Usage: spawn.fish [--help] --nodes NODES_COUNT --home NODES_HOME [--profile]"
end

argparse -n spawn.fish help 'nodes=' 'home=' profile -- $argv
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

if set -q _flag_profile
    echo "Profiling enabled."
    set profile true
    set build_profile profiling
else
    set profile false
    set build_profile release
end

set -x MALACHITE__CONSENSUS__MAX_BLOCK_SIZE "1 MiB"
set -x MALACHITE__TEST__TXS_PER_PART 50
set -x MALACHITE__TEST__TIME_ALLOWANCE_FACTOR 0.7
set -x MALACHITE__TEST__EXEC_TIME_PER_PART 10ms

echo "Compiling Malachite..."
cargo build --profile $build_profile

set session malachite
tmux kill-session -t $session
tmux new-session -s $session -n main -d

set NODES_COUNT $_flag_nodes
set NODES_HOME $_flag_home

for NODE in (seq 0 $(math $NODES_COUNT - 1))
    set NODE_HOME "$NODES_HOME/$NODE"
    mkdir -p "$NODE_HOME/logs"
    rm -f "$NODE_HOME/logs/*.log"

    if $profile
        set cmd_prefix "samply record --save-only -o '$NODE_HOME/perf.json' --"
    else
        set cmd_prefix ""
    end

    set pane $(tmux new-window -P -n "node-$NODE" /bin/zsh)

    echo "[Node $NODE] Spawning node..."

    tmux send -t "$pane" "$cmd_prefix ./target/$build_profile/malachite-cli start --home '$NODE_HOME' 2>&1 > '$NODE_HOME/logs/node.log' &" Enter
    tmux send -t "$pane" "echo \$! > '$NODE_HOME/node.pid'" Enter
    tmux send -t "$pane" "tail -f '$NODE_HOME/logs/node.log'" Enter
end

echo "Spawned $NODES_COUNT nodes."
echo

read -l -P "Launch tmux? [y/N] " launch_tmux
switch $launch_tmux
    case Y y
        tmux attach -t $session
    case '*'
        echo "To attach to the tmux session, run:"
        echo "  tmux attach -t $session"
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
