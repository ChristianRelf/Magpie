//! Aggregation queries for analytics. All figures are computed from the
//! local execution log and therefore describe usage *through the harness*,
//! not total provider-account usage.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{Store, StoreResult};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageSummary {
    pub from: i64,
    pub to: i64,
    pub requests: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub running: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub reasoning_tokens: u64,
    /// Requests whose token counts were reported by the provider.
    pub tokens_reported_requests: u64,
    /// Requests whose token counts were estimated locally.
    pub tokens_estimated_requests: u64,
    /// Cost reported by providers (actual charges).
    pub cost_reported_usd: f64,
    /// Cost computed from token counts and configured prices.
    pub cost_calculated_usd: f64,
    /// Cost estimated from catalog prices or estimated token counts.
    pub cost_estimated_usd: f64,
    /// API-equivalent value of subscription-backed usage (not a charge).
    pub api_equivalent_usd: f64,
    pub avg_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub avg_ttft_ms: Option<f64>,
    pub requests_per_minute: f64,
    pub fallbacks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesPoint {
    /// Bucket start, unix milliseconds.
    pub t: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub requests: u64,
    pub failed: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub avg_duration_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakdownRow {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub requests: u64,
    pub failed: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub avg_duration_ms: Option<f64>,
}

/// Historical performance used by the router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub model_key: String,
    pub requests: u64,
    pub failures: u64,
    pub avg_duration_ms: Option<f64>,
    pub avg_ttft_ms: Option<f64>,
    /// Output tokens per second across successful requests.
    pub output_tps: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    None,
    Provider,
    Model,
    Account,
    Task,
    Status,
}

impl GroupBy {
    fn column(self) -> Option<&'static str> {
        match self {
            GroupBy::None => None,
            GroupBy::Provider => Some("provider"),
            GroupBy::Model => Some("model_key"),
            GroupBy::Account => Some("account_id"),
            GroupBy::Task => Some("task"),
            GroupBy::Status => Some("status"),
        }
    }
}

/// Optional filters shared by analytics queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UsageFilter {
    pub provider: Option<String>,
    pub account_id: Option<String>,
    pub model_key: Option<String>,
}

impl UsageFilter {
    fn sql(&self, start_idx: usize) -> (String, Vec<String>) {
        let mut parts = Vec::new();
        let mut args = Vec::new();
        for (col, val) in [("provider", &self.provider), ("account_id", &self.account_id), ("model_key", &self.model_key)] {
            if let Some(v) = val {
                args.push(v.clone());
                parts.push(format!(" AND {col} = ?{}", start_idx + args.len()));
            }
        }
        (parts.concat(), args)
    }
}

