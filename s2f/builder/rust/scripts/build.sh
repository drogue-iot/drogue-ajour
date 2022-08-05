#!/usr/bin/env bash
set -x
set -e

PROJECT=$1

pushd ${PROJECT}

REVISION=$(git rev-parse --short HEAD | tr -d '\n')
echo "Building firmware"
REVISION=${REVISION} cargo build --release ${BUILD_ARGS}

echo "Creating binary file"
REVISION=${REVISION} cargo objcopy --release ${BUILD_ARGS} -- -O binary artifact.bin

popd
