use chrono::{DateTime, Utc};
use rand::Rng;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{env, error::Error, io, time::Duration};
use tracing::{info, warn};

type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone, Copy)]
enum Preset {
    Logistica,
    ClimaExterno,
    Energia,
    Calefaccion,
}

#[derive(Debug)]
struct Config {
    ingest_url: String,
    device_id: String,
    preset: Preset,
    interval: Duration,
    timeout: Duration,
}

#[derive(Debug, Serialize)]
struct TelemetryRequest {
    timestamp: DateTime<Utc>,
    payload: Map<String, Value>,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "telemetry_simulator=info,reqwest=warn".into()),
        )
        .init();

    let config = Config::from_env()?;
    let client = reqwest::Client::builder().timeout(config.timeout).build()?;

    info!(
        preset = config.preset.as_str(),
        device_id = %config.device_id,
        ingest_url = %config.ingest_url,
        interval_seconds = config.interval.as_secs(),
        "telemetry simulator started"
    );

    loop {
        let request = TelemetryRequest {
            timestamp: Utc::now(),
            payload: config.preset.generate_payload(),
        };

        let payload_log =
            serde_json::to_string(&request.payload).unwrap_or_else(|_| "{}".to_owned());

        match post_telemetry(&client, &config.ingest_url, &request).await {
            Ok(()) => info!(
                preset = config.preset.as_str(),
                device_id = %config.device_id,
                payload = %payload_log,
                "telemetry posted"
            ),
            Err(err) => warn!(
                preset = config.preset.as_str(),
                device_id = %config.device_id,
                error = %err,
                "telemetry post failed"
            ),
        }

        tokio::time::sleep(config.interval).await;
    }
}

impl Config {
    fn from_env() -> AppResult<Self> {
        let gateway_url = required_env("GATEWAY_URL")?;
        let preset: Preset = env::var("PRESET")
            .unwrap_or_else(|_| "logistica".to_owned())
            .parse()?;
        let device_id =
            env::var("DEVICE_ID").unwrap_or_else(|_| format!("simulated-{}", preset.as_str()));
        let interval = Duration::from_secs(parse_env_u64("INTERVAL_SECONDS", 60)?);
        let timeout = Duration::from_secs(parse_env_u64("REQUEST_TIMEOUT_SECONDS", 10)?);

        if interval.is_zero() {
            return Err(invalid_input("INTERVAL_SECONDS must be greater than 0"));
        }

        if timeout.is_zero() {
            return Err(invalid_input("REQUEST_TIMEOUT_SECONDS must be greater than 0"));
        }

        Ok(Self {
            ingest_url: ingest_url(&gateway_url, &device_id),
            device_id,
            preset,
            interval,
            timeout,
        })
    }
}

impl Preset {
    fn as_str(self) -> &'static str {
        match self {
            Self::Logistica => "logistica",
            Self::ClimaExterno => "clima_externo",
            Self::Energia => "energia",
            Self::Calefaccion => "calefaccion",
        }
    }

    fn generate_payload(self) -> Map<String, Value> {
        let mut rng = rand::thread_rng();

        match self {
            Self::Logistica => Map::from_iter([
                (
                    "pesoKg".to_owned(),
                    json!(round2(rng.gen_range(0.0..=50.0))),
                ),
                ("presencia".to_owned(), json!(rng.gen_bool(0.20))),
                (
                    "tempZonaC".to_owned(),
                    json!(round1(rng.gen_range(18.0..=30.0))),
                ),
                (
                    "bateriaPct".to_owned(),
                    json!(round1(rng.gen_range(20.0..=100.0))),
                ),
            ]),
            Self::ClimaExterno => Map::from_iter([
                (
                    "tempExternaC".to_owned(),
                    json!(round1(rng.gen_range(15.0..=38.0))),
                ),
                (
                    "humedadExternaPct".to_owned(),
                    json!(round1(rng.gen_range(40.0..=100.0))),
                ),
                (
                    "vientoMps".to_owned(),
                    json!(round1(rng.gen_range(0.0..=25.0))),
                ),
                (
                    "lluviaMm".to_owned(),
                    json!(round1(rng.gen_range(0.0..=150.0))),
                ),
            ]),
            Self::Energia => {
                let voltaje = round2(rng.gen_range(10.5..=14.8));
                let corriente = round2(rng.gen_range(0.0..=2.5));
                let potencia = round1((voltaje * corriente).min(35.0));

                Map::from_iter([
                    ("voltajeV".to_owned(), json!(voltaje)),
                    ("corrienteA".to_owned(), json!(corriente)),
                    ("potenciaW".to_owned(), json!(potencia)),
                    ("rssiDbm".to_owned(), json!(rng.gen_range(-85..=-40))),
                ])
            }
            Self::Calefaccion => Map::from_iter([
                (
                    "tempSalidaC".to_owned(),
                    json!(round1(rng.gen_range(20.0..=75.0))),
                ),
                (
                    "potenciaW".to_owned(),
                    json!(round1(rng.gen_range(0.0..=1500.0))),
                ),
                (
                    "velVentiladorPct".to_owned(),
                    json!(round1(rng.gen_range(0.0..=100.0))),
                ),
            ]),
        }
    }
}

impl std::str::FromStr for Preset {
    type Err = io::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "logistica" | "logistics" => Ok(Self::Logistica),
            "clima_externo" | "clima-externo" | "weather" => Ok(Self::ClimaExterno),
            "energia" | "energy" => Ok(Self::Energia),
            "calefaccion" | "heating" => Ok(Self::Calefaccion),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PRESET must be one of: logistica, clima_externo, energia, calefaccion",
            )),
        }
    }
}

async fn post_telemetry(
    client: &reqwest::Client,
    ingest_url: &str,
    request: &TelemetryRequest,
) -> AppResult<()> {
    let response = client.post(ingest_url).json(request).send().await?;
    let status = response.status();

    if status.is_success() {
        return Ok(());
    }

    let body = response.text().await.unwrap_or_default();
    Err(Box::new(io::Error::new(
        io::ErrorKind::Other,
        format!("gateway returned {status}: {body}"),
    )))
}

fn ingest_url(gateway_url: &str, device_id: &str) -> String {
    let trimmed = gateway_url.trim().trim_end_matches('/');
    let encoded_device_id = encode_path_segment(device_id);

    if trimmed.contains("{device_id}") {
        trimmed.replace("{device_id}", &encoded_device_id)
    } else if trimmed.ends_with("/ingest") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/devices/{encoded_device_id}/ingest")
    }
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn required_env(name: &str) -> AppResult<String> {
    env::var(name).map_err(|_| invalid_input(&format!("{name} must be set")))
}

fn parse_env_u64(name: &str, default: u64) -> AppResult<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| invalid_input(&format!("{name} must be a positive integer"))),
        Err(_) => Ok(default),
    }
}

fn invalid_input(message: &str) -> Box<dyn Error + Send + Sync> {
    Box::new(io::Error::new(io::ErrorKind::InvalidInput, message.to_owned()))
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
