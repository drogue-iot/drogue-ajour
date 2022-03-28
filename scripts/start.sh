#!/bin/sh
ARGS=""


if [ -z "${OCI_REGISTRY_TOKEN}" ]; then
    OCI_REGISTRY_TOKEN=$(cat /var/run/secrets/kubernetes.io/serviceaccount/token)
fi

ARGS="${ARGS} --oci-registry-token ${OCI_REGISTRY_TOKEN}"
ARGS="${ARGS} --oci-registry-tls"
ARGS="${ARGS} --oci-registry-prefix ${OCI_REGISTRY_PREFIX}"
ARGS="${ARGS} --mqtt-uri ssl://${DROGUE_MQTT_INTEGRATION}"
ARGS="${ARGS} --token ${DROGUE_TOKEN}"
ARGS="${ARGS} --user ${DROGUE_USER}"
ARGS="${ARGS} --device-registry ${DROGUE_DEVICE_REGISTRY}"
ARGS="${ARGS} --oci-registry-insecure"

if [ "${DROGUE_APPLICATION}" != "" ]; then
    ARGS="${ARGS} --application ${DROGUE_APPLICATION}"
fi

ARGS="${ARGS} --exclude-applications lulf-drogue-1"

/drogue-ajour ${ARGS}
