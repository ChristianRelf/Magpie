//! SQLite persistence for the harness.
//!
//! A single connection guarded by a mutex is sufficient for the harness's
//! write volume (one row per execution) and keeps the store simple. All
//! blocking calls are short; the engine invokes them from async code via
//! `spawn_blocking` where latency matters.

mod migrations;
pub mod paths;
pub mod usage;

use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use magpie_core::*;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub use usage::*;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

impl From<StoreError> for HarnessError {
    fn from(e: StoreError) -> Self {
        HarnessError::internal(e.to_string())
    }
}

pub type StoreResult<T> = Result<T, StoreError>;

pub struct Store {
    conn: Mutex<Connection>,
}

/// Serialize a unit enum to its serde string form.
pub(crate) fn enum_str<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

pub(crate) fn parse_enum<T: DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

pub(crate) fn ms(t: Timestamp) -> i64 {
    t.timestamp_millis()
}

pub(crate) fn from_ms(v: i64) -> Timestamp {
    Utc.timestamp_millis_opt(v).single().unwrap_or_else(Utc::now)
}

fn parse_ts(s: Option<String>) -> Option<Timestamp> {
    s.and_then(|s| DateTime::parse_from_rfc3339(&s).ok()).map(|d| d.with_timezone(&Utc))
}

/// A local API client key. Only the hash of the token is stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiClient {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub scopes: Vec<String>,
    pub created_at: Timestamp,
    pub last_used_at: Option<Timestamp>,
    pub revoked_at: Option<Timestamp>,
}

