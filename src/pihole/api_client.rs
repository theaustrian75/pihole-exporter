use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::Mutex;

const MAX_RESPONSE_SIZE: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("authentication request failed for {url}: {source}")]
    AuthRequest {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("authentication failed, status code: {0}")]
    AuthStatus(u16),
    #[error("authentication unsuccessful")]
    AuthUnsuccessful,
    #[error("failed to fetch data from {url}: {source}")]
    Fetch {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("non-200 status code: {0}")]
    BadStatus(u16),
    #[error("failed to parse JSON response: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    session: AuthSession,
}

#[derive(Debug, Deserialize)]
struct AuthSession {
    valid: bool,
    sid: String,
    validity: u64,
}

struct SessionState {
    session_id: String,
    validity: Instant,
}

pub struct ApiClient {
    base_url: String,
    password: String,
    client: Client,
    session: Arc<Mutex<Option<SessionState>>>,
}

impl ApiClient {
    pub fn new(base_url: String, password: String, timeout: Duration, skip_tls_verification: bool) -> Self {
        let mut builder = Client::builder().timeout(timeout);

        if skip_tls_verification {
            builder = builder.danger_accept_invalid_certs(true);
        }

        Self {
            base_url,
            password,
            client: builder.build().expect("failed to build HTTP client"),
            session: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn authenticate(&self) -> Result<(), ApiError> {
        let url = format!("{}/api/auth", self.base_url);
        tracing::debug!("Authenticating to {}", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "password": self.password }))
            .send()
            .await
            .map_err(|source| ApiError::AuthRequest {
                url: url.clone(),
                source,
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::AuthStatus(status.as_u16()));
        }

        let body = response.bytes().await.map_err(|source| ApiError::AuthRequest {
            url: url.clone(),
            source,
        })?;
        if body.len() > MAX_RESPONSE_SIZE {
            return Err(ApiError::AuthStatus(413));
        }

        let auth: AuthResponse = serde_json::from_slice(&body)?;
        if !auth.session.valid {
            return Err(ApiError::AuthUnsuccessful);
        }

        let mut session = self.session.lock().await;
        *session = Some(SessionState {
            session_id: auth.session.sid,
            validity: Instant::now() + Duration::from_secs(auth.session.validity),
        });

        tracing::debug!("Authentication successful");
        Ok(())
    }

    async fn ensure_auth(&self) -> Result<(), ApiError> {
        let needs_auth = {
            let session = self.session.lock().await;
            session
                .as_ref()
                .map(|s| Instant::now() >= s.validity)
                .unwrap_or(true)
        };

        if needs_auth {
            tracing::debug!("Session expired, re-authenticating");
            self.authenticate().await?;
        }

        Ok(())
    }

    pub async fn fetch_data<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, ApiError> {
        self.ensure_auth().await?;

        let url = format!("{}{}", self.base_url, endpoint);
        tracing::debug!("Fetching data from {}", url);

        let session_id = {
            let session = self.session.lock().await;
            session
                .as_ref()
                .map(|s| s.session_id.clone())
                .unwrap_or_default()
        };

        let response = self
            .client
            .get(&url)
            .header("X-FTL-SID", session_id)
            .header("X-Content-Type-Options", "nosniff")
            .send()
            .await
            .map_err(|source| ApiError::Fetch {
                url: url.clone(),
                source,
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::BadStatus(status.as_u16()));
        }

        let body = response.bytes().await.map_err(|source| ApiError::Fetch {
            url: url.clone(),
            source,
        })?;
        if body.len() > MAX_RESPONSE_SIZE {
            return Err(ApiError::BadStatus(413));
        }

        let parsed = serde_json::from_slice(&body)?;
        tracing::debug!("Successfully fetched data from endpoint: {}", endpoint);
        Ok(parsed)
    }
}
