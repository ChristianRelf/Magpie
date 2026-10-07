use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::{Stream, StreamExt};
use magpie_core::*;
use magpie_engine::{AccountPatch, ConnectRequest, Scope};
use magpie_store::{ExecutionQuery, GroupBy, UsageFilter};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::Auth;
use crate::convert::ResponsesBody;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

pub async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "product": PRODUCT_NAME, "version": VERSION, "api_version": API_VERSION}))
}

pub async fn status(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    let mut v = serde_json::to_value(s.harness.status()).unwrap_or_default();
    v["principal"] = json!(auth.0.name());
    v["is_admin"] = json!(auth.0.has(Scope::Admin));
    Ok(Json(v))
}

// ------------------------------------------------------------------ execute

fn exec_event_name(e: &ExecEvent) -> &'static str {
    match e {
        ExecEvent::Started { .. } => "started",
        ExecEvent::RoutingChanged { .. } => "routing_changed",
        ExecEvent::TextDelta { .. } => "text_delta",
        ExecEvent::ReasoningDelta { .. } => "reasoning_delta",
        ExecEvent::ToolCall { .. } => "tool_call",
        ExecEvent::Completed { .. } => "completed",
        ExecEvent::Failed { .. } => "failed",
    }
}

fn response_json(result: &ExecResult, routing: Option<(TaskClass, Vec<String>)>, notable: &[ExecEvent]) -> Value {
    let fallbacks: Vec<&ExecEvent> = notable.iter().filter(|e| matches!(e, ExecEvent::RoutingChanged { .. })).collect();
    json!({
        "id": result.execution_id,
        "object": "response",
        "status": "completed",
        "model": result.model,
        "output_text": result.output_text,
        "tool_calls": result.tool_calls,
        "finish_reason": result.finish_reason,
        "usage": result.usage,
        "cost": result.cost,
        "duration_ms": result.duration_ms,
        "time_to_first_token_ms": result.time_to_first_token_ms,
        "attempts": result.attempts,
        "routing": routing.map(|(task, reasons)| json!({"task": task, "reasons": reasons})),
        "routing_changes": fallbacks,
        "created_at": result.created_at,
    })
}

pub async fn responses(State(s): State<AppState>, auth: Auth, Json(body): Json<ResponsesBody>) -> ApiResult<Response> {
    auth.require(Scope::Execute)?;
    let req = body.into_request()?;
    let stream = req.stream;
    let client = auth.0.name();
    if !stream {
        let (result, notable) = s.harness.execute_collect(req, client).await?;
        let routing = notable.iter().find_map(|e| match e {
            ExecEvent::Started { task, reasons, .. } => Some((*task, reasons.clone())),
            _ => None,
        });
        return Ok(Json(response_json(&result, routing, &notable)).into_response());
    }
    let handle = s.harness.execute(req, client).await?;
    let events = tokio_stream::wrappers::ReceiverStream::new(handle.events).map(|ev| {
        let name = exec_event_name(&ev);
        Ok::<_, Infallible>(Event::default().event(name).data(serde_json::to_string(&ev).unwrap_or_default()))
    });
    Ok(Sse::new(events).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))).into_response())
}

pub async fn route_preview(State(s): State<AppState>, auth: Auth, Json(body): Json<ResponsesBody>) -> ApiResult<Json<RoutingDecision>> {
    auth.require(Scope::Read)?;
    let req = body.into_request()?;
    Ok(Json(s.harness.route_preview(&req)?))
}

// ---------------------------------------------------------------- providers

pub async fn list_providers(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    let models = s.harness.list_models();
    let limits = s.harness.account_limits();
    let accounts: Vec<Value> = s
        .harness
        .list_accounts()
        .into_iter()
        .map(|a| {
            let mut v = serde_json::to_value(&a).unwrap_or_default();
            let ms: Vec<&ModelInfo> = models.iter().filter(|m| m.account_id == a.id).collect();
            v["model_count"] = json!(ms.len());
            v["available_model_count"] = json!(ms.iter().filter(|m| m.available).count());
            v["capabilities"] = json!({
                "tools": ms.iter().any(|m| m.model.capabilities.tools),
                "vision": ms.iter().any(|m| m.model.capabilities.vision),
                "reasoning": ms.iter().any(|m| m.model.capabilities.reasoning),
                "structured_output": ms.iter().any(|m| m.model.capabilities.structured_output),
                "agentic": ms.iter().any(|m| m.model.capabilities.agentic),
            });
            v["limits"] = serde_json::to_value(limits.iter().find(|l| l.account_id == a.id)).unwrap_or(Value::Null);
            v["descriptor"] = serde_json::to_value(a.kind.descriptor()).unwrap_or_default();
            v
        })
        .collect();
    let kinds: Vec<_> = ProviderKind::ALL.iter().map(|k| k.descriptor()).collect();
    Ok(Json(json!({"accounts": accounts, "kinds": kinds})))
}

