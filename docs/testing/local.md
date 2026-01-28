# Testing Malachite locally within Docker

> Commands prefixed with `$` must be ran on the host machine, while commands prefixed with `#` must be run in the container.

## Host requirements:

- Git
- Docker

## Setup

1. Clone Malachite

```
$ git clone https://github.com/circlefin/malachite.git
$ cd malachite
```

2. Create a Docker container with the NET_ADMIN capability, mounting the repository inside as a volume

```
$ docker run -it -v .:/app --cap-add=NET_ADMIN rust:1-slim /bin/bash
```

3. From within the container, install the required packages

```
# apt update
# apt install -y wget unzip git iproute2 procps iputils-ping curl vim make
```

4. Build Malachite

```
# cd app/code
# cargo build --release
```

5. Introduce network latency (optional)

To introduce 50ms latency in both ingress and egress on the loopback interface,
so a total 100ms round-trip time between localhost to itself:

```
# tc qdisc add dev lo root handle 1:0 netem delay 50msec
```

To introduce packet loss as well, add `loss X%`:

```
# tc qdisc add dev lo root handle 1:0 netem delay 50msec loss 1%
```

To restore it back to normal:

```
# tc qdisc del dev lo root
```

**Note:** To modify these settings, you first need to disable them using the command above before re-introducing them.

6. Test the latency to localhost

```
# ping 127.0.0.1
PING 127.0.0.1 (127.0.0.1) 56(84) bytes of data.
64 bytes from 127.0.0.1: icmp_seq=1 ttl=64 time=110 ms
64 bytes from 127.0.0.1: icmp_seq=2 ttl=64 time=110 ms
64 bytes from 127.0.0.1: icmp_seq=3 ttl=64 time=109 ms
```

7. Generate the testnet configuration in the directory `x`

```
# cargo run --release -- testnet --nodes 20 --home x -d
```

8. In `scripts/spawn.bash`, modify the environment variables controlling the config according your needs

```
# vim scripts/spawn.bash
```

9. Run the devnet

```
# ./scripts/spawn.bash --nodes 20 --home x
```

10. Open another session to the container from the host machine

For this, first find the name of the container that is running, then open a new bash shell within it:

```
$ docker ps
$ docker exec -it CONTAINER_NAME /bin/bash
```

11. In this new session, check the logs

```
# tail -f app/code/x/0/logs/node.log
```

12. Check the metrics

For the block time:

```
# curl -s localhost:29000/metrics | grep 'time_per_block_[sum|count]'
```

For the number of rounds per block:

```
# curl -s localhost:29000/metrics | grep time_per_block
```

For the latency as seen by libp2p:

```
# curl -s localhost:29000/metrics | grep 'consensus_libp2p_ping_rtt_seconds'
```

