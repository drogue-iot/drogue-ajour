#!/usr/bin/env bash
set -x
set -e

PROJECT=$1
ARTIFACT=$2

pushd ${PROJECT}
REVISION=$(git rev-parse --short HEAD | tr -d '\n')
SZ=$(du -b ${ARTIFACT} | cut -f1)
CHECKSUM=$(sha256sum ${ARTIFACT} | awk '{ print $1 }')

cp ${ARTIFACT} firmware.bin 
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
