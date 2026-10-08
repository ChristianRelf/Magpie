//! Shared HTTP utilities: client construction, server-sent event decoding,
//! error classification and rate-limit header parsing.

use std::time::Duration;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use magpie_core::*;
use reqwest::header::HeaderMap;

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("{}/{}", PRODUCT_NAME, VERSION))
        // Custom authentication headers (e.g. x-api-key) must never follow
        // an endpoint redirect to another service.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .expect("http client")
}

/// Map a reqwest transport error.
pub fn transport_error(e: reqwest::Error) -> HarnessError {
    let msg = magpie_security::redact(&e.to_string());
    if e.is_timeout() {
        HarnessError::new(ErrorKind::Timeout, msg)
    } else if e.is_connect() {
        HarnessError::new(ErrorKind::Network, format!("Could not connect: {msg}"))
    } else {
        HarnessError::new(ErrorKind::Network, msg)
    }
}

/// Build a classified error from a non-success HTTP response.
pub async fn error_from_response(resp: reqwest::Response) -> HarnessError {
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    classify_error_body(status, &headers, &body)
}

pub fn classify_error_body(status: u16, headers: &HeaderMap, body: &str) -> HarnessError {
    let json: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let err_obj = json.as_ref().and_then(|j| j.get("error").cloned().or_else(|| Some(j.clone())));
    let message = err_obj
        .as_ref()
        .and_then(|e| e.get("message").and_then(|m| m.as_str()).map(str::to_string))
        .or_else(|| err_obj.as_ref().and_then(|e| e.as_str().map(str::to_string)))
        .unwrap_or_else(|| {
            let t = body.trim();
            if t.is_empty() {
                format!("HTTP {status}")
            } else {
                t.chars().take(500).collect()
            }
        });
    let code = err_obj
        .as_ref()
        .and_then(|e| {
            e.get("code")
                .or_else(|| e.get("type"))
                .or_else(|| e.get("status"))
                .map(|c| c.as_str().map(str::to_string).unwrap_or_else(|| c.to_string()))
        })
        .unwrap_or_default()
        .to_ascii_lowercase();
    let lower = message.to_ascii_lowercase();

    let mut err = HarnessError::from_http_status(status, magpie_security::redact(&message));
    if code.contains("insufficient_quota")
        || code.contains("billing")
        || lower.contains("credit balance")
        || lower.contains("insufficient credits")
        || lower.contains("exceeded your current quota")
        || (status == 429 && lower.contains("quota") && (lower.contains("per day") || lower.contains("daily")))
    {
        err.kind = ErrorKind::QuotaExhausted;
    } else if code.contains("context_length")
        || lower.contains("context length")
        || lower.contains("context window")
        || lower.contains("too many tokens")
        || lower.contains("prompt is too long")
    {
        err.kind = ErrorKind::ContextLength;
    } else if code.contains("overloaded") || status == 529 {
        err.kind = ErrorKind::ProviderUnavailable;
    } else if status == 404 && !(lower.contains("model") || code.contains("model")) {
        // A 404 on an endpoint (wrong base URL) is not a model problem.
        err.kind = ErrorKind::InvalidRequest;
    } else if code.contains("rate_limit") || code.contains("resource_exhausted") {
        err.kind = ErrorKind::RateLimited;
    }
    err.retry_after_secs = retry_after(headers).or_else(|| google_retry_delay(json.as_ref()));
    err
}

pub fn retry_after(headers: &HeaderMap) -> Option<u64> {
    let v = headers.get("retry-after")?.to_str().ok()?;
    if let Ok(secs) = v.trim().parse::<f64>() {
        return Some(secs.ceil().max(0.0) as u64);
    }
    DateTime::parse_from_rfc2822(v).ok().map(|d| (d.with_timezone(&Utc) - Utc::now()).num_seconds().max(0) as u64)
}

