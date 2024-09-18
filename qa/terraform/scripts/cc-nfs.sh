#!/bin/bash

# NFS server
mkdir /data
chown nobody:nogroup /data
systemctl start nfs-kernel-server
systemctl enable nfs-kernel-server
