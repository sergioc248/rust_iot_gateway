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
GRID_FILA=1
GRID_COL=1
```

Supported presets are `logistica`, `clima_externo`, `energia`, `calefaccion`, and `cultivo_cacao`.
`GRID_FILA` and `GRID_COL` are optional and intended for `cultivo_cacao` nodes with fixed grid coordinates.

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
docker run -d --name sim-cultivo-cacao-01 -e GATEWAY_URL=https://example.com -e DEVICE_ID=nodo-cultivo-cacao-01 -e PRESET=cultivo_cacao telemetry-simulator
```

## VPS Simulator Fleet

Use `compose.simulators.yml` to run the unattended simulator fleet on one VPS. It starts 11 containers:

| Count | Preset | Device IDs |
| --- | --- | --- |
| 2 | `logistica` | `nodo-logistica-01` to `nodo-logistica-02` |
| 2 | `clima_externo` | `nodo-clima-externo-01` to `nodo-clima-externo-02` |
| 2 | `energia` | `nodo-energia-01` to `nodo-energia-02` |
| 5 | `cultivo_cacao` | `nodo-cultivo-cacao-01` to `nodo-cultivo-cacao-05` |

Create a VPS `.env` next to the compose file:

```bash
GATEWAY_URL=https://your-gateway.example.com
INTERVAL_SECONDS=60
REQUEST_TIMEOUT_SECONDS=10
SIMULATOR_RUST_LOG=telemetry_simulator=info
```

Start or update the fleet:

```bash
docker compose -f compose.simulators.yml up -d --build
```

Docker Compose uses `restart: unless-stopped`, so containers restart after crashes and host reboots unless you explicitly stop them.

Check status and logs:

```bash
docker compose -f compose.simulators.yml ps
docker compose -f compose.simulators.yml logs -f
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

### `cultivo_cacao`

For `Nodo Cultivo de Cacao` and other crop base nodes.

Payload fields:

```json
{
  "tempC": 25.7,
  "humedadAirePct": 82.4,
  "bateriaPct": 76.2,
  "rssiDbm": -64,
  "gridFila": 2,
  "gridCol": 4
}
```

Ranges:

| Field | Range |
| --- | --- |
| `tempC` | -40.0 to 80.0 C |
| `humedadAirePct` | 0.0 to 100.0% |
| `bateriaPct` | 0.0 to 100.0% |
| `rssiDbm` | -90 to -30 dBm |
| `gridFila` | 1 to 5 |
| `gridCol` | 1 to 5 |
