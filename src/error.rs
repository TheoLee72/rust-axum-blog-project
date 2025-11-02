use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Error response structure sent to clients
///
/// This struct represents the JSON error format returned to API clients.
/// It provides a consistent error response structure across all endpoints.
///
/// Example JSON response:
/// ```
/// {
///   "status": "fail",
///   "message": "Email or password is wrong"
/// }
/// ```
///
/// Why separate from HttpError?
/// - ErrorResponse: External format for API responses (what clients see)
/// - HttpError: Internal error type with additional context (what handlers use)
///
/// This separation allows us to:
/// - Hide sensitive internal details from clients
/// - Add internal-only fields to HttpError without affecting API contract
/// - Transform errors before sending to clients
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: String, // Always "fail" for errors (could also be "error" for server errors)
    pub message: String, // Human-readable error message
}

impl fmt::Display for ErrorResponse {
    /// Implement Display trait to control how ErrorResponse is formatted with {}
    ///
    /// This allows ErrorResponse to be printed or converted to strings easily.
    ///
    /// Implementation details:
    /// - Serializes the struct to JSON format using serde_json
    /// - Writes to a formatter buffer (f)
    /// - The buffer is later used by println!, format!, etc.
    ///
    /// Why &mut fmt::Formatter?
    /// - Formatters maintain internal state (buffer position, flags, etc.)
    /// - Writing to the buffer modifies this state
    /// - Therefore, we need mutable access
    ///
    /// # Returns
    /// - `Ok(())`: Successfully written to the formatter
    /// - `Err(fmt::Error)`: Failed to serialize or write
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // write! is a macro that writes formatted data to a buffer
        // println! reads from this buffer and outputs to stdout
        match serde_json::to_string(self) {
            Ok(s) => write!(f, "{}", s), // Successfully serialized to JSON string
            Err(_) => Err(fmt::Error),   // Serialization failed
        }
    }
}

/// Enumeration of all possible error types in the application
///
/// This enum provides type-safe error variants with clear semantic meaning.
/// Each variant represents a specific error condition in the application.
///
/// Benefits of using an enum:
/// - Exhaustive matching: Compiler ensures all cases are handled
/// - Type safety: Can't accidentally use wrong error message
/// - Easy refactoring: Change message in one place
/// - Self-documenting: Error names describe what went wrong
///
/// PartialEq allows comparing error variants (useful in tests)
#[derive(Debug, PartialEq)]
pub enum ErrorMessage {
    // Password validation errors
    EmptyPassword,
    ExceededMaxPasswordLength(usize), // Contains the max length value
    InvalidHashFormat,
    HashingError,

    // Authentication errors
    InvalidToken,
    TokenNotProvided,
    UserNotAuthenticated,

    // Authorization errors
    PermissionDenied,

    // User management errors
    UserNoLongerExist,

    //Else
    ServerError,
}

impl fmt::Display for ErrorMessage {
    /// Convert ErrorMessage to user-friendly string
    ///
    /// This formats the enum variant as a human-readable error message.
    /// Same messages as ToString for consistency - users see the same error text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            ErrorMessage::UserNoLongerExist => {
                "User belonging to this token no longer exists".to_string()
            }
            ErrorMessage::EmptyPassword => "Password cannot be empty".to_string(),
            ErrorMessage::HashingError => "Error while hashing password".to_string(),
            ErrorMessage::InvalidHashFormat => "Invalid password hash format".to_string(),
            ErrorMessage::ExceededMaxPasswordLength(max_length) => {
                format!("Password must not be more than {} characters", max_length)
            }
            ErrorMessage::InvalidToken => "Token is invalid or expired".to_string(),
            ErrorMessage::TokenNotProvided => {
                "You are not logged in, please provide a token".to_string()
            }
            ErrorMessage::PermissionDenied => {
                "You are not allowed to perform this action".to_string()
            }
            ErrorMessage::UserNotAuthenticated => {
                "Authentication required. Please log in.".to_string()
            }
            ErrorMessage::ServerError => "Server Error. Please try again later".to_string(),
        };
        write!(f, "{}", message)
    }
}

/// Internal HTTP error type used throughout the application
///
/// This is the primary error type used in handlers, middleware, and business logic.
/// It combines an error message with an HTTP status code.
///
/// Why this design?
/// - Handlers can return Result<T, HttpError> for consistent error handling
/// - Axum automatically converts HttpError to HTTP responses (via IntoResponse)
/// - Status codes are bundled with messages (no risk of mismatch)
/// - Easy to construct with builder methods (unauthorized(), bad_request(), etc.)
///
/// Clone allows passing errors around without ownership issues
#[derive(Debug, Clone)]
pub struct HttpError {
    pub message: String,    // Error message for the client
    pub status: StatusCode, // HTTP status code (400, 401, 500, etc.)
}

