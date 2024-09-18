#!/bin/bash

sed -i -e 's,^export MALACHITE_DIR=.*,export MALACHITE_DIR=/root/malachite,' -e 's,^export IS_CC=.*,export IS_CC=1,' /etc/profile.d/commands.sh
