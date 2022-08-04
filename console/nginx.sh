#!/usr/bin/env bash
set -e
set -x
set -o pipefail

: "${BACKEND_JSON_FILE:=/backend.template.json}"

if [ -z $CLIENT_ID ]; then
    export CLIENT_ID="drogue"
fi

if [ -z $ISSUER_URL ]; then
    export ISSUER_URL="https://sso.sandbox.drogue.cloud/auth/realms/drogue"
fi

if [ -z $DROGUE_API_URL ]; then
    export DROGUE_API_URL="https://api.sandbox.drogue.cloud"
fi

if [ -z $AJOUR_API_URL ]; then
    export AJOUR_API_URL="https://api.firmware.sandbox.drogue.cloud"
fi

echo "Using base config from file: $BACKEND_JSON_FILE"
cat $BACKEND_JSON_FILE | jq --arg client_id "${CLIENT_ID}" '. + {client_id: $client_id}' | jq --arg issuer_url "${ISSUER_URL}" '. + {issuer_url: $issuer_url}' | jq --arg drogue_api_url "${DROGUE_API_URL}" '. + {drogue_api_url: $drogue_api_url}' | jq --arg ajour_api_url "${AJOUR_API_URL}" '. + {ajour_api_url: $ajour_api_url}'  > /endpoints/backend.json

echo "Final backend information:"
echo "---"
cat /endpoints/backend.json
echo "---"

exec /usr/sbin/nginx -g "daemon off;"
