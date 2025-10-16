# OrbitSailor AIS Forwarder

This program listens on a TCP/UDP port and forwards received AIS (Automatic Identification System) data to a configured endpoint.

## Usage

```bash
ais-forwarder [OPTIONS] --udp-listener <SOCKET> | --tcp-listener <SOCKET>
```

## Options

### Required

- `-e`, `--upload-endpoint <URL>`
  The endpoint to which AIS messages will be forwarded.
  Can also be set via the environment variable `UPLOAD_ENDPOINT`.

- `-a`, `--auth-token <STRING>`
  Authentication token used when sending data to the upload endpoint
  Can also be set via the environment variable `AUTH_TOKEN`.

### Listener Ports (at least one required)

You must specify **at least one** of the following:

- `-u`, `--udp-listener <SOCKET>`
  Listen on the specified UDP socket address for AIS messages.
  Expects a single AIS message per packet.

- `-t`, `--tcp-listener <SOCKET>`
  Listen on the specified TCP socket address for AIS messages.

### Optional Flags

- `-l`, `--write-to-stdout`
  Write all forwarded messages to standard output in addition to forwarding them.

- `-p`, `--prefix-current-time`
  Prefix each received line with the current Unix timestamp.

## Environment Variables

You can also configure the application using environment variables:

- `UPLOAD_ENDPOINT` – equivalent to `--upload-endpoint`
- `AUTH_TOKEN` – equivalent to `--auth-token`

## Example

```bash
ais-forwarder \
  --upload-endpoint https://aisinput.orbitsailor.com/dataInput \
  --auth-token mysecrettoken \
  --udp-listener 0.0.0.0:4001 \
  --write-to-stdout \
  --prefix-current-time
```