impl HttpError {
    /// Generic constructor for creating any HttpError
    ///
    /// # Parameters
    /// - `message`: Anything that can be converted into a String
    ///   (String, &str, ErrorMessage, etc.)
    /// - `status`: HTTP status code from axum::http::StatusCode
    ///
    /// The `impl Into<String>` pattern allows flexible input:
    /// - HttpError::new("error", StatusCode::BAD_REQUEST)
    /// - HttpError::new(String::from("error"), StatusCode::BAD_REQUEST)
    /// - HttpError::new(ErrorMessage::EmptyPassword.to_string(), StatusCode::BAD_REQUEST)
    ///
    /// Into and From traits are usually paired - if From is implemented,
    /// Into is automatically available.
    pub fn new(message: impl Into<String>, status: StatusCode) -> Self {
        HttpError {
            message: message.into(), // Automatically converts to String
            status,
        }
    }

    /// Convenience constructor for 500 Internal Server Error
    ///
    /// Use this for unexpected errors, database failures, external service failures, etc.
    /// These indicate something went wrong on the server side, not user error.
    pub fn server_error(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR, // 500
        }
    }

    /// Convenience constructor for 400 Bad Request
    ///
    /// Use this for invalid input, malformed requests, validation failures, etc.
    /// These indicate the client sent something wrong.
    pub fn bad_request(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::BAD_REQUEST, // 400
        }
    }

    /// Convenience constructor for 409 Conflict
    ///
    /// Use this for database constraint violations (unique email, duplicate username, etc.)
    /// Indicates the request conflicts with existing data.
    pub fn unique_constraint_violation(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::CONFLICT, // 409
        }
    }

    /// Convenience constructor for 401 Unauthorized
    ///
    /// Use this for authentication failures (invalid credentials, expired tokens, etc.)
    /// Client needs to authenticate or re-authenticate.
    ///
    /// Note: Despite the name, 401 means "unauthenticated", not "unauthorized"
    pub fn unauthorized(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::UNAUTHORIZED, // 401
        }
    }

    /// Convenience constructor for 404 Not Found
    ///
    /// Use this when a requested resource doesn't exist (user, post, comment, etc.)
    pub fn not_found(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::NOT_FOUND, // 404
        }
    }

    /// Convert HttpError into an Axum HTTP Response
    ///
    /// This creates a JSON response with the error message and appropriate status code.
    ///
    /// Response format:
    /// - Status code: From self.status
    /// - Content-Type: application/json
    /// - Body: {"status": "fail", "message": "..."}
    pub fn into_http_response(self) -> Response {
        let json_response = Json(ErrorResponse {
            status: "fail".to_string(),
            message: self.message.clone(),
        });

        // Create a tuple of (StatusCode, Json) and convert to Response
        // Axum automatically handles the conversion
        (self.status, json_response).into_response()
    }
}

impl fmt::Display for HttpError {
    /// Implement Display for logging and debugging
    ///
    /// This format is used internally for logging, not sent to clients.
    /// Provides more detailed information than what clients see.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HttpError: message: {}, status: {}",
            self.message, self.status
        )
    }
}

/// Mark HttpError as a standard Rust error type
///
/// This allows HttpError to integrate with Rust's error handling ecosystem:
/// - Can be used with ? operator
/// - Can be boxed in Result<T, Box<dyn Error>>
/// - Works with error reporting libraries
/// - Compatible with std::error::Error trait bounds
///
/// The empty implementation is fine because we already have Display and Debug
impl std::error::Error for HttpError {}

/// Implement IntoResponse to integrate with Axum's error handling
///
/// This trait allows HttpError to be returned directly from handlers:
///
/// ```
/// async fn my_handler() -> Result<Json<User>, HttpError> {
///     let user = db.get_user().await
///         .map_err(|_| HttpError::not_found("User not found"))?;
///     Ok(Json(user))
/// }
/// ```
///
/// When a handler returns Err(HttpError), Axum automatically:
/// 1. Calls into_response() on the error
/// 2. Sends the resulting HTTP response to the client
///
/// This eliminates boilerplate error handling in every handler
impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        self.into_http_response()
    }
}
