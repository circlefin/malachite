#!/bin/sh

set -eu

## Get TC parameters (Example: "delay 100ms 10ms")
if [ -z "${TC_PARAMS:-}" ]; then
  TC_PARAMS="$( (grep "^tc_params=" /config/config.toml | cut -d= -f2 | tr -d \") || echo "")"
fi

# Add delay using TC. Make sure the image is running with --privileged or --cap-add=NET_ADMIN.
if [ ! -f /etc/tc_done ] && [ -n "${TC_PARAMS:-}" ]; then
  #shellcheck disable=SC2086
  tc qdisc add dev eth0 root netem $TC_PARAMS
  touch /etc/tc_done
fi

/usr/local/bin/malachite-cli "$@"