impl Store {
    pub fn open(path: &Path) -> StoreResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| StoreError::Other(e.to_string()))?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> StoreResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> StoreResult<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let store = Self { conn: Mutex::new(conn) };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> StoreResult<()> {
        let mut conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL);",
        )?;
        for (name, sql) in migrations::MIGRATIONS {
            let applied: bool = conn
                .query_row("SELECT 1 FROM schema_migrations WHERE name = ?1", [name], |_| Ok(true))
                .optional()?
                .unwrap_or(false);
            if applied {
                continue;
            }
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, ?2)",
                params![name, Utc::now().to_rfc3339()],
            )?;
            tx.commit()?;
            tracing::info!(migration = name, "applied database migration");
        }
        Ok(())
    }

    pub fn applied_migrations(&self) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT name FROM schema_migrations ORDER BY name")?;
        let rows = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    // ---------------------------------------------------------------- kv

    pub fn get_kv<T: DeserializeOwned>(&self, key: &str) -> StoreResult<Option<T>> {
        let conn = self.conn.lock();
        let v: Option<String> = conn
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
            .optional()?;
        match v {
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
            None => Ok(None),
        }
    }

    pub fn set_kv<T: Serialize>(&self, key: &str, value: &T) -> StoreResult<()> {
        let s = serde_json::to_string(value)?;
        self.conn.lock().execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, s],
        )?;
        Ok(())
    }

    pub fn settings(&self) -> StoreResult<Settings> {
        Ok(self.get_kv("settings")?.unwrap_or_default())
    }

    pub fn save_settings(&self, s: &Settings) -> StoreResult<()> {
        self.set_kv("settings", s)
    }

    pub fn routing_config(&self) -> StoreResult<RoutingConfig> {
        Ok(self.get_kv("routing")?.unwrap_or_default())
    }

    pub fn save_routing_config(&self, r: &RoutingConfig) -> StoreResult<()> {
        self.set_kv("routing", r)
    }

    // ---------------------------------------------------------- accounts

    fn row_to_account(r: &Row) -> rusqlite::Result<Account> {
        let kind: String = r.get("kind")?;
        let auth: String = r.get("auth_method")?;
        let billing: String = r.get("billing_mode")?;
        let status: String = r.get("status")?;
        let options: String = r.get("options")?;
        Ok(Account {
            id: r.get("id")?,
            kind: ProviderKind::parse(&kind).unwrap_or(ProviderKind::OpenAiCompatible),
            label: r.get("label")?,
            auth_method: parse_enum(&auth).unwrap_or(AuthMethod::ApiKey),
            billing_mode: parse_enum(&billing).unwrap_or(BillingMode::Unknown),
            billing_reported: r.get::<_, i64>("billing_reported")? != 0,
            base_url: r.get("base_url")?,
            identity: r.get("identity")?,
            plan: r.get("plan")?,
            status: parse_enum(&status).unwrap_or(ConnectionStatus::Pending),
            status_message: r.get("status_message")?,
            enabled: r.get::<_, i64>("enabled")? != 0,
            last_verified_at: parse_ts(r.get("last_verified_at")?),
            created_at: parse_ts(r.get("created_at")?).unwrap_or_else(Utc::now),
            has_secret: r.get::<_, i64>("has_secret")? != 0,
            secret_store: r.get("secret_store")?,
            options: serde_json::from_str(&options).unwrap_or(serde_json::Value::Null),
        })
    }

    pub fn list_accounts(&self) -> StoreResult<Vec<Account>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT * FROM accounts ORDER BY sort_order, created_at")?;
        let rows = stmt.query_map([], Self::row_to_account)?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn get_account(&self, id: &str) -> StoreResult<Option<Account>> {
        let conn = self.conn.lock();
        Ok(conn.query_row("SELECT * FROM accounts WHERE id = ?1", [id], Self::row_to_account).optional()?)
    }

    pub fn upsert_account(&self, a: &Account) -> StoreResult<()> {
        self.conn.lock().execute(
            r#"INSERT INTO accounts (id, kind, label, auth_method, billing_mode, billing_reported, base_url,
                identity, plan, status, status_message, enabled, last_verified_at, created_at, has_secret,
                secret_store, options)
              VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
              ON CONFLICT(id) DO UPDATE SET
                label=excluded.label, auth_method=excluded.auth_method, billing_mode=excluded.billing_mode,
                billing_reported=excluded.billing_reported, base_url=excluded.base_url, identity=excluded.identity,
                plan=excluded.plan, status=excluded.status, status_message=excluded.status_message,
                enabled=excluded.enabled, last_verified_at=excluded.last_verified_at, has_secret=excluded.has_secret,
                secret_store=excluded.secret_store, options=excluded.options"#,
            params![
                a.id,
                a.kind.as_str(),
                a.label,
                enum_str(&a.auth_method),
                enum_str(&a.billing_mode),
                a.billing_reported as i64,
                a.base_url,
                a.identity,
                a.plan,
                enum_str(&a.status),
                a.status_message,
                a.enabled as i64,
                a.last_verified_at.map(|t| t.to_rfc3339()),
                a.created_at.to_rfc3339(),
                a.has_secret as i64,
                a.secret_store,
                serde_json::to_string(&a.options)?,
            ],
        )?;
        Ok(())
    }

    pub fn delete_account(&self, id: &str) -> StoreResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM models WHERE account_id = ?1", [id])?;
        conn.execute("DELETE FROM limits WHERE account_id = ?1", [id])?;
        conn.execute("DELETE FROM model_prefs WHERE model_key LIKE ?1", [format!("{id}/%")])?;
        conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        Ok(())
    }

    // ------------------------------------------------------------ models

    /// Replace the discovered model list for an account.
    pub fn replace_models(&self, account_id: &str, models: &[DiscoveredModel]) -> StoreResult<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM models WHERE account_id = ?1", [account_id])?;
        let now = Utc::now().to_rfc3339();
        for m in models {
            tx.execute(
                "INSERT OR REPLACE INTO models (account_id, model_id, data, discovered_at) VALUES (?1, ?2, ?3, ?4)",
                params![account_id, m.model_id, serde_json::to_string(m)?, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// All discovered models with their discovery timestamp.
    pub fn list_models(&self) -> StoreResult<Vec<(String, DiscoveredModel, Timestamp)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT account_id, data, discovered_at FROM models ORDER BY account_id, model_id")?;
        let rows = stmt
            .query_map([], |r| {
                let data: String = r.get(1)?;
                Ok((r.get::<_, String>(0)?, data, parse_ts(r.get(2)?).unwrap_or_else(Utc::now)))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(a, d, t)| serde_json::from_str(&d).ok().map(|m| (a, m, t)))
            .collect())
    }

    pub fn model_prefs(&self) -> StoreResult<std::collections::HashMap<String, ModelPreference>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT model_key, data FROM model_prefs")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().filter_map(|(k, d)| serde_json::from_str(&d).ok().map(|p| (k, p))).collect())
    }

    pub fn set_model_pref(&self, key: &str, pref: &ModelPreference) -> StoreResult<()> {
        let conn = self.conn.lock();
        if *pref == ModelPreference::default() {
            conn.execute("DELETE FROM model_prefs WHERE model_key = ?1", [key])?;
        } else {
            conn.execute(
                "INSERT INTO model_prefs (model_key, data) VALUES (?1, ?2) ON CONFLICT(model_key) DO UPDATE SET data = excluded.data",
                params![key, serde_json::to_string(pref)?],
            )?;
        }
        Ok(())
    }

    // ------------------------------------------------------------ limits

    pub fn upsert_limits(&self, windows: &[LimitWindow]) -> StoreResult<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for w in windows {
            tx.execute(
                "INSERT INTO limits (account_id, key, data, observed_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(account_id, key) DO UPDATE SET data = excluded.data, observed_at = excluded.observed_at",
                params![w.account_id, w.key, serde_json::to_string(w)?, ms(w.observed_at)],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_limits(&self) -> StoreResult<Vec<LimitWindow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT data FROM limits ORDER BY account_id, key")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().filter_map(|d| serde_json::from_str(&d).ok()).collect())
    }

    pub fn clear_limits(&self, account_id: &str) -> StoreResult<()> {
        self.conn.lock().execute("DELETE FROM limits WHERE account_id = ?1", [account_id])?;
        Ok(())
    }

    // -------------------------------------------------------- executions

    pub fn insert_execution(&self, e: &ExecutionRecord) -> StoreResult<()> {
        self.write_execution(e, true)
    }

    pub fn update_execution(&self, e: &ExecutionRecord) -> StoreResult<()> {
        self.write_execution(e, false)
    }

    fn write_execution(&self, e: &ExecutionRecord, insert: bool) -> StoreResult<()> {
        #[derive(Serialize)]
        struct Detail<'a> {
            attempts: &'a [AttemptRecord],
            routing: &'a Option<RoutingDecision>,
            error: &'a Option<HarnessError>,
        }
        let detail = serde_json::to_string(&Detail { attempts: &e.attempts, routing: &e.routing, error: &e.error })?;
        let sql = if insert {
            "INSERT INTO executions (id, created_at, completed_at, client, status, task, complexity, preset,
                account_id, provider, model_id, model_key, model_name, input_tokens, output_tokens, cached_tokens,
                reasoning_tokens, usage_provenance, cost_usd, cost_provenance, cost_api_equivalent, duration_ms,
                ttft_ms, finish_reason, error_kind, error_message, stream, attempts, detail, request_content,
                response_content)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31)"
        } else {
            "UPDATE executions SET created_at=?2, completed_at=?3, client=?4, status=?5, task=?6, complexity=?7,
                preset=?8, account_id=?9, provider=?10, model_id=?11, model_key=?12, model_name=?13,
                input_tokens=?14, output_tokens=?15, cached_tokens=?16, reasoning_tokens=?17, usage_provenance=?18,
                cost_usd=?19, cost_provenance=?20, cost_api_equivalent=?21, duration_ms=?22, ttft_ms=?23,
                finish_reason=?24, error_kind=?25, error_message=?26, stream=?27, attempts=?28, detail=?29,
                request_content=?30, response_content=?31
             WHERE id=?1"
        };
        let m = e.model.as_ref();
        self.conn.lock().execute(
            sql,
            params![
                e.id,
                ms(e.created_at),
                e.completed_at.map(ms),
                e.client,
                e.status.as_str(),
                e.task.as_str(),
                enum_str(&e.complexity),
                e.preset.as_str(),
                m.map(|m| m.account_id.clone()),
                m.map(|m| m.provider.as_str()),
                m.map(|m| m.model_id.clone()),
                m.map(|m| m.key.clone()),
                m.map(|m| m.display_name.clone()),
                e.usage.input_tokens.map(|v| v as i64),
                e.usage.output_tokens.map(|v| v as i64),
                e.usage.cached_input_tokens.map(|v| v as i64),
                e.usage.reasoning_tokens.map(|v| v as i64),
                enum_str(&e.usage.provenance),
                e.cost.map(|c| c.usd),
                e.cost.map(|c| enum_str(&c.provenance)),
                e.cost.map(|c| c.api_equivalent as i64).unwrap_or(0),
                e.duration_ms.map(|v| v as i64),
                e.time_to_first_token_ms.map(|v| v as i64),
                e.finish_reason.map(|f| enum_str(&f)),
                e.error.as_ref().map(|er| er.kind.as_str()),
                e.error.as_ref().map(|er| er.message.clone()),
                e.stream as i64,
                e.attempts.len() as i64,
                detail,
                e.request_content.as_ref().map(|v| v.to_string()),
                e.response_content,
            ],
        )?;
        Ok(())
    }

    fn row_to_execution(r: &Row, full: bool) -> rusqlite::Result<ExecutionRecord> {
        #[derive(Deserialize, Default)]
        struct Detail {
            #[serde(default)]
            attempts: Vec<AttemptRecord>,
            #[serde(default)]
            routing: Option<RoutingDecision>,
            #[serde(default)]
            error: Option<HarnessError>,
        }
        let provider: Option<String> = r.get("provider")?;
        let model = match (r.get::<_, Option<String>>("model_key")?, provider) {
            (Some(key), Some(p)) => Some(SelectedModel {
                key,
                account_id: r.get::<_, Option<String>>("account_id")?.unwrap_or_default(),
                provider: ProviderKind::parse(&p).unwrap_or(ProviderKind::OpenAiCompatible),
                model_id: r.get::<_, Option<String>>("model_id")?.unwrap_or_default(),
                display_name: r.get::<_, Option<String>>("model_name")?.unwrap_or_default(),
            }),
            _ => None,
        };
        let detail_s: String = r.get("detail")?;
        let detail: Detail = if full { serde_json::from_str(&detail_s).unwrap_or_default() } else { Detail::default() };
        let error = detail.error.or_else(|| {
            let kind: Option<String> = r.get("error_kind").ok().flatten();
            let msg: Option<String> = r.get("error_message").ok().flatten();
            kind.map(|k| {
                HarnessError::new(parse_enum(&k).unwrap_or(ErrorKind::Internal), msg.unwrap_or_default())
            })
        });
        let cost_usd: Option<f64> = r.get("cost_usd")?;
        let cost_prov: Option<String> = r.get("cost_provenance")?;
        let req_content: Option<String> = if full { r.get("request_content")? } else { None };
        let attempts_count: i64 = r.get("attempts")?;
        let mut attempts = detail.attempts;
        if !full && attempts.is_empty() && attempts_count > 0 {
            // Summary rows only need the attempt count for display.
            if let Some(m) = &model {
                for _ in 0..attempts_count {
                    attempts.push(AttemptRecord { model: m.clone(), started_at: from_ms(0), duration_ms: 0, error: None });
                }
            }
        }
        Ok(ExecutionRecord {
            id: r.get("id")?,
            created_at: from_ms(r.get("created_at")?),
            completed_at: r.get::<_, Option<i64>>("completed_at")?.map(from_ms),
            client: r.get("client")?,
            status: ExecutionStatus::parse(&r.get::<_, String>("status")?),
            task: TaskClass::parse(&r.get::<_, String>("task")?).unwrap_or(TaskClass::General),
            complexity: parse_enum(&r.get::<_, String>("complexity")?).unwrap_or(Complexity::Medium),
            preset: RoutingPreset::parse(&r.get::<_, String>("preset")?).unwrap_or(RoutingPreset::Automatic),
            model,
            usage: TokenUsage {
                input_tokens: r.get::<_, Option<i64>>("input_tokens")?.map(|v| v as u64),
                output_tokens: r.get::<_, Option<i64>>("output_tokens")?.map(|v| v as u64),
                cached_input_tokens: r.get::<_, Option<i64>>("cached_tokens")?.map(|v| v as u64),
                cache_write_tokens: None,
                reasoning_tokens: r.get::<_, Option<i64>>("reasoning_tokens")?.map(|v| v as u64),
                provenance: parse_enum(&r.get::<_, String>("usage_provenance")?).unwrap_or_default(),
            },
            cost: cost_usd.map(|usd| Cost {
                usd,
                provenance: cost_prov.and_then(|p| parse_enum(&p)).unwrap_or(Provenance::Estimated),
                api_equivalent: r.get::<_, i64>("cost_api_equivalent").unwrap_or(0) != 0,
            }),
            duration_ms: r.get::<_, Option<i64>>("duration_ms")?.map(|v| v as u64),
            time_to_first_token_ms: r.get::<_, Option<i64>>("ttft_ms")?.map(|v| v as u64),
            finish_reason: r.get::<_, Option<String>>("finish_reason")?.and_then(|f| parse_enum(&f)),
            error,
            stream: r.get::<_, i64>("stream")? != 0,
            attempts,
            routing: detail.routing,
            request_content: req_content.and_then(|s| serde_json::from_str(&s).ok()),
            response_content: if full { r.get("response_content")? } else { None },
        })
    }

    pub fn get_execution(&self, id: &str) -> StoreResult<Option<ExecutionRecord>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row("SELECT * FROM executions WHERE id = ?1", [id], |r| Self::row_to_execution(r, true))
            .optional()?)
    }

    /// Recent executions, newest first. Summary rows omit routing detail and
    /// content to keep list queries cheap.
    pub fn list_executions(&self, q: &ExecutionQuery) -> StoreResult<Vec<ExecutionRecord>> {
        let conn = self.conn.lock();
        let (where_sql, args) = q.where_clause();
        let sql = format!(
            "SELECT * FROM executions {where_sql} ORDER BY created_at DESC LIMIT {} OFFSET {}",
            q.limit.clamp(1, 5000),
            q.offset
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), |r| Self::row_to_execution(r, false))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn count_executions(&self, q: &ExecutionQuery) -> StoreResult<u64> {
        let conn = self.conn.lock();
        let (where_sql, args) = q.where_clause();
        let n: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM executions {where_sql}"),
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }

    /// Mark executions left running by a crashed harness as failed.
    pub fn recover_interrupted(&self) -> StoreResult<usize> {
        let n = self.conn.lock().execute(
            "UPDATE executions SET status = 'failed', error_kind = 'internal',
               error_message = 'Harness stopped before the execution completed', completed_at = created_at
             WHERE status = 'running'",
            [],
        )?;
        Ok(n)
    }

    /// Delete executions older than `days` days. Returns rows removed.
    pub fn apply_retention(&self, days: u32) -> StoreResult<usize> {
        if days == 0 {
            return Ok(0);
        }
        let cutoff = ms(Utc::now() - chrono::Duration::days(days as i64));
        let conn = self.conn.lock();
        let n = conn.execute("DELETE FROM executions WHERE created_at < ?1", [cutoff])?;
        conn.execute("DELETE FROM notifications WHERE created_at < ?1", [cutoff])?;
        Ok(n)
    }

    pub fn clear_history(&self) -> StoreResult<usize> {
        let conn = self.conn.lock();
        let n = conn.execute("DELETE FROM executions WHERE status != 'running'", [])?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(n)
    }

    /// Remove stored request/response content (when retention is disabled).
    pub fn purge_content(&self) -> StoreResult<usize> {
        Ok(self.conn.lock().execute(
            "UPDATE executions SET request_content = NULL, response_content = NULL
             WHERE request_content IS NOT NULL OR response_content IS NOT NULL",
            [],
        )?)
    }

    // ------------------------------------------------------- api clients

    pub fn insert_client(&self, c: &ApiClient, token_hash: &str) -> StoreResult<()> {
        self.conn.lock().execute(
            "INSERT INTO api_clients (id, name, token_hash, prefix, scopes, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![c.id, c.name, token_hash, c.prefix, c.scopes.join(","), c.created_at.to_rfc3339()],
        )?;
        Ok(())
    }

    fn row_to_client(r: &Row) -> rusqlite::Result<ApiClient> {
        let scopes: String = r.get("scopes")?;
        Ok(ApiClient {
            id: r.get("id")?,
            name: r.get("name")?,
            prefix: r.get("prefix")?,
            scopes: scopes.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect(),
            created_at: parse_ts(r.get("created_at")?).unwrap_or_else(Utc::now),
            last_used_at: parse_ts(r.get("last_used_at")?),
            revoked_at: parse_ts(r.get("revoked_at")?),
        })
    }

    pub fn list_clients(&self) -> StoreResult<Vec<ApiClient>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT * FROM api_clients ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], Self::row_to_client)?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Look up an active client by token hash.
    pub fn client_by_hash(&self, hash: &str) -> StoreResult<Option<ApiClient>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT * FROM api_clients WHERE token_hash = ?1 AND revoked_at IS NULL",
                [hash],
                Self::row_to_client,
            )
            .optional()?)
    }

    pub fn touch_client(&self, id: &str) -> StoreResult<()> {
        self.conn
            .lock()
            .execute("UPDATE api_clients SET last_used_at = ?2 WHERE id = ?1", params![id, Utc::now().to_rfc3339()])?;
        Ok(())
    }

    pub fn revoke_client(&self, id: &str) -> StoreResult<bool> {
        let n = self.conn.lock().execute(
            "UPDATE api_clients SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
            params![id, Utc::now().to_rfc3339()],
        )?;
        Ok(n > 0)
    }

    // ----------------------------------------------------- notifications

    pub fn insert_notification(&self, n: &Notification) -> StoreResult<()> {
        self.conn.lock().execute(
            "INSERT INTO notifications (id, kind, title, body, account_id, created_at, read) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![n.id, enum_str(&n.kind), n.title, n.body, n.account_id, ms(n.created_at), n.read as i64],
        )?;
        Ok(())
    }

    pub fn list_notifications(&self, limit: u32) -> StoreResult<Vec<Notification>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, kind, title, body, account_id, created_at, read FROM notifications ORDER BY created_at DESC LIMIT {}",
            limit.clamp(1, 500)
        ))?;
        let rows = stmt
            .query_map([], |r| {
                let kind: String = r.get(1)?;
                Ok((kind, r.get(0)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get::<_, i64>(6)?))
            })?
            .collect::<Result<Vec<(String, String, String, String, Option<String>, i64, i64)>, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(kind, id, title, body, account_id, created, read)| {
                Some(Notification {
                    id,
                    kind: parse_enum(&kind)?,
                    title,
                    body,
                    account_id,
                    created_at: from_ms(created),
                    read: read != 0,
                })
            })
            .collect())
    }

    pub fn mark_notifications_read(&self) -> StoreResult<()> {
        self.conn.lock().execute("UPDATE notifications SET read = 1 WHERE read = 0", [])?;
        Ok(())
    }

    pub(crate) fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> StoreResult<T>) -> StoreResult<T> {
        let conn = self.conn.lock();
        f(&conn)
    }
}

