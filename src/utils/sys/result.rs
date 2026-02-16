//! Result extension utilities for better error handling
//!
//! This module provides extension traits and utilities to make error handling
//! more ergonomic and reduce the need for unwrap() calls.

#![allow(dead_code)] // Tool module - functions may be used in the future

use crate::utils::error::gateway_error::{GatewayError, Result};
use tracing::{error, warn};

/// Extension trait for Result types to provide better error handling
pub trait ResultExt<T> {
    /// Log an error and return a default value instead of panicking
    fn unwrap_or_log_default(self, context: &str) -> T
    where
        T: Default;

    /// Log an error and return the provided default value
    fn unwrap_or_log(self, default: T, context: &str) -> T;

    /// Convert to a GatewayError with additional context
    fn with_context(self, context: &str) -> Result<T>;

    /// Log the error and continue with a default value
    fn log_and_continue(self, context: &str) -> Option<T>;
}

impl<T> ResultExt<T> for Result<T> {
    fn unwrap_or_log_default(self, context: &str) -> T
    where
        T: Default,
    {
        match self {
            Ok(value) => value,
            Err(e) => {
                error!("Error in {}: {}. Using default value.", context, e);
                T::default()
            }
        }
    }

    fn unwrap_or_log(self, default: T, context: &str) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                error!("Error in {}: {}. Using fallback value.", context, e);
                default
            }
        }
    }

    fn with_context(self, context: &str) -> Result<T> {
        self.map_err(|e| GatewayError::Internal(format!("{}: {}", context, e)))
    }

    fn log_and_continue(self, context: &str) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(e) => {
                warn!("Non-fatal error in {}: {}. Continuing...", context, e);
                None
            }
        }
    }
}

/// Extension trait for Option types
pub trait OptionExt<T> {
    /// Convert None to a GatewayError with context
    fn ok_or_context(self, context: &str) -> Result<T>;

    /// Log when None and return default
    fn unwrap_or_log_default(self, context: &str) -> T
    where
        T: Default;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_context(self, context: &str) -> Result<T> {
        self.ok_or_else(|| GatewayError::Internal(format!("Missing required value: {}", context)))
    }

    fn unwrap_or_log_default(self, context: &str) -> T
    where
        T: Default,
    {
        match self {
            Some(value) => value,
            None => {
                warn!("Missing value in {}, using default", context);
                T::default()
            }
        }
    }
}

/// Utility for safe numeric conversions
pub trait SafeConvert<T> {
    /// Safely convert to target type with error context
    fn safe_convert(self, context: &str) -> Result<T>;
}

impl SafeConvert<usize> for u32 {
    fn safe_convert(self, context: &str) -> Result<usize> {
        usize::try_from(self).map_err(|e| {
            GatewayError::Internal(format!("Numeric conversion failed in {}: {}", context, e))
        })
    }
}

impl SafeConvert<u32> for usize {
    fn safe_convert(self, context: &str) -> Result<u32> {
        u32::try_from(self).map_err(|e| {
            GatewayError::Internal(format!("Numeric conversion failed in {}: {}", context, e))
        })
    }
}

/// Macro for safe unwrapping with context
#[macro_export]
macro_rules! safe_unwrap {
    ($expr:expr, $context:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                error!("Error in {}: {}", $context, e);
                return Err($crate::utils::error::gateway_error::GatewayError::Internal(
                    format!("Failed in {}: {}", $context, e),
                ));
            }
        }
    };
}

/// Macro for safe option unwrapping with context
#[macro_export]
macro_rules! safe_unwrap_option {
    ($expr:expr, $context:expr) => {
        match $expr {
            Some(val) => val,
            None => {
                error!("Missing required value in {}", $context);
                return Err($crate::utils::error::gateway_error::GatewayError::Internal(
                    format!("Missing required value in {}", $context),
                ));
            }
        }
    };
}

/// Utility function to create a configuration error
pub fn config_error(msg: &str) -> GatewayError {
    GatewayError::Config(msg.to_string())
}

