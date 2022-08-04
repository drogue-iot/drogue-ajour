#!/bin/sh
ARGS=""

ARGS="${ARGS} --namespace ${NAMESPACE}"
ARGS="${ARGS} --port 8080"
ARGS="${ARGS} --device-registry ${DEVICE_REGISTRY}"
ARGS="${ARGS} --allowed-applications ${ALLOWED_APPLICATIONS}"

/drogue-ajour-api ${ARGS}