pub async fn connect_provider(State(s): State<AppState>, auth: Auth, Json(req): Json<ConnectRequest>) -> ApiResult<(StatusCode, Json<Account>)> {
    auth.require(Scope::Admin)?;
    Ok((StatusCode::CREATED, Json(s.harness.connect(req).await?)))
}

pub async fn update_provider(State(s): State<AppState>, auth: Auth, Path(id): Path<String>, Json(p): Json<AccountPatch>) -> ApiResult<Json<Account>> {
    auth.require(Scope::Admin)?;
    Ok(Json(s.harness.update_account(&id, p).await?))
}

pub async fn delete_provider(State(s): State<AppState>, auth: Auth, Path(id): Path<String>) -> ApiResult<StatusCode> {
    auth.require(Scope::Admin)?;
    s.harness.delete_account(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn verify_provider(State(s): State<AppState>, auth: Auth, Path(id): Path<String>) -> ApiResult<Json<Account>> {
    auth.require(Scope::Admin)?;
    Ok(Json(s.harness.verify_account(&id).await?))
}

pub async fn refresh_provider(State(s): State<AppState>, auth: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    auth.require(Scope::Admin)?;
    let account = s.harness.verify_account(&id).await?;
    let models = if account.status == ConnectionStatus::Connected { s.harness.refresh_models(&id).await.ok() } else { None };
    let limits = if account.status == ConnectionStatus::Connected { s.harness.refresh_limits(&id).await.ok() } else { None };
    Ok(Json(json!({"account": account, "models": models, "limits": limits})))
}

pub async fn detect_clis(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    Ok(Json(json!({"clis": s.harness.detect_clis().await})))
}

pub async fn limits(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    Ok(Json(json!({"accounts": s.harness.account_limits()})))
}

#[derive(Deserialize)]
pub struct PrefBody {
    key: String,
    preference: ModelPreference,
}

pub async fn set_model_preference(State(s): State<AppState>, auth: Auth, Json(b): Json<PrefBody>) -> ApiResult<Json<ModelPreference>> {
    auth.require(Scope::Admin)?;
    Ok(Json(s.harness.set_model_preference(&b.key, b.preference)?))
}

// -------------------------------------------------------------------- usage

#[derive(Debug, Deserialize, Default)]
pub struct RangeQuery {
    range: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    bucket_ms: Option<i64>,
    group_by: Option<GroupBy>,
    provider: Option<String>,
    account_id: Option<String>,
    model_key: Option<String>,
    format: Option<String>,
}

impl RangeQuery {
    fn bounds(&self) -> ApiResult<(i64, i64)> {
        let now = now().timestamp_millis();
        let span = match self.range.as_deref() {
            Some("1h") => 3_600_000,
            Some("24h") | None => 86_400_000,
            Some("7d") => 7 * 86_400_000,
            Some("30d") => 30 * 86_400_000,
            Some("90d") => 90 * 86_400_000,
            Some("custom") => 0,
            Some(other) => return Err(ApiError::bad_request(format!("Unknown range: {other}"))),
        };
        let (from, to) = if self.range.as_deref() == Some("custom") || self.from.is_some() {
            let from = self.from.ok_or_else(|| ApiError::bad_request("`from` is required for custom ranges"))?;
            (from, self.to.unwrap_or(now))
        } else {
            (now - span, now)
        };
        if to <= from {
            return Err(ApiError::bad_request("`to` must be after `from`"));
        }
        Ok((from, to))
    }

    fn filter(&self) -> UsageFilter {
        UsageFilter { provider: self.provider.clone(), account_id: self.account_id.clone(), model_key: self.model_key.clone() }
    }
}

/// Bucket size giving roughly 60-170 points per chart.
fn auto_bucket(span: i64) -> i64 {
    const MIN: i64 = 60_000;
    const HOUR: i64 = 3_600_000;
    match span {
        s if s <= 2 * HOUR => MIN,
        s if s <= 36 * HOUR => 15 * MIN,
        s if s <= 8 * 24 * HOUR => HOUR,
        s if s <= 31 * 24 * HOUR => 6 * HOUR,
        _ => 24 * HOUR,
    }
}

pub async fn usage_summary(State(s): State<AppState>, auth: Auth, Query(q): Query<RangeQuery>) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    let (from, to) = q.bounds()?;
    let filter = q.filter();
    let current = s.harness.store.usage_summary(from, to, &filter).map_err(HarnessError::from)?;
    let span = to - from;
    let previous = s.harness.store.usage_summary(from - span, from, &filter).map_err(HarnessError::from)?;
    Ok(Json(json!({"current": current, "previous": previous, "active": s.harness.active_executions().len()})))
}

pub async fn usage_timeseries(State(s): State<AppState>, auth: Auth, Query(q): Query<RangeQuery>) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    let (from, to) = q.bounds()?;
    let bucket = q.bucket_ms.filter(|b| *b >= 60_000).unwrap_or_else(|| auto_bucket(to - from));
    // Align to bucket boundaries so points line up across requests.
    let aligned_from = from - from.rem_euclid(bucket);
    let group = q.group_by.unwrap_or(GroupBy::None);
    let mut points = s.harness.store.usage_timeseries(aligned_from, to, bucket, group, &q.filter()).map_err(HarnessError::from)?;
    if group == GroupBy::None {
        // Fill empty buckets with zeros for continuous charts.
        let mut filled = Vec::new();
        let mut t = aligned_from;
        let mut iter = points.into_iter().peekable();
        while t < to {
            match iter.peek() {
                Some(p) if p.t == t => filled.push(iter.next().unwrap()),
                _ => filled.push(magpie_store::TimeseriesPoint {
                    t,
                    group: None,
                    requests: 0,
                    failed: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_usd: 0.0,
                    avg_duration_ms: None,
                }),
            }
            t += bucket;
        }
        points = filled;
    }
    Ok(Json(json!({"from": aligned_from, "to": to, "bucket_ms": bucket, "group_by": group, "points": points})))
}

