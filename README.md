# Rust IoT Gateway

Small Rust gateway that accepts IoT telemetry through `POST /devices/{device_id}/ingest` and stores each ingest event in Postgres.

This repository also includes a dockerizable telemetry simulator for deploying many simulated devices by changing environment variables.

## Gateway

Required environment variables:

```bash
DATABASE_URL=postgresql://USER:PASSWORD@HOST/DB?sslmode=require
PORT=3000
RUST_LOG=rust_iot_gateway=info
```

Run locally:

```bash
cargo run --bin rust_iot_gateway
```

Build the gateway image:

```bash
docker build -t rust-iot-gateway .
```

## Telemetry Simulator

The simulator sends JSON payloads to the gateway ingest endpoint on a fixed interval.

Required environment variables:

```bash
GATEWAY_URL=http://localhost:3000
```

Optional environment variables:

```bash
DEVICE_ID=simulated-logistica-01
PRESET=logistica
INTERVAL_SECONDS=60
REQUEST_TIMEOUT_SECONDS=10
RUST_LOG=telemetry_simulator=info
```

`GATEWAY_URL` can be either the gateway base URL or a full ingest URL containing `{device_id}`:

```bash
GATEWAY_URL=https://example.com
GATEWAY_URL=https://example.com/devices/{device_id}/ingest
```

Run locally:

```bash
cargo run --bin telemetry_simulator
```

Build the simulator image:

```bash
docker build -f Dockerfile.simulator -t telemetry-simulator .
```

Run a simulated logistics station:

```bash
docker run --rm \
  -e GATEWAY_URL=https://rust-iot-gateway-fragrant-harborbird-9015.fly.dev \
  -e DEVICE_ID=nodo-logistica-01 \
  -e PRESET=logistica \
  -e INTERVAL_SECONDS=60 \
  telemetry-simulator
```

Run multiple simulated containers by changing `DEVICE_ID` and `PRESET`:

```bash
docker run -d --name sim-logistica-01 -e GATEWAY_URL=https://example.com -e DEVICE_ID=nodo-logistica-01 -e PRESET=logistica telemetry-simulator
docker run -d --name sim-clima-01 -e GATEWAY_URL=https://example.com -e DEVICE_ID=nodo-clima-externo-01 -e PRESET=clima_externo telemetry-simulator
docker run -d --name sim-energia-01 -e GATEWAY_URL=https://example.com -e DEVICE_ID=nodo-energia-01 -e PRESET=energia telemetry-simulator
docker run -d --name sim-calefaccion-01 -e GATEWAY_URL=https://example.com -e DEVICE_ID=nodo-calefaccion-01 -e PRESET=calefaccion telemetry-simulator
```

## Simulator Presets

### `logistica`

For `Nodo Estación de Logística`.

Payload fields:

```json
{
  "pesoKg": 12.34,
  "presencia": true,
  "tempZonaC": 24.1,
  "bateriaPct": 88.5
}
```

Ranges:

| Field | Range |
| --- | --- |
| `pesoKg` | 0.00 to 50.00 kg |
| `presencia` | `true` or `false` |
| `tempZonaC` | 18.0 to 30.0 C |
| `bateriaPct` | 20.0 to 100.0% |

### `clima_externo`

For `Nodo Estación Clima Externo`.

Payload fields:

```json
{
  "tempExternaC": 28.4,
  "humedadExternaPct": 78.2,
  "vientoMps": 4.6,
  "lluviaMm": 12.3
}
```

Ranges:

| Field | Range |
| --- | --- |
| `tempExternaC` | 15.0 to 38.0 C |
| `humedadExternaPct` | 40.0 to 100.0% |
| `vientoMps` | 0.0 to 25.0 m/s |
| `lluviaMm` | 0.0 to 150.0 mm |

### `energia`

For `Nodo Monitoreo Energia`.

Payload fields:

```json
{
  "voltajeV": 12.56,
  "corrienteA": 1.21,
  "potenciaW": 15.2,
  "rssiDbm": -61
}
```

Ranges:

| Field | Range |
| --- | --- |
| `voltajeV` | 10.50 to 14.80 V |
| `corrienteA` | 0.00 to 2.50 A |
| `potenciaW` | Calculated from voltage and current, capped at 35.0 W |
| `rssiDbm` | -85 to -40 dBm |

### `calefaccion`

For `Nodo Monitoreo Calefacción`.

Payload fields:

```json
{
  "tempSalidaC": 52.8,
  "potenciaW": 940.5,
  "velVentiladorPct": 62.4
}
```

Ranges:

| Field | Range |
| --- | --- |
| `tempSalidaC` | 20.0 to 75.0 C |
| `potenciaW` | 0.0 to 1500.0 W |
| `velVentiladorPct` | 0.0 to 100.0% |
