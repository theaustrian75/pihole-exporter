//! Tracks whether the last Pi-hole metrics scrape succeeded.

#[derive(Debug, Default)]
pub struct UpstreamHealth {
    ready: bool,
    last_error: Option<String>,
}

impl UpstreamHealth {
    pub fn mark_success(&mut self) {
        self.ready = true;
        self.last_error = None;
    }

    pub fn record_failure(&mut self, error: impl Into<String>) {
        self.ready = false;
        self.last_error = Some(error.into());
    }

    /// `Ok(())` when the last metrics scrape succeeded; otherwise the error detail.
    pub fn status(&self) -> Result<(), String> {
        if self.ready {
            Ok(())
        } else {
            Err(self
                .last_error
                .clone()
                .unwrap_or_else(|| "no successful fetch yet".to_string()))
        }
    }
}

pub type SharedUpstreamHealth = std::sync::Arc<std::sync::RwLock<UpstreamHealth>>;

#[cfg(test)]
mod tests {
    use super::UpstreamHealth;

    #[test]
    fn status_unavailable_until_first_success() {
        let health = UpstreamHealth::default();
        assert!(health.status().is_err());
    }

    #[test]
    fn status_ok_after_success_and_err_after_failure() {
        let mut health = UpstreamHealth::default();
        health.mark_success();
        assert!(health.status().is_ok());

        health.record_failure("timeout");
        assert_eq!(health.status().unwrap_err(), "timeout".to_string());
    }
}