pub async fn usage_breakdown(State(s): State<AppState>, auth: Auth, Query(q): Query<RangeQuery>) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    let (from, to) = q.bounds()?;
    let group = q.group_by.unwrap_or(GroupBy::Provider);
    let rows = s.harness.store.usage_breakdown(from, to, group, &q.filter()).map_err(HarnessError::from)?;
    Ok(Json(json!({"from": from, "to": to, "group_by": group, "rows": rows})))
}

pub async fn provider_reports(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    Ok(Json(json!({"reports": s.harness.provider_usage_reports()})))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Export execution metadata. Never includes request content or secrets.
pub async fn usage_export(State(s): State<AppState>, auth: Auth, Query(q): Query<RangeQuery>) -> ApiResult<Response> {
    auth.require(Scope::Read)?;
    let (from, to) = q.bounds()?;
    let rows = s
        .harness
        .store
        .list_executions(&ExecutionQuery { limit: 100_000, from: Some(from), to: Some(to), ..Default::default() })
        .map_err(HarnessError::from)?;
    let opt = |v: Option<u64>| v.map(|x| x.to_string()).unwrap_or_default();
    if q.format.as_deref() == Some("json") {
        let data: Vec<Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.id, "created_at": r.created_at, "status": r.status, "task": r.task, "client": r.client,
                    "model": r.model, "usage": r.usage, "cost": r.cost, "duration_ms": r.duration_ms,
                    "time_to_first_token_ms": r.time_to_first_token_ms, "attempts": r.attempts.len(),
                    "error_kind": r.error.as_ref().map(|e| e.kind),
                })
            })
            .collect();
        let body = serde_json::to_string_pretty(&data).unwrap_or_default();
        return Ok(([(header::CONTENT_TYPE, "application/json"), (header::CONTENT_DISPOSITION, "attachment; filename=\"magpie-usage.json\"")], body).into_response());
    }
    let mut out = String::from("id,created_at,status,task,client,provider,model,input_tokens,output_tokens,cached_tokens,reasoning_tokens,token_provenance,cost_usd,cost_provenance,api_equivalent,duration_ms,ttft_ms,attempts,error_kind\n");
    for r in &rows {
        let m = r.model.as_ref();
        let line = [
            r.id.clone(),
            r.created_at.to_rfc3339(),
            r.status.as_str().to_string(),
            r.task.as_str().to_string(),
            csv_field(&r.client),
            m.map(|m| m.provider.as_str().to_string()).unwrap_or_default(),
            csv_field(&m.map(|m| m.model_id.clone()).unwrap_or_default()),
            opt(r.usage.input_tokens),
            opt(r.usage.output_tokens),
            opt(r.usage.cached_input_tokens),
            opt(r.usage.reasoning_tokens),
            format!("{:?}", r.usage.provenance).to_lowercase(),
            r.cost.map(|c| format!("{:.6}", c.usd)).unwrap_or_default(),
            r.cost.map(|c| format!("{:?}", c.provenance).to_lowercase()).unwrap_or_default(),
            r.cost.map(|c| c.api_equivalent.to_string()).unwrap_or_default(),
            opt(r.duration_ms),
            opt(r.time_to_first_token_ms),
            r.attempts.len().to_string(),
            r.error.as_ref().map(|e| e.kind.as_str().to_string()).unwrap_or_default(),
        ]
        .join(",");
        out.push_str(&line);
        out.push('\n');
    }
    Ok(([(header::CONTENT_TYPE, "text/csv; charset=utf-8"), (header::CONTENT_DISPOSITION, "attachment; filename=\"magpie-usage.csv\"")], out).into_response())
}

