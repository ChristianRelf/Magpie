//! Schema migrations. Each entry is applied once, in order, inside a
//! transaction. Never edit a released migration; append a new one.

pub const MIGRATIONS: &[(&str, &str)] = &[(
    "0001_initial",
    r#"
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    auth_method TEXT NOT NULL,
    billing_mode TEXT NOT NULL,
    billing_reported INTEGER NOT NULL DEFAULT 0,
    base_url TEXT,
    identity TEXT,
    plan TEXT,
    status TEXT NOT NULL,
    status_message TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_verified_at TEXT,
    created_at TEXT NOT NULL,
    has_secret INTEGER NOT NULL DEFAULT 0,
    secret_store TEXT,
    options TEXT NOT NULL DEFAULT '{}',
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE models (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    data TEXT NOT NULL,
    discovered_at TEXT NOT NULL,
    PRIMARY KEY (account_id, model_id)
);

CREATE TABLE model_prefs (
    model_key TEXT PRIMARY KEY,
    data TEXT NOT NULL
);

CREATE TABLE executions (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    client TEXT NOT NULL,
    status TEXT NOT NULL,
    task TEXT NOT NULL,
    complexity TEXT NOT NULL,
    preset TEXT NOT NULL,
    account_id TEXT,
    provider TEXT,
    model_id TEXT,
    model_key TEXT,
    model_name TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cached_tokens INTEGER,
    reasoning_tokens INTEGER,
    usage_provenance TEXT NOT NULL DEFAULT 'unavailable',
    cost_usd REAL,
    cost_provenance TEXT,
    cost_api_equivalent INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER,
    ttft_ms INTEGER,
    finish_reason TEXT,
    error_kind TEXT,
    error_message TEXT,
    stream INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    detail TEXT NOT NULL DEFAULT '{}',
    request_content TEXT,
    response_content TEXT
);
CREATE INDEX idx_exec_created ON executions(created_at);
CREATE INDEX idx_exec_model ON executions(model_key, created_at);
CREATE INDEX idx_exec_account ON executions(account_id, created_at);
CREATE INDEX idx_exec_status ON executions(status);

CREATE TABLE limits (
    account_id TEXT NOT NULL,
    key TEXT NOT NULL,
    data TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, key)
);

CREATE TABLE kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE api_clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    prefix TEXT NOT NULL,
    scopes TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT
);

CREATE TABLE notifications (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    account_id TEXT,
    created_at INTEGER NOT NULL,
    read INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_notifications_created ON notifications(created_at);
"#,
)];
