#!/usr/bin/env bash
set -x
set -e

PROJECT=$1
CARGO_BUILD_ARGS=$2

rustup component add llvm-tools-preview || skip=1

pushd ${PROJECT}
REVISION=$(git rev-parse --short HEAD | tr -d '\n')
echo "Building firmware"
REVISION=${REVISION} cargo build --release ${CARGO_BUILD_ARGS}

echo "Creating binary file"
REVISION=${REVISION} cargo objcopy --release ${CARGO_BUILD_ARGS} -- -O binary firmware.bin
SZ=$(du -b firmware.bin | cut -f1)
CHECKSUM=$(sha256sum firmware.bin | awk '{ print $1 }')

cat<<EOF > firmware.json
{
  "version": "${REVISION}",
  "size": ${SZ},
  "checksum": "${CHECKSUM}"
}
EOF

echo "Generated metadata:"
cat firmware.json
popd
