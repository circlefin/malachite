#!/bin/bash

# node-done
while [ ! -f /etc/done ];
do
  sleep 5
done

# node-nfs
mount /data
