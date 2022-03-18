# fleet-manager

The Drogue IoT fleet manager is an add-on for Drogue IoT Cloud that allows you to build and manage firmware for a fleet of devices for a Drogue IoT Cloud application.

You can run the fleet manager locally or on Kubernetes (recommended for production).

## Architecture

The fleet manager uses the Drogue IoT Cloud event stream and command API to communicate with devices that have firmware update capability. The Drogue IoT device registry is used to store the 'desired' firmware and version for a given device.

An Open Container Initiative (OCI) registry is used to store images containing the device firmware. This allows reusing existing infrastructure commonly used in Kubernetes. 

Building and deploying firmware to the OCI registry is decoupled from the fleet manager, as long as the expected manifest is part of the container image. As a reference architecture, fleet manager uses [tekton](tekton.dev) pipelines to build firmware, and a local file database for storing an index of image versions that are built (expect this to move to some kind of SQL database eventually). 

NOTE: At present, only the 'latest' built version will be served. In the future, additional metadata to the Drogue IoT Application and Device types will make it possible to select the firmware version to serve.

```
+----------------+          +------------------+          +---------------+ 
|                | -------> |                  | -------> |               | 
| Device/Gateway |          | Drogue IoT Cloud |          | Fleet Manager | 
|                | <------- |                  | <------- |               | 
+----------------+          +------------------+          +---------------+ 
                                                                  |
                                                                  |
                                                                  |
                                                                  |
                      +-------------------------+          +--------------+
                      |                         |          |              |
                      | Firmware Build Pipeline | -------> | OCI Registry |
                      |                         |          |              |
                      +-------------------------+          +--------------+
```

## Protocol

The fleet manager uses a custom application level protocol sent on the 'dfu' channel and 'dfu' command to communicate with a device or gateway. The protocol is stateless, meaning the fleet manager will track only send out firmware updates to devices that are requesting the fleet manager to sync them.

The protocol is designed so that devices do not have to be online continuously during the update, but can consume firmware at their own pace.

The protocol uses Consise Binary Object Representation (CBOR), to ensure a small message size that works well with embedded devices. For descriptive purposes, the examples here are provided in JSON, with CBOR values to follow once implementation has started.

A typical firmware update cycle runs as follows:

1. At any given time, a device publishes message to the 'dfu' channel with the following payload

```
{
   "version": "0.1.0",
   "mtu": 512, // Optional: The desired block size to use for firmware blobs
}
```

This allows the fleet-manager to check if an update is necessary at all.

2. Fleet manager checks the latest (or desired eventually) firmware for that particular device. 

3a. If the device is already updated, the fleet manager sends a 'dfu' command to the device with the following payload:

```
{
  "op": "SYNC",
  "version": "0.1.0",
  "poll": 300, // The amount of time to wait before checking in again
}
```

3b. If a device needs to be updated, fleet manager sends a 'dfu' command to the device with the following payload

```
{
  "op": "WRITE"
  "version": "0.1.1",
  "offset": 0, // Offset in firmware exchange
  "size": 5, // Size of data
  "data": aGVsbG8= // Base64-encoded firmware block
}
```

4a. When a device is seeing the "SYNC" operation, it should make sure it's current firmware version is marked as 'good'. The 'poll' field can be used as a heuristic of how long the device should wait before checking in again.

4b. When a device is seeing the "WRITE" operation, it should write the provided data to it's storage. Once persisted, it should report back using the same payload as in (1) but with some additional fields:


```
{
   "version": "0.1.0",
   "mtu": 512, // The desired block size to use for the next firmware block
   "status": {
      "version": "0.1.1", // Version of the last block the device received from the server. To safeguard against new versions arriving
      "offset": 0, // Current write offset after applying the last firmware block.
   }
}
```

5b. When the server receives the next event from the device, it repeats step 4b. until the complete firmware have been sent. When the final block is confirmed written, and it is desired to deploy the new firmware, the fleet-manager will send the following command:

```
{
  "op": "SWAP",
  "version": "0.1.1",
  "checksum": "097d501382f5817eb2c5f741d37845158c76dd6a8af899001b36b5a75188aeeb"
}
```

6b. When the device receives the 'SWAP' command, it should initiate the firmware update and report back with the updated version as soon as it's back online.
