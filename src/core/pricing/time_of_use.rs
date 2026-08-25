use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::Deserialize;

use super::LiteLLMModelInfo;

pub(crate) const TIME_OF_USE_PRICING_KEY: &str = "time_of_use_pricing";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TokenRates {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_read_input_token_cost: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeOfUsePricing {
    timezone: String,
    peak_windows: Vec<WeeklyPeakWindow>,
    peak_rates: TokenRatesConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WeeklyPeakWindow {
    weekdays: Vec<u8>,
    start_hour: u8,
    end_hour: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenRatesConfig {
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: f64,
}

/// Return peak token rates when `at` falls inside a configured UTC window.
///
/// A missing `time_of_use_pricing` entry means the catalog's scalar rates
/// apply. A present entry is validated completely so malformed schedules
/// cannot silently fall back to a cheaper rate.
pub(crate) fn peak_token_rates_at(
    model_info: &LiteLLMModelInfo,
    at: DateTime<Utc>,
) -> Result<Option<TokenRates>, String> {
    let Some(pricing) = parse_time_of_use_pricing(model_info)? else {
        return Ok(None);
    };

    if pricing
        .peak_windows
        .iter()
        .any(|window| window.contains(at))
    {
        Ok(Some(pricing.peak_rates.into()))
    } else {
        Ok(None)
    }
}

/// Validate an optional time-of-use declaration without selecting a rate.
pub(crate) fn validate_time_of_use_pricing(model_info: &LiteLLMModelInfo) -> Result<(), String> {
    parse_time_of_use_pricing(model_info).map(|_| ())
}

/// Return the highest configured rates for conservative budget reservation.
pub(crate) fn configured_peak_token_rates(
    model_info: &LiteLLMModelInfo,
) -> Result<Option<TokenRates>, String> {
    Ok(parse_time_of_use_pricing(model_info)?.map(|pricing| {
        let mut rates: TokenRates = pricing.peak_rates.into();
        let base_input = model_info.input_cost_per_token.unwrap_or(0.0);
        let base_output = model_info.output_cost_per_token.unwrap_or(0.0);
        let base_cache_read = model_info
            .extra
            .get("cache_read_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(base_input);
        rates.input_cost_per_token = rates.input_cost_per_token.max(base_input);
        rates.output_cost_per_token = rates.output_cost_per_token.max(base_output);
        rates.cache_read_input_token_cost = rates.cache_read_input_token_cost.max(base_cache_read);
        rates
    }))
}

fn parse_time_of_use_pricing(
    model_info: &LiteLLMModelInfo,
) -> Result<Option<TimeOfUsePricing>, String> {
    let Some(value) = model_info.extra.get(TIME_OF_USE_PRICING_KEY) else {
        return Ok(None);
    };

    let pricing: TimeOfUsePricing = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid {TIME_OF_USE_PRICING_KEY}: {error}"))?;
    pricing.validate()?;
    Ok(Some(pricing))
}

/// DeepSeek V4's documented peak schedule, used only by the legacy fallback
/// when no catalog row is available.
pub(crate) fn is_deepseek_v4_peak_at(at: DateTime<Utc>) -> bool {
    let weekday = at.weekday().number_from_monday();
    let hour = at.hour();
    weekday <= 5 && ((1..4).contains(&hour) || (6..10).contains(&hour))
}

impl TimeOfUsePricing {
    fn validate(&self) -> Result<(), String> {
        if self.timezone != "UTC" {
            return Err(format!(
                "{TIME_OF_USE_PRICING_KEY}.timezone must be UTC, got {:?}",
                self.timezone
            ));
        }
        if self.peak_windows.is_empty() {
            return Err(format!(
                "{TIME_OF_USE_PRICING_KEY}.peak_windows must not be empty"
            ));
        }
        for (index, window) in self.peak_windows.iter().enumerate() {
            window.validate(index)?;
        }
        self.peak_rates.validate()
    }
}

impl WeeklyPeakWindow {
    fn validate(&self, index: usize) -> Result<(), String> {
        if self.weekdays.is_empty()
            || self
                .weekdays
                .iter()
                .any(|weekday| !(1..=7).contains(weekday))
        {
            return Err(format!(
                "{TIME_OF_USE_PRICING_KEY}.peak_windows[{index}].weekdays must contain ISO weekdays 1 through 7"
            ));
        }
        if self.start_hour >= self.end_hour || self.end_hour > 24 {
            return Err(format!(
                "{TIME_OF_USE_PRICING_KEY}.peak_windows[{index}] must satisfy start_hour < end_hour <= 24"
            ));
        }
        Ok(())
    }

    fn contains(&self, at: DateTime<Utc>) -> bool {
        let weekday = at.weekday().number_from_monday() as u8;
        let hour = at.hour() as u8;
        self.weekdays.contains(&weekday) && hour >= self.start_hour && hour < self.end_hour
    }
}

impl TokenRatesConfig {
    fn validate(&self) -> Result<(), String> {
        for (name, rate) in [
            ("input_cost_per_token", self.input_cost_per_token),
            ("output_cost_per_token", self.output_cost_per_token),
            (
                "cache_read_input_token_cost",
                self.cache_read_input_token_cost,
            ),
        ] {
            if !rate.is_finite() || rate < 0.0 {
                return Err(format!(
                    "{TIME_OF_USE_PRICING_KEY}.peak_rates.{name} must be finite and non-negative"
                ));
            }
        }
        Ok(())
    }
}

impl From<TokenRatesConfig> for TokenRates {
    fn from(rates: TokenRatesConfig) -> Self {
        Self {
            input_cost_per_token: rates.input_cost_per_token,
            output_cost_per_token: rates.output_cost_per_token,
            cache_read_input_token_cost: rates.cache_read_input_token_cost,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::HashMap;

    fn model_info(schedule: serde_json::Value) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: Some(1.0),
            output_cost_per_token: Some(2.0),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "test".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::from([(TIME_OF_USE_PRICING_KEY.to_string(), schedule)]),
        }
    }

    fn deepseek_schedule() -> serde_json::Value {
        serde_json::json!({
            "timezone": "UTC",
            "peak_windows": [
                {"weekdays": [1, 2, 3, 4, 5], "start_hour": 1, "end_hour": 4},
                {"weekdays": [1, 2, 3, 4, 5], "start_hour": 6, "end_hour": 10}
            ],
            "peak_rates": {
                "input_cost_per_token": 3.0,
                "output_cost_per_token": 4.0,
                "cache_read_input_token_cost": 5.0
            }
        })
    }

    #[test]
    fn selects_half_open_weekday_peak_windows() {
        let info = model_info(deepseek_schedule());
        for (hour, minute, is_peak) in [
            (0, 59, false),
            (1, 0, true),
            (3, 59, true),
            (4, 0, false),
            (5, 59, false),
            (6, 0, true),
            (9, 59, true),
            (10, 0, false),
        ] {
            let at = Utc.with_ymd_and_hms(2026, 8, 24, hour, minute, 0).unwrap();
            assert_eq!(peak_token_rates_at(&info, at).unwrap().is_some(), is_peak);
        }

        let saturday = Utc.with_ymd_and_hms(2026, 8, 29, 2, 0, 0).unwrap();
        assert!(peak_token_rates_at(&info, saturday).unwrap().is_none());
    }

    #[test]
    fn rejects_a_malformed_declared_schedule() {
        let info = model_info(serde_json::json!({
            "timezone": "America/Los_Angeles",
            "peak_windows": [
                {"weekdays": [0], "start_hour": 4, "end_hour": 1}
            ],
            "peak_rates": {
                "input_cost_per_token": -1.0,
                "output_cost_per_token": 4.0,
                "cache_read_input_token_cost": 5.0
            }
        }));
        let at = Utc.with_ymd_and_hms(2026, 8, 24, 2, 0, 0).unwrap();
        let error = peak_token_rates_at(&info, at).unwrap_err();
        assert!(error.contains("timezone must be UTC"));
    }

    #[test]
    fn maximum_rates_never_drop_below_scalar_rates() {
        let info = model_info(serde_json::json!({
            "timezone": "UTC",
            "peak_windows": [
                {"weekdays": [1], "start_hour": 1, "end_hour": 4}
            ],
            "peak_rates": {
                "input_cost_per_token": 0.5,
                "output_cost_per_token": 1.0,
                "cache_read_input_token_cost": 0.25
            }
        }));
        let rates = configured_peak_token_rates(&info).unwrap().unwrap();
        assert_eq!(rates.input_cost_per_token, 1.0);
        assert_eq!(rates.output_cost_per_token, 2.0);
        assert_eq!(rates.cache_read_input_token_cost, 1.0);
    }
}
