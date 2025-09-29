# Testnet Scenarios

Testnet scenarios built for validating P2P bootstrap discovery in differnt networking environments.

## Overview
 There are 3 distinct testnet scenarios that test the bootstrap discovery:

1. **Single Network + Host IP** (`make testnet`) - Docker port mapping scenario
2. **Multi-Network Address Mismatch** (`make testnet-multi`) - Enterprise network segments
3. **NAT with Gateway** (`make testnet-nat`) - NAT traversal

Each scenario validates that the bootstrap identification logic correctly uses `dial_data` addresses (that actually work) rather than `identify` protocol addresses (that peers advertise but may not be reachable).

---

## `make testnet` - Single Network + Host IP

**Purpose**: Tests Docker port mapping scenario where external node connects via host IP. 

### Network Topology:
```
DOCKER HOST: 192.168.1.147
Port 27000 ----+    Port 27001 ----+    Port 27002 ----+
               |                   |                   |
               v                   v                   v

+-------------------------------------------------------------+
|            Docker Network (172.20.0.0/16)                   |
|                                                             |
|  +------------+    +------------+    +------------+         |
|  |   node0    |<-->|   node1    |<-->|   node2    |         |
|  |172.20.0.10 |    |172.20.0.11 |    |172.20.0.12 |         |
|  |  :27000    |    |  :27000    |    |  :27000    |         |
|  +------------+    +------------+    +------------+         |
|                                                             |
|  +-------------------------------------------------------+  |
|  |                      node3                            |  |
|  |                  172.20.0.13:27000                    |  |
|  |                                                       |  |
|  |  Configured to dial HOST IP (not container IPs):      |  |
|  |  • 192.168.1.147:27000 → node0                        |  |
|  |  • 192.168.1.147:27001 → node1                        |  |
|  |  • 192.168.1.147:27002 → node2                        |  |
|  +-------------------------------------------------------+  |
+-------------------------------------------------------------+

PORT MAPPING FLOW:
192.168.1.147:27000 → 172.20.0.10:27000 (node0)
192.168.1.147:27001 → 172.20.0.11:27000 (node1) 
192.168.1.147:27002 → 172.20.0.12:27000 (node2)

ADDRESS MISMATCH TEST:
• Node3 dials: 192.168.1.147:27000-27002 (what works)
• Validators advertise: 172.20.0.x (what they think)
• Bootstrap logic must use dial_data, not identify info
```

### Configuration:
- **Docker Compose**: `docker-compose.yml`
- **Network**: Single `testnet` (172.20.0.0/16)
- **Configs**: `single-network-configs/`

### Address Challenge:
- **Node3 dials**: `192.168.1.147:27000/27001/27002` (host IP + port mapping) ✅
- **Validators advertise**: `172.20.0.10/11/12` (container IPs) in `identify` protocol ❌
- **Bootstrap matching**: Must use `dial_data` addresses that worked for connection

### Test Case:
Validates that bootstrap identification works when external nodes connect through Docker port mapping.

### Applicability:
**Common scenarios**: AWS ECS/EKS port mapping, Kubernetes NodePort services, or cloud load balancers where external clients connect via public IPs but services advertise internal container IPs.

Container orchestration environments where:
- **AWS ECS/EKS**: External clients connect via Application Load Balancer public IP, but containers advertise internal cluster IPs
- **Kubernetes NodePort**: External services reach pods via worker node IPs + port mapping, but pods advertise their internal cluster IPs
- **Docker Swarm**: External traffic routes through manager node IPs, but services advertise internal overlay network IPs
- **Cloud Load Balancers**: External traffic hits public load balancer IPs, but backend services advertise private subnet IPs

**Note**: The `make testnet-mixed` command uses the same network topology as `make testnet` (single-network scenario) but tests validators 0..2 with different binary versions than node3:
- **Nodes 0,1,2**: Old version binaries with commit #befe02ace90ea38eed4795d42157b3bfe61d0572, before the reconnect feature was introduced. See `./makefile-scripts/setup-mixed-testnet.sh`  
- **Node 3**: Latest version (current HEAD)

This validates that P2P discovery and bootstrap identification work correctly across different software versions.

---

## `make testnet-multi` - Multi-Network Address Mismatch

**Purpose**: Simulates enterprise networks with multiple network segments.

### Network Topology:
```
VALIDATORS NETWORK       PUBLIC NETWORK           FULLNODE NETWORK
172.21.0.0/16           172.23.0.0/16             172.22.0.0/16
(internal)              (bridge)                  (external)

+-----------------+     +-----------------+     +-----------------+
|     node0       |<--->|     node0       |<--->|                 |
|  172.21.0.10    |     |  172.23.0.10    |     |                 |
|    :27000       |     |    :27000       |     |                 |
+-----------------+     +-----------------+     |                 |
                                                |     node3       |
+-----------------+     +-----------------+     |  172.22.0.13    |
|     node1       |<--->|     node1       |<--->|    :27000       |
|  172.21.0.11    |     |  172.23.0.11    |     |                 |
|    :27000       |     |    :27000       |     | Dials:          |
+-----------------+     +-----------------+     | 172.23.0.10     |
                                                | 172.23.0.11     |
+-----------------+     +-----------------+     | 172.23.0.12     |
|     node2       |<--->|     node2       |<--->|                 |
|  172.21.0.12    |     |  172.23.0.12    |     |                 |
|    :27000       |     |    :27000       |     |                 |
+-----------------+     +-----------------+     +-----------------+

Multi-homed validators have IPs on both networks:
• Internal network: 172.21.0.x (what they advertise)
• Public network: 172.23.0.x (what node3 can reach)

ADDRESS MISMATCH TEST:
• Node3 dials: 172.23.0.x (public network - what works)
• Validators advertise: 172.21.0.x (internal network - what they think)
• Bootstrap logic must use dial_data, not identify info
```

