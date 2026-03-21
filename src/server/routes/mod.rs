//! HTTP route modules
//!
//! This module contains all HTTP route handlers organized by functionality.

pub mod ai;
pub mod auth;
pub mod budget;
pub mod health;
pub mod keys;
pub mod pricing;
pub mod teams;

use actix_web::HttpResponse;

/// Standard API response structure
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiResponse<T> {
    /// Whether the request was successful
    pub success: bool,
    /// Response data (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl<T> ApiResponse<T>
where
    T: serde::Serialize,
{
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: None,
        }
    }

    /// Create an error response
    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
            meta: None,
        }
    }
}

impl<T> ApiResponse<T> {
    /// Create an error response for any type
    pub fn error_for_type(message: String) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
            meta: None,
        }
    }
}

impl<T> ApiResponse<T>
where
    T: serde::Serialize,
{
    /// Convert the API response to an HTTP response
    ///
    /// Returns HTTP 200 for successful responses and HTTP 400 for error responses
    pub fn to_http_response(&self) -> HttpResponse {
        if self.success {
            HttpResponse::Ok().json(self)
        } else {
            HttpResponse::BadRequest().json(self)
        }
    }
}

/// Pagination metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct PaginationMeta {
    /// Current page number
    pub page: u32,
    /// Number of items per page
    pub limit: u32,
    /// Total number of items
    pub total: u64,
    /// Total number of pages
    pub pages: u32,
    /// Whether there is a next page
    pub has_next: bool,
    /// Whether there is a previous page
    pub has_prev: bool,
}

impl PaginationMeta {
    /// Create pagination metadata
    pub fn new(page: u32, limit: u32, total: u64) -> Self {
        let pages = ((total as f64) / (limit as f64)).ceil() as u32;

        Self {
            page,
            limit,
            total,
            pages,
            has_next: page < pages,
            has_prev: page > 1,
        }
    }
}

/// Paginated response
#[derive(Debug, Clone, serde::Serialize)]
pub struct PaginatedResponse<T> {
    /// Response items
    pub items: Vec<T>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

impl<T> PaginatedResponse<T>
where
    T: serde::Serialize,
{
    /// Create a paginated response
    pub fn new(items: Vec<T>, page: u32, limit: u32, total: u64) -> Self {
        Self {
            items,
            pagination: PaginationMeta::new(page, limit, total),
        }
    }
}

/// Query parameters for pagination
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PaginationQuery {
    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Number of items per page
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 {
    1
}

fn default_limit() -> u32 {
    20
}

impl PaginationQuery {
    /// Validate pagination parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.page == 0 {
            return Err("Page must be greater than 0".to_string());
        }
        if self.limit == 0 {
            return Err("Limit must be greater than 0".to_string());
        }
        if self.limit > 1000 {
            return Err("Limit cannot exceed 1000".to_string());
        }
        Ok(())
    }

    /// Get offset for database queries
    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.limit
    }
}

/// Query parameters for sorting
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SortQuery {
    /// Field to sort by
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    /// Sort order (asc or desc)
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

fn default_sort_by() -> String {
    "created_at".to_string()
}

fn default_sort_order() -> String {
    "desc".to_string()
}

impl SortQuery {
    /// Validate sort parameters
    pub fn validate(&self, valid_fields: &[&str]) -> Result<(), String> {
        if !valid_fields.contains(&self.sort_by.as_str()) {
            return Err(format!("Invalid sort field: {}", self.sort_by));
        }
        if !["asc", "desc"].contains(&self.sort_order.as_str()) {
            return Err("Sort order must be 'asc' or 'desc'".to_string());
        }
        Ok(())
    }
}

/// Combined query parameters for listing endpoints
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ListQuery {
    /// Pagination parameters (page, limit)
    #[serde(flatten)]
    pub pagination: PaginationQuery,
    /// Sorting parameters (sort_by, sort_order)
    #[serde(flatten)]
    pub sort: SortQuery,
}

impl ListQuery {
    /// Validate all query parameters
    pub fn validate(&self, valid_sort_fields: &[&str]) -> Result<(), String> {
        self.pagination.validate()?;
        self.sort.validate(valid_sort_fields)?;
        Ok(())
    }
}

/// Error response helpers
pub mod errors {
    use super::*;
    use crate::utils::error::gateway_error::GatewayError;
    use actix_web::ResponseError;

    /// Convert GatewayError to HTTP response.
    ///
    /// Kept as a compatibility shim while routes migrate to `ResponseError` directly.
    pub fn gateway_error_to_response(error: GatewayError) -> HttpResponse {
        error.error_response()
    }

    /// Create a validation error response
    pub fn validation_error(message: &str) -> HttpResponse {
        HttpResponse::BadRequest().json(ApiResponse::<()>::error(message.to_string()))
    }

    /// Create an unauthorized error response
    pub fn unauthorized_error(message: &str) -> HttpResponse {
        HttpResponse::Unauthorized().json(ApiResponse::<()>::error(message.to_string()))
    }

    /// Create a forbidden error response
    pub fn forbidden_error(message: &str) -> HttpResponse {
        HttpResponse::Forbidden().json(ApiResponse::<()>::error(message.to_string()))
    }

    /// Create a not found error response
    pub fn not_found_error(message: &str) -> HttpResponse {
        HttpResponse::NotFound().json(ApiResponse::<()>::error(message.to_string()))
    }

    /// Create an internal server error response
    pub fn internal_error(message: &str) -> HttpResponse {
        HttpResponse::InternalServerError().json(ApiResponse::<()>::error(message.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response = ApiResponse::<()>::error("test error".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_pagination_meta() {
        let meta = PaginationMeta::new(2, 10, 25);
        assert_eq!(meta.page, 2);
        assert_eq!(meta.limit, 10);
        assert_eq!(meta.total, 25);
        assert_eq!(meta.pages, 3);
        assert!(meta.has_next);
        assert!(meta.has_prev);
    }

    #[test]
    fn test_pagination_query_validation() {
        let valid_query = PaginationQuery { page: 1, limit: 20 };
        assert!(valid_query.validate().is_ok());

        let invalid_page = PaginationQuery { page: 0, limit: 20 };
        assert!(invalid_page.validate().is_err());

        let invalid_limit = PaginationQuery { page: 1, limit: 0 };
        assert!(invalid_limit.validate().is_err());

        let too_large_limit = PaginationQuery {
            page: 1,
            limit: 2000,
        };
        assert!(too_large_limit.validate().is_err());
    }

    #[test]
    fn test_pagination_query_offset() {
        let query = PaginationQuery { page: 3, limit: 10 };
        assert_eq!(query.offset(), 20);
    }

    #[test]
    fn test_sort_query_validation() {
        let valid_fields = &["name", "created_at", "updated_at"];

        let valid_query = SortQuery {
            sort_by: "name".to_string(),
            sort_order: "asc".to_string(),
        };
        assert!(valid_query.validate(valid_fields).is_ok());

        let invalid_field = SortQuery {
            sort_by: "invalid".to_string(),
            sort_order: "asc".to_string(),
        };
        assert!(invalid_field.validate(valid_fields).is_err());

        let invalid_order = SortQuery {
            sort_by: "name".to_string(),
            sort_order: "invalid".to_string(),
        };
        assert!(invalid_order.validate(valid_fields).is_err());
    }
}
