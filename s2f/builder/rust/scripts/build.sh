#!/usr/bin/env bash
set -x
set -e

REVISION=$(git rev-parse --short HEAD | tr -d '\n')
echo "Building firmware"
REVISION=${REVISION} cargo build --release $@

echo "Creating binary file"
REVISION=${REVISION} cargo objcopy --release $@ -- -O binary firmware.bin
