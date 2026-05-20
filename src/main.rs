use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::{env, net::SocketAddr};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct RootResponse {
    service: &'static str,
    description: &'static str,
    health_endpoint: &'static str,
    ingest_endpoint: &'static str,
}

#[derive(Debug, Deserialize)]
struct IngestRequest {
    timestamp: Option<DateTime<Utc>>,
    payload: Map<String, Value>,
}

#[derive(Debug, Serialize)]
struct IngestResponse {
    ingest_id: Uuid,
    device_id: String,
    received_at: DateTime<Utc>,
    measurements_count: usize,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

struct MeasurementDefinition {
    id: i64,
    value_type: String,
    canonical_unit_id: Option<i64>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rust_iot_gateway=info,tower_http=info".into()),
        )
        .init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect postgres");

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/devices/{device_id}/ingest", post(ingest))
        .with_state(AppState { db });

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind listener");

    info!(address = %addr, "server listening");

    axum::serve(listener, app).await.expect("serve axum app");
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn root() -> Json<RootResponse> {
    Json(RootResponse {
        service: "rust_iot_gateway",
        description: "Accepts IoT device ingestion payloads and stores them in Postgres.",
        health_endpoint: "/health",
        ingest_endpoint: "/devices/{device_id}/ingest",
    })
}

async fn ingest(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Json(request): Json<IngestRequest>,
) -> Result<Json<IngestResponse>, (StatusCode, Json<Value>)> {
    if request.payload.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "payload must contain at least one measurement",
        ));
    }

    let received_at = request.timestamp.unwrap_or_else(Utc::now);
    let measurements_count = request.payload.len();
    let ingest_id = Uuid::new_v4();

    let mut tx = state.db.begin().await.map_err(internal_error)?;

    let device_row = sqlx::query(
        "INSERT INTO devices (external_id) VALUES ($1) ON CONFLICT (external_id) DO UPDATE SET external_id = EXCLUDED.external_id RETURNING id",
    )
    .bind(&device_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal_error)?;

    let db_device_id: Uuid = device_row.get("id");

    let event_row = sqlx::query(
        "INSERT INTO ingest_events (ingest_id, device_id, received_at, payload) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(ingest_id)
    .bind(db_device_id)
    .bind(received_at)
    .bind(sqlx::types::Json(request.payload.clone()))
    .fetch_one(&mut *tx)
    .await
    .map_err(internal_error)?;

    let event_id: i64 = event_row.get("id");

    for (key, value) in &request.payload {
        let (value_number, value_text, value_bool, value_type) = measurement_parts(value)?;
        let definition = load_or_create_measurement_definition(&mut tx, key, value_type).await?;

        if definition.value_type != value_type {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                &format!("payload value for '{key}' must be {}", definition.value_type),
            ));
        }

        sqlx::query(
            "INSERT INTO measurements (event_id, definition_id, value_type, value_number, value_text, value_bool, unit_id) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(event_id)
        .bind(definition.id)
        .bind(value_type)
        .bind(value_number)
        .bind(value_text)
        .bind(value_bool)
        .bind(definition.canonical_unit_id)
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;
    }

    tx.commit().await.map_err(internal_error)?;

    info!(
        %ingest_id,
        device_id,
        measurements_count,
        "accepted ingest payload"
    );

    Ok(Json(IngestResponse {
        ingest_id,
        device_id,
        received_at,
        measurements_count,
    }))
}

fn measurement_parts(
    value: &Value,
) -> Result<(Option<f64>, Option<String>, Option<bool>, &'static str), (StatusCode, Json<Value>)> {
    match value {
        Value::Number(number) => {
            let Some(parsed) = number.as_f64() else {
                return Err(error_response(
                    StatusCode::BAD_REQUEST,
                    "numeric payload values must fit into f64",
                ));
            };

            Ok((Some(parsed), None, None, "number"))
        }
        Value::String(text) => Ok((None, Some(text.clone()), None, "string")),
        Value::Bool(flag) => Ok((None, None, Some(*flag), "boolean")),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "payload values must be number, string, or boolean",
        )),
    }
}

async fn load_or_create_measurement_definition(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    metric_key: &str,
    value_type: &str,
) -> Result<MeasurementDefinition, (StatusCode, Json<Value>)> {
    let definition_row = sqlx::query(
        "SELECT id, value_type, canonical_unit_id FROM measurement_definitions WHERE metric_key = $1",
    )
    .bind(metric_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(internal_error)?;

    let row = if let Some(row) = definition_row {
        row
    } else {
        sqlx::query(
            "INSERT INTO measurement_definitions (metric_key, display_name, value_type, canonical_unit_id) VALUES ($1, $2, $3, NULL) RETURNING id, value_type, canonical_unit_id",
        )
        .bind(metric_key)
        .bind(display_name(metric_key))
        .bind(value_type)
        .fetch_one(&mut **tx)
        .await
        .map_err(internal_error)?
    };

    Ok(MeasurementDefinition {
        id: row.get("id"),
        value_type: row.get("value_type"),
        canonical_unit_id: row.get("canonical_unit_id"),
    })
}

fn display_name(metric_key: &str) -> String {
    let mut out = String::with_capacity(metric_key.len());

    for (index, part) in metric_key.split('_').filter(|part| !part.is_empty()).enumerate() {
        if index > 0 {
            out.push(' ');
        }

        let mut chars = part.chars();

        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }

    if out.is_empty() {
        metric_key.to_owned()
    } else {
        out
    }
}

fn internal_error(error: sqlx::Error) -> (StatusCode, Json<Value>) {
    error!(?error, "database operation failed");
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}

fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(serde_json::json!(ErrorResponse {
            error: message.to_owned(),
        })),
    )
}
