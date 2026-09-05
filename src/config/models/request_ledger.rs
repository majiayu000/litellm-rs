//! Metadata-only request ledger persistence configuration.

use serde::{Deserialize, Serialize};

/// Default retention window for terminal request-ledger rows.
pub const DEFAULT_REQUEST_LEDGER_RETENTION_DAYS: u32 = 30;
/// Inclusive upper bound for configured ledger retention.
pub const MAX_REQUEST_LEDGER_RETENTION_DAYS: u32 = 366;

/// How a request-ledger write failure is handled.
///
/// `continue` logs the error and keeps serving the request. `fail` surfaces the
/// error on unary responses that have not yet committed a body. Streaming
/// responses are already committed, so `fail` is still logged and never silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RequestLedgerWriteFailure {
    /// Reject unary requests when persistence fails.
    Fail,
    /// Log persistence failures and continue serving the request.
    #[default]
    Continue,
}

/// Persistence policy for one metadata-only terminal row per gateway request.
///
/// Owned by `storage.request_ledger`. The table never stores prompt, response
/// body, Authorization, raw API keys, or provider secrets. Rows older than
/// `retention_days` are deleted on subsequent writes so retention stays bounded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RequestLedgerConfig {
    /// Persist terminal request metadata. Disabled by default.
    pub enabled: bool,
    /// Fail or continue when a ledger write fails.
    pub write_failure: RequestLedgerWriteFailure,
    /// Delete rows whose `finished_at` is older than this many days (1–366).
    pub retention_days: u32,
}

impl Default for RequestLedgerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            write_failure: RequestLedgerWriteFailure::Continue,
            retention_days: DEFAULT_REQUEST_LEDGER_RETENTION_DAYS,
        }
    }
}

impl RequestLedgerConfig {
    /// Merge layered storage overlays. A non-default overlay replaces the base.
    pub fn merge(self, other: Self) -> Self {
        if other == Self::default() {
            self
        } else {
            other
        }
    }

    /// Reject unbounded or zero retention.
    pub fn validate(&self) -> Result<(), String> {
        if self.retention_days == 0 || self.retention_days > MAX_REQUEST_LEDGER_RETENTION_DAYS {
            return Err(format!(
                "storage.request_ledger.retention_days must be between 1 and {MAX_REQUEST_LEDGER_RETENTION_DAYS}"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_disabled_continue_with_bounded_retention() {
        let config = RequestLedgerConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.write_failure, RequestLedgerWriteFailure::Continue);
        assert_eq!(config.retention_days, DEFAULT_REQUEST_LEDGER_RETENTION_DAYS);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn deserializes_fail_policy() {
        let config: RequestLedgerConfig =
            serde_yml::from_str("enabled: true\nwrite_failure: fail\nretention_days: 7\n")
                .expect("valid request ledger yaml");
        assert!(config.enabled);
        assert_eq!(config.write_failure, RequestLedgerWriteFailure::Fail);
        assert_eq!(config.retention_days, 7);
    }

    #[test]
    fn rejects_unbounded_retention() {
        let config = RequestLedgerConfig {
            retention_days: 0,
            ..RequestLedgerConfig::default()
        };
        assert!(config.validate().is_err());
    }
}
