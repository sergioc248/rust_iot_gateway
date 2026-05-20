CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    external_id TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE ingest_events (
    id BIGSERIAL PRIMARY KEY,
    ingest_id UUID NOT NULL UNIQUE,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    received_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE units (
    id BIGSERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    symbol TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE measurement_definitions (
    id BIGSERIAL PRIMARY KEY,
    metric_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    value_type TEXT NOT NULL CHECK (value_type IN ('number', 'string', 'boolean')),
    canonical_unit_id BIGINT REFERENCES units(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE measurements (
    id BIGSERIAL PRIMARY KEY,
    event_id BIGINT NOT NULL REFERENCES ingest_events(id) ON DELETE CASCADE,
    definition_id BIGINT NOT NULL REFERENCES measurement_definitions(id),
    value_type TEXT NOT NULL CHECK (value_type IN ('number', 'string', 'boolean')),
    value_number DOUBLE PRECISION,
    value_text TEXT,
    value_bool BOOLEAN,
    unit_id BIGINT REFERENCES units(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (
        (value_number IS NOT NULL)::int +
        (value_text IS NOT NULL)::int +
        (value_bool IS NOT NULL)::int = 1
    )
);

CREATE INDEX idx_ingest_events_device_received_at
ON ingest_events (device_id, received_at DESC);

CREATE INDEX idx_measurements_event_id
ON measurements (event_id);

CREATE INDEX idx_measurements_definition_id
ON measurements (definition_id);

INSERT INTO units (code, display_name, symbol)
VALUES
    ('celsius', 'Celsius', 'degC'),
    ('percent', 'Percent', '%');

INSERT INTO measurement_definitions (metric_key, display_name, value_type, canonical_unit_id)
VALUES
    ('temperature', 'Temperature', 'number', (SELECT id FROM units WHERE code = 'celsius')),
    ('humidity', 'Humidity', 'number', (SELECT id FROM units WHERE code = 'percent'));
