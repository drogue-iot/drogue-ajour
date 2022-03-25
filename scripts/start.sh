#!/bin/sh
/drogue-ajour --oci-registry-prefix ${OCI_REGISTRY_PREFIX} --oci-registry-token $(cat /var/run/secrets/kubernetes.io/serviceaccount/token) --mqtt-uri ssl://${DROGUE_MQTT_INTEGRATION} --application ${DROGUE_APPLICATION} --token ${DROGUE_TOKEN} --user ${DROGUE_USER} --device-registry ${DROGUE_DEVICE_REGISTRY}