/// Filters for execution listings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExecutionQuery {
    pub limit: u32,
    pub offset: u32,
    pub status: Option<String>,
    pub provider: Option<String>,
    pub account_id: Option<String>,
    pub model_key: Option<String>,
    pub task: Option<String>,
    /// Unix milliseconds.
    pub from: Option<i64>,
    pub to: Option<i64>,
}

impl ExecutionQuery {
    fn where_clause(&self) -> (String, Vec<rusqlite::types::Value>) {
        use rusqlite::types::Value;
        let mut clauses = Vec::new();
        let mut args: Vec<Value> = Vec::new();
        let mut push = |col: &str, v: Value| {
            args.push(v);
            clauses.push(format!("{col} ?{}", args.len()));
        };
        if let Some(s) = &self.status {
            push("status =", Value::Text(s.clone()));
        }
        if let Some(s) = &self.provider {
            push("provider =", Value::Text(s.clone()));
        }
        if let Some(s) = &self.account_id {
            push("account_id =", Value::Text(s.clone()));
        }
        if let Some(s) = &self.model_key {
            push("model_key =", Value::Text(s.clone()));
        }
        if let Some(s) = &self.task {
            push("task =", Value::Text(s.clone()));
        }
        if let Some(v) = self.from {
            push("created_at >=", Value::Integer(v));
        }
        if let Some(v) = self.to {
            push("created_at <", Value::Integer(v));
        }
        if clauses.is_empty() {
            (String::new(), args)
        } else {
            (format!("WHERE {}", clauses.join(" AND ")), args)
        }
    }
}

#[cfg(test)]
mod tests;
