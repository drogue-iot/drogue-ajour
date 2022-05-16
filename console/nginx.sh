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

if [ -z $API_URL ]; then
    export API_URL="https://api.sandbox.drogue.cloud"
fi

echo "Using base config from file: $BACKEND_JSON_FILE"
cat $BACKEND_JSON_FILE | jq --arg client_id "${CLIENT_ID}" '. + {client_id: $client_id}' | jq --arg issuer_url "${ISSUER_URL}" '. + {issuer_url: $issuer_url}' | jq --arg api_url "${API_URL}" '. + {api_url: $api_url}' > /endpoints/backend.json

echo "Final backend information:"
echo "---"
cat /endpoints/backend.json
echo "---"

exec /usr/sbin/nginx -g "daemon off;"
