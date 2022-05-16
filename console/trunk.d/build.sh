#!/usr/bin/env bash

set -e

mkdir -p "${TRUNK_STAGING_DIR}/endpoints"
cat<<EOF > ${TRUNK_STAGING_DIR}/endpoints/backend.json
{
    "client_id": "drogue",
    "issuer_url": "https://sso.sandbox.drogue.cloud/auth/realms/drogue",
    "api_url": "https://api.sandbox.drogue.cloud"
}
EOF
