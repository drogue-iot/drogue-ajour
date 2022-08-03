#!/bin/sh
ARGS=""

ARGS="${ARGS} --namespace ${NAMESPACE}"
ARGS="${ARGS} --port 8080"
ARGS="${ARGS} --device-registry ${DEVICE_REGISTRY}"

/drogue-ajour-api ${ARGS}
