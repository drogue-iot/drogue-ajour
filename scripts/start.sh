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
ARGS="${ARGS} --oci-cache-expiry 30"
ARGS="${ARGS} --oci-registry-enable"

if [ "${MQTT_GROUP_ID}" != "" ]; then
    ARGS="${ARGS} --mqtt-group-id ${MQTT_GROUP_ID}"
fi

if [ "${DROGUE_APPLICATION}" != "" ]; then
    ARGS="${ARGS} --application ${DROGUE_APPLICATION}"
fi

/drogue-ajour ${ARGS}