fn google_retry_delay(json: Option<&serde_json::Value>) -> Option<u64> {
    let details = json?.get("error")?.get("details")?.as_array()?;
    details.iter().find_map(|d| {
        let delay = d.get("retryDelay")?.as_str()?;
        delay.trim_end_matches('s').parse::<f64>().ok().map(|s| s.ceil() as u64)
    })
}

/// Parse Go-style durations used by OpenAI/Groq reset headers
/// (`1s`, `6m0s`, `59.6ms`, `2h1m3.5s`, `7.66s`).
pub fn parse_go_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut total = 0f64;
    let mut num = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut matched = false;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            i += 1;
            continue;
        }
        let mut unit = String::new();
        while i < chars.len() && chars[i].is_ascii_alphabetic() {
            unit.push(chars[i]);
            i += 1;
        }
        let n: f64 = num.parse().ok()?;
        num.clear();
        total += match unit.as_str() {
            "h" => n * 3600.0,
            "m" => n * 60.0,
            "s" => n,
            "ms" => n / 1000.0,
            "us" | "µs" => n / 1_000_000.0,
            "ns" => n / 1_000_000_000.0,
            _ => return None,
        };
        matched = true;
    }
    if !num.is_empty() {
        // Bare number: seconds.
        total += num.parse::<f64>().ok()?;
        matched = true;
    }
    matched.then(|| Duration::from_secs_f64(total))
}

fn header_f64(h: &HeaderMap, name: &str) -> Option<f64> {
    h.get(name)?.to_str().ok()?.trim().parse().ok()
}

fn header_str<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name)?.to_str().ok()
}

/// Parse OpenAI-style `x-ratelimit-*` headers (OpenAI, Groq, OpenRouter,
/// and many compatible servers).
pub fn openai_rate_limits(account_id: &str, h: &HeaderMap) -> Vec<LimitWindow> {
    let now = Utc::now();
    let mut out = Vec::new();
    for (suffix, metric, label) in [
        ("requests", LimitMetric::Requests, "Requests"),
        ("tokens", LimitMetric::Tokens, "Tokens"),
        ("requests-day", LimitMetric::Requests, "Requests per day"),
        ("tokens-day", LimitMetric::Tokens, "Tokens per day"),
    ] {
        let limit = header_f64(h, &format!("x-ratelimit-limit-{suffix}"));
        let remaining = header_f64(h, &format!("x-ratelimit-remaining-{suffix}"));
        if limit.is_none() && remaining.is_none() {
            continue;
        }
        let reset = header_str(h, &format!("x-ratelimit-reset-{suffix}"))
            .and_then(|v| {
                parse_go_duration(v).or_else(|| {
                    // Some servers send a unix timestamp in ms or s.
                    v.parse::<f64>().ok().map(|n| {
                        let secs = if n > 1e12 { n / 1000.0 } else { n };
                        Duration::from_secs_f64((secs - now.timestamp() as f64).max(0.0))
                    })
                })
            })
            .map(|d| now + chrono::Duration::milliseconds(d.as_millis() as i64));
        out.push(LimitWindow {
            account_id: account_id.to_string(),
            key: format!("rl.{suffix}"),
            label: label.to_string(),
            metric,
            model_scope: None,
            limit,
            remaining,
            used: None,
            used_percent: None,
            window_secs: if suffix.ends_with("day") { Some(86_400) } else { None },
            resets_at: reset,
            provenance: Provenance::Reported,
            exhausted: false,
            approaching: false,
            observed_at: now,
        });
    }
    out
}

