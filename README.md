# drogue-ajour

The Drogue Ajour is an add-on for Drogue IoT Cloud that allows you to manage firmware updates for a fleet of devices for Drogue IoT Cloud applications. It can be used to monitor all applications accessible by a Drogue Cloud access token, or a single application.

You can run Drogue Ajour locally or on Kubernetes (recommended for production).

## Architecture

Drogue Ajour uses the Drogue IoT Cloud event stream and command API to communicate with devices that have firmware update capability. The Drogue IoT device registry is used to store the 'desired' firmware and version for a given device. The `Application` and `Device` objects need to use an extended schema as follows:

```
spec:
  firmware:
    oci:
      image: 'image-within-repo:1234'
```

If defined at the `Application` it will apply to all devices unless there is a per device property, in which case that will override the application level property.

As a reference architecture, Drogue Ajour uses an Open Container Initiative (OCI) registry to store container images with the device firmware. This allows reusing existing infrastructure commonly used in Kubernetes. However, the goal is
to support alternatives like Eclipse Hawkbit for storing the firmware.

Building and deploying firmware is decoupled from the Drogue Ajour, as long as the expected manifest format can be retrieved from a firmware metadata (in case of OCI, as a label on the container image). As a reference architecture,
a [tekton](tekton.dev) pipeline is used to build and deploy firmware to the registry, and a local file database for storing an index of image versions that are built (expect this to move to some kind of SQL database eventually). 

The Drogue IoT Application and Device objects contain custom properties that define the desired firmware version that each device (or all) should be served.
```
+----------------+          +------------------+          +--------------+ 
|                | -------> |                  | -------> |              | 
| Device/Gateway |          | Drogue IoT Cloud |          | Drogue Ajour | 
|                | <------- |                  | <------- |              | 
+----------------+          +------------------+          +--------------+ 
                                                                  |
                                                                  |
                                                                  |
                                                                  |
                +-------------------------+          +-------------------+
                |                         |          |                   |
                | Firmware Build Pipeline | -------> | Firmware Registry |
                |                         |          |                   |
                +-------------------------+          +-------------------+
```

## Protocol

Drogue Ajour uses a custom application level protocol sent on the 'dfu' channel and 'dfu' command to communicate with a device or gateway. The protocol is stateless, meaning that Drogue Ajour will track only send out firmware updates to devices that are reporting their status.

The protocol is designed so that devices do not have to be online continuously during the update, but can receive firmware at their own pace.

The protocol uses Consise Binary Object Representation (CBOR), to ensure a small message size that works well with embedded devices. For descriptive purposes, the examples here are provided in JSON, with CBOR values to follow once implementation has started.

A typical firmware update cycle runs as follows:

1. At any given time, a device publishes message to the 'dfu' channel with the following payload

```
{
   "version": "0.1.0",
   "correlation_id": 1 // Optional: Opaque request identifier, for devices that aggregate multiple updates
   "mtu": 512, // Optional: The desired block size to use for firmware blobs
}
```

This allows the Drogue Ajour to check if an update is necessary at all.

2. Drogue Ajour checks the desired firmware for that particular device and will attempt to locate it in the firmware registry.

3a. If the device is already updated, a 'dfu' command is sent to the device with the following payload:

```
{
  "sync": {
    "version": "0.1.0", // Desired firmware version
    "correlation_id": 1 // Optional: Should be set to the id from the device status
    "poll": 300, // The amount of time to wait before checking in again
  }
}
```

3b. If a device needs to be updated, a 'dfu' command is sent to the device with the following payload

```
{
  "write": {
    "version": "0.1.1", // Version of blob
    "offset": 0, // Offset this blob should be written to
    "data": aGVsbG8= // Base64-encoded firmware block (Binary in CBOR)
  }
}
```

4a. When a device is seeing the "sync" operation, it should make sure it's current firmware version is marked as 'good' if it have not already done so. The 'poll' field can be used as a heuristic of how long the device should wait before checking in again.

4b. When a device is seeing the "write" operation, it should write the provided data to it's storage. Once persisted, it should report back the next expected offset and last received version:

```
{
   "version": "0.1.0",
   "mtu": 512, // The desired block size to use for the next firmware block
   "status": {
      "version": "0.1.1", // Version of the last block the device received from the server. To safeguard against new versions arriving
      "offset": 512, // Current write offset after applying the last firmware block.
   }
}
```

5b. When the server receives the next event from the device, it repeats step 4b. until the complete firmware have been sent. When the final block is confirmed written, and it is desired to deploy the new firmware, the fleet-manager will send the following command:

```
{
  "swap": {
    "version": "0.1.1",
    "checksum": "097d501382f5817eb2c5f741d37845158c76dd6a8af899001b36b5a75188aeeb"
  }
}
```

6b. When the device receives the 'swap' command, it should initiate the firmware update and report back with the updated version as soon as it's back online.

## Client

The [`drgdfu`](https://github.com/drogue-iot/drgdfu) implements the above protocol, either using a file as the server or using the Drogue IoT cloud endpoints acting as a gateway device.