impl Store {
    pub fn usage_summary(&self, from: i64, to: i64, filter: &UsageFilter) -> StoreResult<UsageSummary> {
        let (fsql, fargs) = filter.sql(2);
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT
                    COUNT(*),
                    SUM(status = 'succeeded'), SUM(status = 'failed'), SUM(status = 'cancelled'), SUM(status = 'running'),
                    COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cached_tokens),0), COALESCE(SUM(reasoning_tokens),0),
                    SUM(usage_provenance = 'reported'), SUM(usage_provenance = 'estimated'),
                    COALESCE(SUM(CASE WHEN cost_provenance = 'reported' AND cost_api_equivalent = 0 THEN cost_usd END),0),
                    COALESCE(SUM(CASE WHEN cost_provenance = 'calculated' AND cost_api_equivalent = 0 THEN cost_usd END),0),
                    COALESCE(SUM(CASE WHEN cost_provenance = 'estimated' AND cost_api_equivalent = 0 THEN cost_usd END),0),
                    COALESCE(SUM(CASE WHEN cost_api_equivalent = 1 THEN cost_usd END),0),
                    AVG(CASE WHEN status = 'succeeded' THEN duration_ms END),
                    AVG(CASE WHEN status = 'succeeded' THEN ttft_ms END),
                    SUM(attempts > 1)
                 FROM executions WHERE created_at >= ?1 AND created_at < ?2{fsql}"
            );
            let mut args: Vec<rusqlite::types::Value> =
                vec![rusqlite::types::Value::Integer(from), rusqlite::types::Value::Integer(to)];
            args.extend(fargs.iter().cloned().map(rusqlite::types::Value::Text));
            let mut s = conn.query_row(&sql, rusqlite::params_from_iter(args.iter()), |r| {
                let u = |i: usize| -> rusqlite::Result<u64> { Ok(r.get::<_, Option<i64>>(i)?.unwrap_or(0) as u64) };
                Ok(UsageSummary {
                    from,
                    to,
                    requests: u(0)?,
                    succeeded: u(1)?,
                    failed: u(2)?,
                    cancelled: u(3)?,
                    running: u(4)?,
                    input_tokens: u(5)?,
                    output_tokens: u(6)?,
                    cached_tokens: u(7)?,
                    reasoning_tokens: u(8)?,
                    tokens_reported_requests: u(9)?,
                    tokens_estimated_requests: u(10)?,
                    cost_reported_usd: r.get(11)?,
                    cost_calculated_usd: r.get(12)?,
                    cost_estimated_usd: r.get(13)?,
                    api_equivalent_usd: r.get(14)?,
                    avg_duration_ms: r.get(15)?,
                    avg_ttft_ms: r.get(16)?,
                    fallbacks: u(17)?,
                    p95_duration_ms: None,
                    requests_per_minute: 0.0,
                })
            })?;
            let succeeded = s.succeeded as i64;
            if succeeded > 0 {
                let offset = ((succeeded as f64) * 0.95).floor() as i64;
                let offset = offset.min(succeeded - 1);
                let sql = format!(
                    "SELECT duration_ms FROM executions WHERE created_at >= ?1 AND created_at < ?2 AND status = 'succeeded'
                     AND duration_ms IS NOT NULL{fsql} ORDER BY duration_ms LIMIT 1 OFFSET {offset}"
                );
                s.p95_duration_ms = conn
                    .query_row(&sql, rusqlite::params_from_iter(args.iter()), |r| r.get::<_, i64>(0))
                    .ok()
                    .map(|v| v as f64);
            }
            let minutes = ((to - from) as f64 / 60_000.0).max(1.0);
            s.requests_per_minute = s.requests as f64 / minutes;
            Ok(s)
        })
    }

    pub fn usage_timeseries(
        &self,
        from: i64,
        to: i64,
        bucket_ms: i64,
        group_by: GroupBy,
        filter: &UsageFilter,
    ) -> StoreResult<Vec<TimeseriesPoint>> {
        let bucket_ms = bucket_ms.max(60_000);
        let group_col = group_by.column();
        let (fsql, fargs) = filter.sql(3);
        self.with_conn(|conn| {
            let gsel = group_col.map(|c| format!(", COALESCE({c}, 'unknown')")).unwrap_or_else(|| ", NULL".into());
            let ggroup = group_col.map(|_| ", 2").unwrap_or_default();
            let sql = format!(
                "SELECT ((created_at - ?1) / ?3) * ?3 + ?1 AS bucket{gsel},
                        COUNT(*), SUM(status = 'failed'),
                        COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                        COALESCE(SUM(CASE WHEN cost_api_equivalent = 0 THEN cost_usd END),0),
                        AVG(CASE WHEN status = 'succeeded' THEN duration_ms END)
                 FROM executions WHERE created_at >= ?1 AND created_at < ?2{fsql}
                 GROUP BY 1{ggroup} ORDER BY 1"
            );
            let mut args: Vec<rusqlite::types::Value> = vec![from.into(), to.into(), bucket_ms.into()];
            args.extend(fargs.iter().cloned().map(rusqlite::types::Value::Text));
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                    Ok(TimeseriesPoint {
                        t: r.get(0)?,
                        group: r.get(1)?,
                        requests: r.get::<_, i64>(2)? as u64,
                        failed: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                        input_tokens: r.get::<_, i64>(4)? as u64,
                        output_tokens: r.get::<_, i64>(5)? as u64,
                        cost_usd: r.get(6)?,
                        avg_duration_ms: r.get(7)?,
                    })
                })?
                .collect::<Result<_, _>>()?;
            Ok(rows)
        })
    }

    pub fn usage_breakdown(&self, from: i64, to: i64, group_by: GroupBy, filter: &UsageFilter) -> StoreResult<Vec<BreakdownRow>> {
        let col = group_by.column().unwrap_or("provider");
        let label = if col == "model_key" { "MAX(model_name)" } else { col };
        let (fsql, fargs) = filter.sql(2);
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT COALESCE({col}, 'unknown'), COALESCE({label}, 'Unrouted'), MAX(provider),
                        COUNT(*), SUM(status = 'failed'),
                        COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                        COALESCE(SUM(CASE WHEN cost_api_equivalent = 0 THEN cost_usd END),0),
                        AVG(CASE WHEN status = 'succeeded' THEN duration_ms END)
                 FROM executions WHERE created_at >= ?1 AND created_at < ?2{fsql}
                 GROUP BY 1 ORDER BY (COALESCE(SUM(input_tokens),0) + COALESCE(SUM(output_tokens),0)) DESC, 4 DESC"
            );
            let mut args: Vec<rusqlite::types::Value> = vec![from.into(), to.into()];
            args.extend(fargs.iter().cloned().map(rusqlite::types::Value::Text));
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                    Ok(BreakdownRow {
                        key: r.get(0)?,
                        label: r.get(1)?,
                        provider: r.get(2)?,
                        requests: r.get::<_, i64>(3)? as u64,
                        failed: r.get::<_, Option<i64>>(4)?.unwrap_or(0) as u64,
                        input_tokens: r.get::<_, i64>(5)? as u64,
                        output_tokens: r.get::<_, i64>(6)? as u64,
                        cost_usd: r.get(7)?,
                        avg_duration_ms: r.get(8)?,
                    })
                })?
                .collect::<Result<_, _>>()?;
            Ok(rows)
        })
    }

    /// Per-model performance over the trailing window, for routing.
    pub fn model_stats(&self, since: i64) -> StoreResult<Vec<ModelStats>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT model_key, COUNT(*), SUM(status = 'failed'),
                        AVG(CASE WHEN status = 'succeeded' THEN duration_ms END),
                        AVG(CASE WHEN status = 'succeeded' THEN ttft_ms END),
                        SUM(CASE WHEN status = 'succeeded' AND duration_ms > 0 THEN output_tokens END) * 1000.0 /
                          NULLIF(SUM(CASE WHEN status = 'succeeded' AND duration_ms > 0 AND output_tokens IS NOT NULL THEN duration_ms END), 0)
                 FROM executions WHERE created_at >= ?1 AND model_key IS NOT NULL AND status != 'running'
                 GROUP BY model_key",
            )?;
            let rows = stmt
                .query_map(params![since], |r| {
                    Ok(ModelStats {
                        model_key: r.get(0)?,
                        requests: r.get::<_, i64>(1)? as u64,
                        failures: r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                        avg_duration_ms: r.get(3)?,
                        avg_ttft_ms: r.get(4)?,
                        output_tps: r.get(5)?,
                    })
                })?
                .collect::<Result<_, _>>()?;
            Ok(rows)
        })
    }

    /// Sum of non-subscription spend since `since` (for spend alerts and
    /// budget checks).
    pub fn spend_since(&self, since: i64) -> StoreResult<f64> {
        self.with_conn(|conn| {
            Ok(conn.query_row(
                "SELECT COALESCE(SUM(cost_usd), 0) FROM executions WHERE created_at >= ?1 AND cost_api_equivalent = 0",
                params![since],
                |r| r.get(0),
            )?)
        })
    }

    /// Local token usage per account since `since`, used to show harness
    /// consumption against provider limits.
    pub fn account_usage_since(&self, since: i64) -> StoreResult<Vec<(String, u64, u64)>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT account_id, COUNT(*), COALESCE(SUM(input_tokens),0) + COALESCE(SUM(output_tokens),0)
                 FROM executions WHERE created_at >= ?1 AND account_id IS NOT NULL GROUP BY account_id",
            )?;
            let rows = stmt
                .query_map(params![since], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)? as u64))
                })?
                .collect::<Result<_, _>>()?;
            Ok(rows)
        })
    }
}