/// Parse Anthropic `anthropic-ratelimit-*` headers.
pub fn anthropic_rate_limits(account_id: &str, model: &str, h: &HeaderMap) -> Vec<LimitWindow> {
    let now = Utc::now();
    let mut out = Vec::new();
    for (dim, metric, label) in [
        ("requests", LimitMetric::Requests, "Requests per minute"),
        ("tokens", LimitMetric::Tokens, "Tokens per minute"),
        ("input-tokens", LimitMetric::InputTokens, "Input tokens per minute"),
        ("output-tokens", LimitMetric::OutputTokens, "Output tokens per minute"),
    ] {
        let limit = header_f64(h, &format!("anthropic-ratelimit-{dim}-limit"));
        let remaining = header_f64(h, &format!("anthropic-ratelimit-{dim}-remaining"));
        if limit.is_none() && remaining.is_none() {
            continue;
        }
        let reset = header_str(h, &format!("anthropic-ratelimit-{dim}-reset"))
            .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
            .map(|d| d.with_timezone(&Utc));
        out.push(LimitWindow {
            account_id: account_id.to_string(),
            key: format!("rl.{dim}"),
            label: label.to_string(),
            metric,
            model_scope: Some(model.to_string()),
            limit,
            remaining,
            used: None,
            used_percent: None,
            window_secs: Some(60),
            resets_at: reset,
            provenance: Provenance::Reported,
            exhausted: false,
            approaching: false,
            observed_at: now,
        });
    }
    out
}

/// One server-sent event.
#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Incremental SSE decoder over a byte stream.
#[derive(Default)]
pub struct SseDecoder {
    buf: String,
}

impl SseDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buf.push_str(&String::from_utf8_lossy(chunk));
        if self.buf.contains('\r') {
            self.buf = self.buf.replace("\r\n", "\n").replace('\r', "\n");
        }
        let mut out = Vec::new();
        while let Some(pos) = self.buf.find("\n\n") {
            let block: String = self.buf.drain(..pos + 2).collect();
            if let Some(ev) = parse_block(&block) {
                out.push(ev);
            }
        }
        out
    }

    pub fn finish(&mut self) -> Option<SseEvent> {
        let block = std::mem::take(&mut self.buf);
        parse_block(&block)
    }
}

fn parse_block(block: &str) -> Option<SseEvent> {
    let mut event = None;
    let mut data = Vec::new();
    for line in block.lines() {
        if line.starts_with(':') || line.is_empty() {
            continue;
        }
        let (field, value) = match line.find(':') {
            Some(i) => (&line[..i], line[i + 1..].strip_prefix(' ').unwrap_or(&line[i + 1..])),
            None => (line, ""),
        };
        match field {
            "event" => event = Some(value.to_string()),
            "data" => data.push(value.to_string()),
            _ => {}
        }
    }
    if data.is_empty() && event.is_none() {
        return None;
    }
    Some(SseEvent { event, data: data.join("\n") })
}

/// Turn a response body into a stream of SSE events, honouring cancellation
/// and an idle timeout between chunks.
pub fn sse_stream(resp: reqwest::Response, idle_timeout: Duration) -> impl Stream<Item = HarnessResult<SseEvent>> {
    let bytes = resp.bytes_stream();
    futures::stream::unfold(
        (Box::pin(bytes), SseDecoder::default(), std::collections::VecDeque::new(), false),
        move |(mut bytes, mut dec, mut pending, done)| async move {
            loop {
                if let Some(ev) = pending.pop_front() {
                    return Some((Ok(ev), (bytes, dec, pending, done)));
                }
                if done {
                    return None;
                }
                let next: Option<Result<Bytes, reqwest::Error>> = match tokio::time::timeout(idle_timeout, bytes.next()).await {
                    Ok(n) => n,
                    Err(_) => {
                        return Some((Err(HarnessError::new(ErrorKind::Timeout, "Provider stream stalled")), (bytes, dec, pending, true)))
                    }
                };
                match next {
                    Some(Ok(chunk)) => pending.extend(dec.push(&chunk)),
                    Some(Err(e)) => return Some((Err(transport_error(e)), (bytes, dec, pending, true))),
                    None => {
                        if let Some(ev) = dec.finish() {
                            pending.push_back(ev);
                        }
                        return pending.pop_front().map(|ev| (Ok(ev), (bytes, dec, pending, true)));
                    }
                }
            }
        },
    )
}

