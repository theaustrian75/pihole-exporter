use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::Mutex;

const MAX_RESPONSE_SIZE: usize = 1024 * 1024;
const AUTH_RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("connection to Pi-hole failed at {url}: {message}")]
    Connection { url: String, message: String },
    #[error("request to Pi-hole timed out at {url}")]
    Timeout { url: String },
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

fn map_request_error(url: String, source: reqwest::Error) -> ApiError {
    if source.is_timeout() {
        ApiError::Timeout { url }
    } else if source.is_connect() {
        ApiError::Connection {
            url,
            message: source.to_string(),
        }
    } else {
        ApiError::Fetch { url, source }
    }
}

fn map_auth_error(url: String, source: reqwest::Error) -> ApiError {
    if source.is_timeout() {
        ApiError::Timeout { url }
    } else if source.is_connect() {
        ApiError::Connection {
            url,
            message: source.to_string(),
        }
    } else {
        ApiError::AuthRequest { url, source }
    }
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
    auth_mutex: Arc<Mutex<()>>,
}

impl ApiClient {
    pub fn new(
        base_url: String,
        password: String,
        timeout: Duration,
        skip_tls_verification: bool,
    ) -> Self {
        let mut builder = Client::builder().timeout(timeout);

        if skip_tls_verification {
            builder = builder.danger_accept_invalid_certs(true);
        }

        Self {
            base_url,
            password,
            client: builder.build().expect("failed to build HTTP client"),
            session: Arc::new(Mutex::new(None)),
            auth_mutex: Arc::new(Mutex::new(())),
        }
    }

    pub async fn ensure_session(&self) -> Result<(), ApiError> {
        self.ensure_auth().await
    }

    async fn ensure_auth(&self) -> Result<(), ApiError> {
        if self.session_valid().await {
            return Ok(());
        }

        let _auth_guard = self.auth_mutex.lock().await;

        if self.session_valid().await {
            return Ok(());
        }

        tracing::debug!("Session expired, re-authenticating");
        self.perform_authenticate().await
    }

    async fn session_valid(&self) -> bool {
        let session = self.session.lock().await;
        session
            .as_ref()
            .is_some_and(|s| Instant::now() < s.validity)
    }

    async fn clear_session(&self) {
        let mut session = self.session.lock().await;
        *session = None;
    }

    async fn perform_authenticate(&self) -> Result<(), ApiError> {
        let url = format!("{}/api/auth", self.base_url);
        tracing::debug!("Authenticating to {}", self.base_url);

        let mut last_status = None;
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(AUTH_RETRY_DELAY * attempt as u32).await;
            }

            let response = self
                .client
                .post(&url)
                .json(&serde_json::json!({ "password": self.password }))
                .send()
                .await
                .map_err(|source| map_auth_error(url.clone(), source))?;

            let status = response.status();
            if status.as_u16() == 429 {
                last_status = Some(429);
                tracing::warn!(
                    attempt = attempt + 1,
                    "Pi-hole authentication rate limited, retrying"
                );
                continue;
            }

            if !status.is_success() {
                return Err(ApiError::AuthStatus(status.as_u16()));
            }

            let body = response
                .bytes()
                .await
                .map_err(|source| map_auth_error(url.clone(), source))?;
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
            return Ok(());
        }

        Err(ApiError::AuthStatus(last_status.unwrap_or(429)))
    }

    pub async fn fetch_data<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, ApiError> {
        self.fetch_data_with_retry(endpoint, true).await
    }

    async fn fetch_data_with_retry<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        may_retry_auth: bool,
    ) -> Result<T, ApiError> {
        self.ensure_auth().await?;

        match self.fetch_data_once::<T>(endpoint).await {
            Ok(value) => Ok(value),
            Err(ApiError::BadStatus(401)) if may_retry_auth => {
                tracing::debug!(endpoint, "Received 401, clearing session and retrying once");
                self.clear_session().await;
                self.ensure_auth().await?;
                self.fetch_data_once(endpoint).await
            }
            Err(err) => Err(err),
        }
    }

    async fn fetch_data_once<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, ApiError> {
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
            .map_err(|source| map_request_error(url.clone(), source))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::BadStatus(status.as_u16()));
        }

        let body = response
            .bytes()
            .await
            .map_err(|source| map_request_error(url.clone(), source))?;
        if body.len() > MAX_RESPONSE_SIZE {
            return Err(ApiError::BadStatus(413));
        }

        let parsed = serde_json::from_slice(&body)?;
        tracing::debug!("Successfully fetched data from endpoint: {}", endpoint);
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_valid_is_false_without_auth() {
        let client = ApiClient::new(
            "http://127.0.0.1".to_string(),
            String::new(),
            Duration::from_secs(1),
            false,
        );
        assert!(!client.session_valid().await);
    }
}