### Configuration:
- **Docker Compose**: `docker-compose-multi-network.yml`
- **Networks**: 
  - `validators_net` (172.21.0.0/16) - Internal validator cluster
  - `public_net` (172.23.0.0/16) - Bridge network  
  - `fullnode_net` (172.22.0.0/16) - External node network
- **Configs**: `multi-net-configs/`

### Address Mismatch:
- **Node3 dials**: `172.23.0.10/11/12` (public network - what works) ✅
- **Validators advertise**: `172.21.0.10/11/12` (internal network - what they think) ❌
- **Test**: Validates that bootstrap matching uses working addresses, not advertised ones

### Applicability:
**Common scenarios**: Multi-cloud deployments, corporate networks with DMZ zones, or microservices across different VPCs where services are reachable via bridge networks but advertise their internal segment IPs.

Enterprise environments where:
- **Multi-cloud deployments**: Validators run in internal Kubernetes cluster, external nodes connect via load balancer
- **Corporate networks with DMZ zones**: Internal services on private VLANs, external access via DMZ bridge networks
- **Microservices across VPCs**: Services reachable via VPC peering or transit gateways, but advertise internal subnet IPs
- **Hybrid cloud**: On-premises services reachable via VPN or dedicated links, but advertise internal network addresses

---

## `make testnet-nat` - NAT with Gateway

**Purpose**: Real NAT scenario with external nodes through NAT gateway.

### Network Topology:
```
PRIVATE NETWORK          NAT GATEWAY              EXTERNAL NETWORK
192.168.100.0/24         (dual-homed)             10.0.1.0/24

+------------------+     +------------------+     +------------------+
|      node0       |<--->|   socat proxy    |<--->|      node3       |
| 192.168.100.10   |     | 192.168.100.254  |     |   10.0.1.13      |
|    :27000        |     |                  |     |    :27000        |
+------------------+     |   10.0.1.254     |     |                  |
                         |                  |     | Dials:           |
+------------------+     | Port Forward:    |     | 10.0.1.254:27000 |
|      node1       |<--->| :27000->10:27000 |     | 10.0.1.254:27001 |
| 192.168.100.11   |     | :27001->11:27000 |     | 10.0.1.254:27002 |
|    :27000        |     | :27002->12:27000 |     |                  |
+------------------+     |                  |     +------------------+
                         |                  |
+------------------+     |                  |
|      node2       |<--->|                  |
| 192.168.100.12   |     |                  |
|    :27000        |     |                  |
+------------------+     +------------------+

ADDRESS MISMATCH TEST:
• Node3 dials: 10.0.1.254:27000-27002 (what works)
• Validators advertise: 192.168.100.x (what they think)
• Bootstrap logic must use dial_data, not identify info
```

### Configuration:
- **Docker Compose**: `docker-compose-nat.yml`
- **Networks**:
  - `internal_net` (192.168.100.0/24) - Private validator network
  - `external_net` (10.0.1.0/24) - External node network
- **NAT Gateway**: Ubuntu container with socat port forwarding
- **Configs**: `nat-configs/`

### NAT Translation:
- **Node3 dials**: `10.0.1.254:27000/27001/27002` (NAT gateway) ✅
- **NAT Gateway translates**:
  - `:27000` → `192.168.100.10:27000` (node0)
  - `:27001` → `192.168.100.11:27000` (node1)
  - `:27002` → `192.168.100.12:27000` (node2)
- **Validators advertise**: `192.168.100.10/11/12` (completely different network) ❌

### Applicability:
**Common scenarios**: Corporate firewalls with NAT, home networks behind routers, or cloud NAT gateways where external nodes connect through completely different IP address spaces (e.g., public 203.x.x.x to private 192.168.x.x).

NAT-based environments where:
- **Corporate firewalls with NAT**: Validators in private subnet (192.168.x.x), external nodes connect via public IPs through NAT gateway
- **Home networks behind routers**: Internal services on private IPs, external access via router's public IP and port forwarding
- **Cloud NAT gateways**: Private cloud instances accessible via NAT gateway or load balancer with completely different address spaces
- **Container networks**: Services in private overlay networks, external access via host port mapping or ingress controllers

---


## 🛠️ Usage

### Commands:
```bash
make testnet          # Single network + host IP scenario
make testnet-mixed    # Single network + host IP scenario + nixed binaries
make testnet-multi    # Multi-network address mismatch
make testnet-nat      # True NAT with gateway
make testnet-mixed    # Mixed version testing
make test-integration # Run basic integration test, starts network, checks sockets, restarts validators 0..2
```

### Network Analysis:
```bash
# Check socket connections for any running testnet
./makefile-scripts/check-socket-leaks-simple.sh

# Monitor socket connections for any running testnet
./makefile-scripts/check-socket-leaks-simple.sh monitor

# View logs for specific scenarios
docker compose logs node3 --follow                             # Standard testnet
docker compose -f docker-compose-multi-network.yml logs node3  # Multi-network
docker compose -f docker-compose-nat.yml logs node3            # NAT scenario
```

### Structure:
```
code/
├── makefile-scripts/          # Scripts for tesnet setup and monitoring 
├── single-network-configs/    # Standard single network scenario
├── multi-net-configs/         # Multi-network address mismatch
├── nat-configs/               # True NAT with gateway
├── docker-compose.yml         # Standard testnet
├── docker-compose-multi-network.yml # Multi network testnet
└── docker-compose-nat.yml     # NAT testnet
```