/// Normalise a base URL (no trailing slash).
pub fn base(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_durations() {
        assert_eq!(parse_go_duration("1s"), Some(Duration::from_secs(1)));
        assert_eq!(parse_go_duration("6m0s"), Some(Duration::from_secs(360)));
        assert_eq!(parse_go_duration("59.6ms").unwrap().as_millis(), 59);
        assert_eq!(parse_go_duration("2h1m3.5s").unwrap().as_secs_f64(), 7263.5);
        assert_eq!(parse_go_duration("12"), Some(Duration::from_secs(12)));
        assert_eq!(parse_go_duration("abc"), None);
    }

    #[test]
    fn sse_decoding_handles_split_chunks() {
        let mut d = SseDecoder::default();
        assert!(d.push(b"event: message_start\ndata: {\"a\"").is_empty());
        let evs = d.push(b":1}\n\ndata: second\r\n\r\n: comment\n\n");
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].event.as_deref(), Some("message_start"));
        assert_eq!(evs[0].data, "{\"a\":1}");
        assert_eq!(evs[1].data, "second");
    }

    #[test]
    fn classifies_quota_and_context_errors() {
        let h = HeaderMap::new();
        let e = classify_error_body(429, &h, r#"{"error":{"message":"You exceeded your current quota","code":"insufficient_quota"}}"#);
        assert_eq!(e.kind, ErrorKind::QuotaExhausted);
        let e = classify_error_body(
            400,
            &h,
            r#"{"error":{"message":"This model's maximum context length is 8192 tokens","code":"context_length_exceeded"}}"#,
        );
        assert_eq!(e.kind, ErrorKind::ContextLength);
        let e = classify_error_body(529, &h, r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#);
        assert_eq!(e.kind, ErrorKind::ProviderUnavailable);
        let e = classify_error_body(401, &h, "");
        assert_eq!(e.kind, ErrorKind::Authentication);
        assert_eq!(e.message, "HTTP 401");
    }

    #[test]
    fn redacts_keys_in_error_messages() {
        let e = classify_error_body(
            401,
            &HeaderMap::new(),
            r#"{"error":{"message":"Incorrect API key provided: sk-proj-abcdefghijklmnopqrstuvwxyz"}}"#,
        );
        assert!(!e.message.contains("abcdefghijklmnop"));
    }

    #[tokio::test]
    async fn authenticated_requests_do_not_follow_redirects() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let location = format!("http://{}/unexpected", target.local_addr().unwrap());
        let url = format!("http://{}/models", origin.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut connection, _) = origin.accept().await.unwrap();
            let mut request = [0; 4096];
            connection.read(&mut request).await.unwrap();
            connection
                .write_all(
                    format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .as_bytes(),
                )
                .await
                .unwrap();
        });
        let response = client().get(url).header("x-api-key", "test-only-sentinel").timeout(Duration::from_secs(2)).send().await.unwrap();
        assert_eq!(response.status().as_u16(), 307);
        assert!(tokio::time::timeout(Duration::from_millis(100), target.accept()).await.is_err());
        server.await.unwrap();
    }

    #[test]
    fn parses_rate_limit_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-ratelimit-limit-requests", "500".parse().unwrap());
        h.insert("x-ratelimit-remaining-requests", "499".parse().unwrap());
        h.insert("x-ratelimit-reset-requests", "120ms".parse().unwrap());
        h.insert("retry-after", "7".parse().unwrap());
        let w = openai_rate_limits("a", &h);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].remaining, Some(499.0));
        assert!(w[0].resets_at.is_some());
        assert_eq!(retry_after(&h), Some(7));

        let mut h = HeaderMap::new();
        h.insert("anthropic-ratelimit-tokens-limit", "80000".parse().unwrap());
        h.insert("anthropic-ratelimit-tokens-remaining", "0".parse().unwrap());
        h.insert("anthropic-ratelimit-tokens-reset", "2030-01-01T00:00:00Z".parse().unwrap());
        let w = anthropic_rate_limits("a", "claude", &h);
        assert_eq!(w[0].state_at(Utc::now(), 0.8), LimitState::Exhausted);
    }
}