/// Utility function to create an internal error
pub fn internal_error(msg: &str) -> GatewayError {
    GatewayError::Internal(msg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ResultExt Tests ====================

    #[test]
    fn test_result_ext_unwrap_or_log_default_ok() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap_or_log_default("test"), 42);
    }

    #[test]
    fn test_result_ext_unwrap_or_log_default_err() {
        let err_result: Result<i32> = Err(GatewayError::Internal("test error".to_string()));
        assert_eq!(err_result.unwrap_or_log_default("test"), 0);
    }

    #[test]
    fn test_result_ext_unwrap_or_log_default_string() {
        let ok_result: Result<String> = Ok("hello".to_string());
        assert_eq!(ok_result.unwrap_or_log_default("test"), "hello");

        let err_result: Result<String> = Err(GatewayError::Internal("error".to_string()));
        assert_eq!(err_result.unwrap_or_log_default("test"), "");
    }

    #[test]
    fn test_result_ext_unwrap_or_log_default_vec() {
        let ok_result: Result<Vec<i32>> = Ok(vec![1, 2, 3]);
        assert_eq!(ok_result.unwrap_or_log_default("test"), vec![1, 2, 3]);

        let err_result: Result<Vec<i32>> = Err(GatewayError::Internal("error".to_string()));
        assert!(err_result.unwrap_or_log_default("test").is_empty());
    }

    #[test]
    fn test_result_ext_unwrap_or_log_ok() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap_or_log(99, "test"), 42);
    }

    #[test]
    fn test_result_ext_unwrap_or_log_err() {
        let err_result: Result<i32> = Err(GatewayError::Internal("test error".to_string()));
        assert_eq!(err_result.unwrap_or_log(99, "test"), 99);
    }

    #[test]
    fn test_result_ext_unwrap_or_log_custom_default() {
        let err_result: Result<String> = Err(GatewayError::Internal("error".to_string()));
        assert_eq!(
            err_result.unwrap_or_log("fallback".to_string(), "test"),
            "fallback"
        );
    }

    #[test]
    fn test_result_ext_with_context_ok() {
        let ok_result: Result<i32> = Ok(42);
        let contexted = ok_result.with_context("additional context");
        assert!(contexted.is_ok());
        assert_eq!(contexted.unwrap(), 42);
    }

    #[test]
    fn test_result_ext_with_context_err() {
        let err_result: Result<i32> = Err(GatewayError::Internal("original error".to_string()));
        let contexted = err_result.with_context("context info");
        assert!(contexted.is_err());

        if let Err(GatewayError::Internal(msg)) = contexted {
            assert!(msg.contains("context info"));
            assert!(msg.contains("original error"));
        } else {
            panic!("Expected Internal error");
        }
    }

    #[test]
    fn test_result_ext_log_and_continue_ok() {
        let ok_result: Result<i32> = Ok(42);
        let option = ok_result.log_and_continue("test");
        assert_eq!(option, Some(42));
    }

    #[test]
    fn test_result_ext_log_and_continue_err() {
        let err_result: Result<i32> = Err(GatewayError::Internal("error".to_string()));
        let option = err_result.log_and_continue("test");
        assert_eq!(option, None);
    }

    // ==================== OptionExt Tests ====================

    #[test]
    fn test_option_ext_ok_or_context_some() {
        let some_val = Some(42);
        let result = some_val.ok_or_context("missing value");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_option_ext_ok_or_context_none() {
        let none_val: Option<i32> = None;
        let result = none_val.ok_or_context("missing value");
        assert!(result.is_err());

        if let Err(GatewayError::Internal(msg)) = result {
            assert!(msg.contains("missing value"));
        } else {
            panic!("Expected Internal error");
        }
    }

    #[test]
    fn test_option_ext_unwrap_or_log_default_some() {
        let some_val = Some(42);
        assert_eq!(some_val.unwrap_or_log_default("test"), 42);
    }

    #[test]
    fn test_option_ext_unwrap_or_log_default_none() {
        let none_val: Option<i32> = None;
        assert_eq!(none_val.unwrap_or_log_default("test"), 0);
    }

    #[test]
    fn test_option_ext_unwrap_or_log_default_string() {
        let some_val = Some("hello".to_string());
        assert_eq!(some_val.unwrap_or_log_default("test"), "hello");

        let none_val: Option<String> = None;
        assert_eq!(none_val.unwrap_or_log_default("test"), "");
    }

    #[test]
    fn test_option_ext_unwrap_or_log_default_vec() {
        let none_val: Option<Vec<u8>> = None;
        assert!(none_val.unwrap_or_log_default("test").is_empty());
    }

    // ==================== SafeConvert Tests ====================

    #[test]
    fn test_safe_convert_u32_to_usize() {
        let val: u32 = 42;
        let converted: Result<usize> = val.safe_convert("test conversion");
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap(), 42);
    }

    #[test]
    fn test_safe_convert_u32_to_usize_zero() {
        let val: u32 = 0;
        let converted: Result<usize> = val.safe_convert("test");
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap(), 0);
    }

    #[test]
    fn test_safe_convert_u32_to_usize_max() {
        let val: u32 = u32::MAX;
        let converted: Result<usize> = val.safe_convert("test");
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap(), u32::MAX as usize);
    }

    #[test]
    fn test_safe_convert_usize_to_u32() {
        let val: usize = 42;
        let converted: Result<u32> = val.safe_convert("test conversion");
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap(), 42);
    }

    #[test]
    fn test_safe_convert_usize_to_u32_zero() {
        let val: usize = 0;
        let converted: Result<u32> = val.safe_convert("test");
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap(), 0);
    }

    #[test]
    fn test_safe_convert_usize_to_u32_boundary() {
        let val: usize = u32::MAX as usize;
        let converted: Result<u32> = val.safe_convert("test");
        assert!(converted.is_ok());
        assert_eq!(converted.unwrap(), u32::MAX);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_safe_convert_usize_to_u32_overflow() {
        let val: usize = u32::MAX as usize + 1;
        let converted: Result<u32> = val.safe_convert("overflow test");
        assert!(converted.is_err());

        if let Err(GatewayError::Internal(msg)) = converted {
            assert!(msg.contains("overflow test"));
            assert!(msg.contains("conversion failed"));
        } else {
            panic!("Expected Internal error");
        }
    }

    // ==================== Error Helper Functions Tests ====================

    #[test]
    fn test_config_error() {
        let err = config_error("Invalid configuration");
        if let GatewayError::Config(msg) = err {
            assert_eq!(msg, "Invalid configuration");
        } else {
            panic!("Expected Config error");
        }
    }

    #[test]
    fn test_config_error_empty() {
        let err = config_error("");
        if let GatewayError::Config(msg) = err {
            assert_eq!(msg, "");
        } else {
            panic!("Expected Config error");
        }
    }

    #[test]
    fn test_config_error_special_chars() {
        let err = config_error("Error with special chars: \"quotes\", 'apostrophes', <tags>");
        if let GatewayError::Config(msg) = err {
            assert!(msg.contains("quotes"));
            assert!(msg.contains("apostrophes"));
        } else {
            panic!("Expected Config error");
        }
    }

    #[test]
    fn test_internal_error() {
        let err = internal_error("Something went wrong");
        if let GatewayError::Internal(msg) = err {
            assert_eq!(msg, "Something went wrong");
        } else {
            panic!("Expected Internal error");
        }
    }

    #[test]
    fn test_internal_error_empty() {
        let err = internal_error("");
        if let GatewayError::Internal(msg) = err {
            assert_eq!(msg, "");
        } else {
            panic!("Expected Internal error");
        }
    }

    #[test]
    fn test_internal_error_long_message() {
        let long_msg = "x".repeat(1000);
        let err = internal_error(&long_msg);
        if let GatewayError::Internal(msg) = err {
            assert_eq!(msg.len(), 1000);
        } else {
            panic!("Expected Internal error");
        }
    }

    // ==================== Combined Usage Tests ====================

    #[test]
    fn test_result_chain_with_context() {
        fn maybe_fail(should_fail: bool) -> Result<i32> {
            if should_fail {
                Err(GatewayError::Internal("operation failed".to_string()))
            } else {
                Ok(42)
            }
        }

        let success = maybe_fail(false).with_context("in test function");
        assert!(success.is_ok());

        let failure = maybe_fail(true).with_context("in test function");
        assert!(failure.is_err());
    }

    #[test]
    fn test_option_to_result_chain() {
        fn get_value(has_value: bool) -> Option<i32> {
            if has_value { Some(100) } else { None }
        }

        let result = get_value(true).ok_or_context("value not found");
        assert!(result.is_ok());

        let result = get_value(false).ok_or_context("value not found");
        assert!(result.is_err());
    }

    #[test]
    fn test_unwrap_patterns() {
        // Pattern 1: Default for errors
        let val: Result<i32> = Err(GatewayError::Internal("err".to_string()));
        let defaulted = val.unwrap_or_log_default("pattern1");
        assert_eq!(defaulted, 0);

        // Pattern 2: Custom fallback
        let val: Result<i32> = Err(GatewayError::Internal("err".to_string()));
        let fallback = val.unwrap_or_log(-1, "pattern2");
        assert_eq!(fallback, -1);

        // Pattern 3: Continue with None
        let val: Result<i32> = Err(GatewayError::Internal("err".to_string()));
        let continued = val.log_and_continue("pattern3");
        assert!(continued.is_none());
    }

    #[test]
    fn test_various_error_types() {
        // Test with different GatewayError variants
        let config_err: Result<i32> = Err(GatewayError::Config("config issue".to_string()));
        assert_eq!(config_err.unwrap_or_log_default("test"), 0);

        let internal_err: Result<i32> = Err(GatewayError::Internal("internal issue".to_string()));
        assert_eq!(internal_err.unwrap_or_log(42, "test"), 42);
    }
}
