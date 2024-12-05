#!/usr/bin/env bash

for file in Cargo.toml src tests benches; do
  ditto $HOME/Informal/Code/malachite-wal/$file ./$file
done

