//! Thin HTTP client for the local harness API.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use magpie_runtime::{paths::Paths, Connection};
use serde_json::Value;

pub struct Api {
    pub url: String,
    token: String,
    http: reqwest::Client,
}

impl Api {
    pub fn new(conn: &Connection) -> Self {
        Self {
            url: conn.url.clone(),
            token: conn.token.clone(),
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("http client"),
        }
    }

    pub fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http.request(method, format!("{}{}", self.url, path)).bearer_auth(&self.token)
    }

    async fn handle(resp: reqwest::Response) -> Result<Value> {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let msg = v["error"]["message"].as_str().map(str::to_string).unwrap_or_else(|| format!("HTTP {status}: {text}"));
            bail!("{msg}");
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).context("invalid JSON from harness")
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        Self::handle(self.request(reqwest::Method::GET, path).send().await?).await
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        Self::handle(self.request(reqwest::Method::POST, path).json(body).send().await?).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value> {
        Self::handle(self.request(reqwest::Method::DELETE, path).send().await?).await
    }
}

/// Connect to the harness, starting it on demand unless `no_start`.
pub async fn connect(no_start: bool) -> Result<Api> {
    if let Ok(token) = std::env::var("MAGPIE_API_KEY") {
        if token.trim().is_empty() {
            bail!("MAGPIE_API_KEY is empty");
        }
        let url = std::env::var("MAGPIE_URL").unwrap_or_else(|_| "http://127.0.0.1:7878".into());
        let parsed = reqwest::Url::parse(&url).context("invalid MAGPIE_URL")?;
        if parsed.scheme() != "http" || !matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
            bail!("MAGPIE_URL must be a loopback HTTP endpoint");
        }
        return Ok(Api {
            url: url.trim_end_matches('/').into(),
            token,
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .build()?,
        });
    }
    let paths = Paths::resolve();
    if let Some(c) = magpie_runtime::discover(&paths).await {
        check_version(&c)?;
        return Ok(Api::new(&c));
    }
    if no_start {
        bail!("The harness is not running. Start it with `magpie start`.");
    }
    let exe = std::env::current_exe().context("cannot locate magpie executable")?;
    eprintln!("Starting harness…");
    let c =
        magpie_runtime::ensure_running(&paths, &exe, &["serve".into()], Duration::from_secs(15)).await.map_err(|e| anyhow!(e.message))?;
    check_version(&c)?;
    Ok(Api::new(&c))
}

fn check_version(c: &Connection) -> Result<()> {
    if c.info.api_version != magpie_core::API_VERSION {
        bail!("Running harness speaks API v{} (this CLI expects v{}). Run `magpie restart`.", c.info.api_version, magpie_core::API_VERSION);
    }
    Ok(())
}
