use crate::core::providers::unified_provider::ProviderError;
use std::collections::HashMap;
use std::time::Duration;

use super::types::ErrorUtils;

impl ErrorUtils {
    pub fn extract_retry_after(headers: &HashMap<String, String>) -> Option<Duration> {
        // Check for Retry-After header
        if let Some(retry_after) = headers.get("retry-after")
            && let Ok(seconds) = retry_after.parse::<u64>()
        {
            return Some(Duration::from_secs(seconds));
        }

        // Check for X-RateLimit-Reset header
        if let Some(reset) = headers.get("x-ratelimit-reset")
            && let Ok(timestamp) = reset.parse::<i64>()
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            if timestamp > now {
                return Some(Duration::from_secs((timestamp - now) as u64));
            }
        }

        None
    }

    pub fn should_retry(error: &ProviderError) -> bool {
        matches!(
            error,
            ProviderError::Network { .. }
                | ProviderError::Timeout { .. }
                | ProviderError::ProviderUnavailable { .. }
                | ProviderError::RateLimit { .. }
        )
    }

    pub fn get_retry_delay(error: &ProviderError) -> Duration {
        match error {
            ProviderError::RateLimit { retry_after, .. } => {
                Duration::from_secs(retry_after.unwrap_or(60))
            }
            ProviderError::ProviderUnavailable { .. } => Duration::from_secs(5),
            ProviderError::Network { .. } => Duration::from_secs(1),
            ProviderError::Timeout { .. } => Duration::from_secs(2),
            _ => Duration::from_secs(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::unified_provider::ProviderError;
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn test_extract_retry_after_seconds() {
        let mut headers = HashMap::new();
        headers.insert("retry-after".to_string(), "120".to_string());

        let duration = ErrorUtils::extract_retry_after(&headers);
        assert_eq!(duration, Some(Duration::from_secs(120)));
    }

    #[test]
    fn test_extract_retry_after_invalid_format() {
        let mut headers = HashMap::new();
        headers.insert("retry-after".to_string(), "invalid".to_string());

        let duration = ErrorUtils::extract_retry_after(&headers);
        assert_eq!(duration, None);
    }

    #[test]
    fn test_extract_retry_after_rate_limit_reset_future() {
        let mut headers = HashMap::new();
        let future_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 300; // 5 minutes in the future
        headers.insert(
            "x-ratelimit-reset".to_string(),
            future_timestamp.to_string(),
        );

        let duration = ErrorUtils::extract_retry_after(&headers);
        assert!(duration.is_some());
        let duration = duration.unwrap();
        // Should be approximately 300 seconds (allow some variance for test execution time)
        assert!(duration.as_secs() >= 299 && duration.as_secs() <= 300);
    }

    #[test]
    fn test_extract_retry_after_rate_limit_reset_past() {
        let mut headers = HashMap::new();
        let past_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 100; // 100 seconds in the past
        headers.insert("x-ratelimit-reset".to_string(), past_timestamp.to_string());

        let duration = ErrorUtils::extract_retry_after(&headers);
        assert_eq!(duration, None);
    }

    #[test]
    fn test_extract_retry_after_no_headers() {
        let headers = HashMap::new();
        let duration = ErrorUtils::extract_retry_after(&headers);
        assert_eq!(duration, None);
    }

    #[test]
    fn test_extract_retry_after_priority() {
        let mut headers = HashMap::new();
        headers.insert("retry-after".to_string(), "60".to_string());
        let future_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 120;
        headers.insert(
            "x-ratelimit-reset".to_string(),
            future_timestamp.to_string(),
        );

        // retry-after should take priority
        let duration = ErrorUtils::extract_retry_after(&headers);
        assert_eq!(duration, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_should_retry_retryable_errors() {
        assert!(ErrorUtils::should_retry(&ProviderError::Network {
            provider: "test",
            message: "test".to_string()
        }));
        assert!(ErrorUtils::should_retry(&ProviderError::Timeout {
            provider: "test",
            message: "test".to_string()
        }));
        assert!(ErrorUtils::should_retry(
            &ProviderError::ProviderUnavailable {
                provider: "test",
                message: "test".to_string()
            }
        ));
        assert!(ErrorUtils::should_retry(&ProviderError::RateLimit {
            provider: "test",
            message: "test".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        }));
    }

    #[test]
    fn test_should_retry_non_retryable_errors() {
        assert!(!ErrorUtils::should_retry(&ProviderError::InvalidRequest {
            provider: "test",
            message: "test".to_string()
        }));
        assert!(!ErrorUtils::should_retry(&ProviderError::Authentication {
            provider: "test",
            message: "test".to_string()
        }));
        assert!(!ErrorUtils::should_retry(&ProviderError::ModelNotFound {
            provider: "test",
            model: "test".to_string()
        }));
        assert!(!ErrorUtils::should_retry(&ProviderError::QuotaExceeded {
            provider: "test",
            message: "test".to_string()
        }));
        assert!(!ErrorUtils::should_retry(&ProviderError::Configuration {
            provider: "test",
            message: "test".to_string()
        }));
    }

    #[test]
    fn test_get_retry_delay_rate_limit_with_retry_after() {
        let error = ProviderError::RateLimit {
            provider: "test",
            message: "test".to_string(),
            retry_after: Some(120),
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        };
        assert_eq!(
            ErrorUtils::get_retry_delay(&error),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn test_get_retry_delay_rate_limit_without_retry_after() {
        let error = ProviderError::RateLimit {
            provider: "test",
            message: "test".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        };
        assert_eq!(ErrorUtils::get_retry_delay(&error), Duration::from_secs(60));
    }

    #[test]
    fn test_get_retry_delay_provider_unavailable() {
        let error = ProviderError::ProviderUnavailable {
            provider: "test",
            message: "test".to_string(),
        };
        assert_eq!(ErrorUtils::get_retry_delay(&error), Duration::from_secs(5));
    }

    #[test]
    fn test_get_retry_delay_network() {
        let error = ProviderError::Network {
            provider: "test",
            message: "test".to_string(),
        };
        assert_eq!(ErrorUtils::get_retry_delay(&error), Duration::from_secs(1));
    }

    #[test]
    fn test_get_retry_delay_timeout() {
        let error = ProviderError::Timeout {
            provider: "test",
            message: "test".to_string(),
        };
        assert_eq!(ErrorUtils::get_retry_delay(&error), Duration::from_secs(2));
    }

    #[test]
    fn test_get_retry_delay_default() {
        let error = ProviderError::Other {
            provider: "test",
            message: "test".to_string(),
        };
        assert_eq!(ErrorUtils::get_retry_delay(&error), Duration::from_secs(1));
    }
}
