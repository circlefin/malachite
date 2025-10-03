#!/bin/bash

echo "Socket Analysis - $(date)"
echo ""

# Summary table
echo "Socket counts per node (ports 27000-27003 only):"
echo "Node     | Total | ESTABLISHED | LISTEN | TIME_WAIT | CLOSE_WAIT | Other"
echo "---------|-------|-------------|--------|-----------|------------|-------"

for node in node0 node1 node2 node3; do
    if docker compose ps $node 2>/dev/null | grep -q "Up"; then
        raw_output=$(docker compose exec -T $node cat /proc/net/tcp 2>/dev/null | tail -n +2 || echo "")
        
        if [ ! -z "$raw_output" ] && [ "$(echo "$raw_output" | wc -l)" -gt 1 ]; then
            filtered_output=$(echo "$raw_output" | grep -E ":6978|:6979|:697A|:697B" || echo "")
            
            if [ ! -z "$filtered_output" ]; then
                total=$(echo "$filtered_output" | wc -l | xargs)
                established=$(echo "$filtered_output" | awk '$4=="01"' | wc -l | xargs)
                listen=$(echo "$filtered_output" | awk '$4=="0A"' | wc -l | xargs)
                time_wait=$(echo "$filtered_output" | awk '$4=="06"' | wc -l | xargs)
                close_wait=$(echo "$filtered_output" | awk '$4=="08"' | wc -l | xargs)
                other=$(( total - established - listen - time_wait - close_wait ))
            else
                total="0"; established="0"; listen="0"; time_wait="0"; close_wait="0"; other="0"
            fi
            
            printf "%-8s | %-5s | %-11s | %-6s | %-9s | %-10s | %-5s\n" "$node" "$total" "$established" "$listen" "$time_wait" "$close_wait" "$other"
        else
            printf "%-8s | %-5s | %-11s | %-6s | %-9s | %-10s | %-5s\n" "$node" "ERROR" "-" "-" "-" "-" "-"
        fi
    else
        printf "%-8s | %-5s | %-11s | %-6s | %-9s | %-10s | %-5s\n" "$node" "DOWN" "-" "-" "-" "-" "-"
    fi
done
echo ""

# Detailed connections per node
for node in node0 node1 node2 node3; do
    if docker compose ps $node 2>/dev/null | grep -q "Up"; then
        echo "$node connections:"
        
        # Collect all connections in a temp file for sorting
        temp_file=$(mktemp)
        docker compose exec -T $node cat /proc/net/tcp 2>/dev/null | tail -n +2 | grep -E ":6978|:6979|:697A|:697B" | while read -r line; do
            local_addr=$(echo $line | awk '{print $2}')
            remote_addr=$(echo $line | awk '{print $3}')
            state=$(echo $line | awk '{print $4}')
            
            local_ip_hex=${local_addr%:*}
            local_port_hex=${local_addr##*:}
            remote_ip_hex=${remote_addr%:*}
            remote_port_hex=${remote_addr##*:}
            
            if [ "$remote_ip_hex" != "00000000" ]; then
                # Convert little-endian hex to IP
                local_a=$((0x${local_ip_hex:6:2}))
                local_b=$((0x${local_ip_hex:4:2}))
                local_c=$((0x${local_ip_hex:2:2}))
                local_d=$((0x${local_ip_hex:0:2}))
                
                remote_a=$((0x${remote_ip_hex:6:2}))
                remote_b=$((0x${remote_ip_hex:4:2}))
                remote_c=$((0x${remote_ip_hex:2:2}))
                remote_d=$((0x${remote_ip_hex:0:2}))
                
                local_port=$((0x$local_port_hex))
                remote_port=$((0x$remote_port_hex))
                
                case $state in
                    "01") state_name="ESTABLISHED";;
                    "02") state_name="SYN_SENT";;
                    "03") state_name="SYN_RECV";;
                    "04") state_name="FIN_WAIT1";;
                    "05") state_name="FIN_WAIT2";;
                    "06") state_name="TIME_WAIT";;
                    "07") state_name="CLOSE";;
                    "08") state_name="CLOSE_WAIT";;
                    "09") state_name="LAST_ACK";;
                    "0A") state_name="LISTEN";;
                    "0B") state_name="CLOSING";;
                    *) state_name="$state";;
                esac
                
                # Identify peer and create sort key
                case "$remote_a.$remote_b.$remote_c.$remote_d" in
                    # Standard testnet IPs (172.20.0.x)
                    "172.20.0.10") peer="→node0"; sort_key="1";;
                    "172.20.0.11") peer="→node1"; sort_key="2";;
                    "172.20.0.12") peer="→node2"; sort_key="3";;
                    "172.20.0.13") peer="→node3"; sort_key="4";;
                    # Multi-network testnet IPs (172.21.0.x validators internal)
                    "172.21.0.10") peer="→node0"; sort_key="1";;
                    "172.21.0.11") peer="→node1"; sort_key="2";;
                    "172.21.0.12") peer="→node2"; sort_key="3";;
                    # Multi-network testnet IPs (172.22.0.x node3 external)
                    "172.22.0.13") peer="→node3"; sort_key="4";;
                    # Multi-network testnet IPs (172.23.0.x public network)
                    "172.23.0.10") peer="→node0"; sort_key="1";;
                    "172.23.0.11") peer="→node1"; sort_key="2";;
                    "172.23.0.12") peer="→node2"; sort_key="3";;
                    "172.23.0.13") peer="→node3"; sort_key="4";;
                    # NAT testnet IPs (192.168.100.x internal, 10.0.1.x external, 172.17.0.1 Docker host)
                    "192.168.100.10") peer="→node0"; sort_key="1";;
                    "192.168.100.11") peer="→node1"; sort_key="2";;
                    "192.168.100.12") peer="→node2"; sort_key="3";;
                    "10.0.1.13") peer="→node3"; sort_key="4";;
                    "172.17.0.1") peer="→host"; sort_key="5";;
                    *) peer=""; sort_key="9";;
                esac
                
                printf "%s|  %s:%d → %s:%d [%s] %s\n" "$sort_key" "$local_a.$local_b.$local_c.$local_d" "$local_port" "$remote_a.$remote_b.$remote_c.$remote_d" "$remote_port" "$state_name" "$peer"
            else
                printf "0|  LISTEN on port %d\n" "$((0x$local_port_hex))"
            fi
        done > "$temp_file"
        
        # Sort by the first field (sort_key) and display without the sort key
        sort -t'|' -k1,1n "$temp_file" | cut -d'|' -f2-
        rm -f "$temp_file"
        echo ""
    fi
done

# Monitor mode
if [ "$1" = "monitor" ]; then
    while true; do
        clear
        $0  # Call itself without monitor argument
        echo "Next update in 5 seconds... (Ctrl+C to stop)"
        sleep 5
    done
fi
