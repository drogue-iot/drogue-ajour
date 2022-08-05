#!/usr/bin/env bash

set -e

mkdir -p "${TRUNK_STAGING_DIR}/endpoints"
cat<<EOF > ${TRUNK_STAGING_DIR}/endpoints/backend.json
{
    "client_id": "drogue",
    "issuer_url": "https://sso.sandbox.drogue.cloud/realms/drogue",
    "drogue_api_url": "https://api.sandbox.drogue.cloud",
    "ajour_api_url": "https://api.firmware.sandbox.drogue.cloud"
}
EOF

    #"ajour_api_url": "http://localhost:8080"