pub async fn clear_history(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Admin)?;
    let n = s.harness.store.clear_history().map_err(HarnessError::from)?;
    Ok(Json(json!({"deleted": n})))
}

// --------------------------------------------------------------- executions

pub async fn list_executions(State(s): State<AppState>, auth: Auth, Query(mut q): Query<ExecutionQuery>) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    if q.limit == 0 {
        q.limit = 200;
    }
    let total = s.harness.store.count_executions(&q).map_err(HarnessError::from)?;
    let data = s.harness.store.list_executions(&q).map_err(HarnessError::from)?;
    Ok(Json(json!({"data": data, "total": total})))
}

pub async fn active_executions(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    Ok(Json(json!({"data": s.harness.active_executions()})))
}

pub async fn get_execution(State(s): State<AppState>, auth: Auth, Path(id): Path<String>) -> ApiResult<Json<ExecutionRecord>> {
    auth.require(Scope::Read)?;
    let mut rec = s.harness.store.get_execution(&id).map_err(HarnessError::from)?.ok_or_else(|| ApiError::not_found("Execution"))?;
    if !auth.0.has(Scope::Admin) {
        // Stored content is only visible to the owner of the harness.
        rec.request_content = None;
        rec.response_content = None;
    }
    Ok(Json(rec))
}

pub async fn cancel_execution(State(s): State<AppState>, auth: Auth, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    auth.require(Scope::Execute)?;
    if !s.harness.cancel(&id) {
        return Err(ApiError::not_found("Active execution"));
    }
    Ok(Json(json!({"cancelled": true})))
}

pub async fn events(State(s): State<AppState>, auth: Auth) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    auth.require(Scope::Read)?;
    let rx = s.harness.subscribe();
    let stop = s.stop.clone();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|r| async move { r.ok() })
        .map(|ev| {
            let v = serde_json::to_value(&ev).unwrap_or_default();
            let name = v["type"].as_str().unwrap_or("event").to_string();
            Ok(Event::default().event(name).data(v.to_string()))
        })
        .take_until(async move { stop.cancelled().await });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

// ------------------------------------------------------------ configuration

pub async fn get_settings(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    Ok(Json(json!({"settings": s.harness.settings(), "autostart_enabled": s.harness.autostart_enabled()})))
}

pub async fn put_settings(State(s): State<AppState>, auth: Auth, Json(new): Json<Settings>) -> ApiResult<Json<Value>> {
    auth.require(Scope::Admin)?;
    let saved = s.harness.update_settings(new)?;
    Ok(Json(json!({"settings": saved, "autostart_enabled": s.harness.autostart_enabled()})))
}

pub async fn get_routing(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<RoutingConfig>> {
    auth.require(Scope::Read)?;
    Ok(Json(s.harness.routing_config()))
}

pub async fn put_routing(State(s): State<AppState>, auth: Auth, Json(cfg): Json<RoutingConfig>) -> ApiResult<Json<RoutingConfig>> {
    auth.require(Scope::Admin)?;
    Ok(Json(s.harness.update_routing(cfg)?))
}

pub async fn list_keys(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Admin)?;
    Ok(Json(json!({"keys": s.harness.list_clients()?})))
}

#[derive(Deserialize)]
pub struct CreateKey {
    name: String,
    #[serde(default = "default_scopes")]
    scopes: Vec<String>,
}

fn default_scopes() -> Vec<String> {
    vec!["execute".into(), "read".into()]
}

pub async fn create_key(State(s): State<AppState>, auth: Auth, Json(b): Json<CreateKey>) -> ApiResult<(StatusCode, Json<Value>)> {
    auth.require(Scope::Admin)?;
    let created = s.harness.create_client(&b.name, &b.scopes)?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(created).unwrap_or_default())))
}

pub async fn revoke_key(State(s): State<AppState>, auth: Auth, Path(id): Path<String>) -> ApiResult<StatusCode> {
    auth.require(Scope::Admin)?;
    if s.harness.revoke_client(&id)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("Key"))
    }
}

pub async fn list_notifications(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    Ok(Json(json!({"notifications": s.harness.notifications(100)?})))
}

pub async fn read_notifications(State(s): State<AppState>, auth: Auth) -> ApiResult<StatusCode> {
    auth.require(Scope::Read)?;
    s.harness.mark_notifications_read()?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn shutdown(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Admin)?;
    let stop = s.stop.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        stop.cancel();
    });
    Ok(Json(json!({"stopping": true})))
}
