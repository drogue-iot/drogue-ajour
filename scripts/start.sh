#!/bin/sh
#OCI_REGISTRY_TOKEN=$(cat /var/run/secrets/kubernetes.io/serviceaccount/token)
/drogue-ajour --oci-registry-token ${OCI_REGISTRY_TOKEN} --oci-registry-tls --oci-registry-prefix ${OCI_REGISTRY_PREFIX} --mqtt-uri ssl://${DROGUE_MQTT_INTEGRATION} --application ${DROGUE_APPLICATION} --token ${DROGUE_TOKEN} --user ${DROGUE_USER} --device-registry ${DROGUE_DEVICE_REGISTRY} --oci-registry-insecure
