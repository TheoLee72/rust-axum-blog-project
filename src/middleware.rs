use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::IntoResponse,
};

use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    db::UserExt,
    error::{ErrorMessage, HttpError},
    models::{User, UserRole},
    utils::token,
};

/// Middleware extension that stores authenticated user information
///
/// This struct is inserted into the request extensions after successful authentication.
/// Subsequent handlers can extract this to access the authenticated user's data.
///
/// Example usage in a handler:
/// ```
/// async fn my_handler(Extension(auth): Extension<JWTAuthMiddleware>) {
///     // Access auth.user here
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JWTAuthMiddleware {
    pub user: User,
}

/// Authentication middleware that validates JWT tokens
///
/// This middleware performs the following steps:
/// 1. Extracts JWT from cookies or Authorization header
/// 2. Validates and decodes the JWT token
/// 3. Fetches the user from the database
/// 4. Attaches user info to request extensions for downstream handlers
///
/// Token extraction priority:
/// - First: Check `access_token` cookie (for browser-based clients)
/// - Second: Check `Authorization: Bearer <token>` header (for API clients)
///
/// # Errors
/// Returns 401 Unauthorized if:
/// - No token is provided
/// - Token is invalid or expired
/// - User no longer exists in database
pub async fn auth(
    cookie_jar: CookieJar,
    State(app_state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<impl IntoResponse, HttpError> {
    // Attempt to extract JWT token from two possible sources:
    // 1. Cookie (preferred for browser clients with same-origin requests)
    // 2. Authorization header (preferred for API clients and cross-origin requests)
    let cookies = cookie_jar
        .get("access_token")
        .map(|cookie| cookie.value().to_string())
        .or_else(|| {
            req.headers()
                .get(header::AUTHORIZATION)
                .and_then(|auth_header| auth_header.to_str().ok()) // Convert Result to Option using .ok()
                .and_then(|auth_value| {
                    // Extract token from "Bearer <token>" format
                    if auth_value.starts_with("Bearer ") {
                        Some(auth_value[7..].to_owned()) // Skip "Bearer " prefix (7 characters)
                    } else {
                        None
                    }
                })
        });

    // Convert Option to Result - return error if no token was found
    // .ok_or_else() transforms Option<T> into Result<T, E>
    let token = cookies
        .ok_or_else(|| HttpError::unauthorized(ErrorMessage::TokenNotProvided.to_string()))?;

    // Decode and verify the JWT token
    // This checks:
    // - Token signature is valid (hasn't been tampered with)
    // - Token hasn't expired
    // - Token was signed with the correct secret key
    let token_details = match token::decode_token(token, app_state.env.jwt_secret.as_bytes()) {
        Ok(token_details) => token_details,
        Err(_) => {
            return Err(HttpError::unauthorized(
                ErrorMessage::InvalidToken.to_string(),
            ));
        }
    };

    // Extract user ID from token claims and parse into UUID format
    // Token details contain the user ID as a string representation
    let user_id = uuid::Uuid::parse_str(&token_details.to_string())
        .map_err(|_| HttpError::unauthorized(ErrorMessage::InvalidToken.to_string()))?;

    // Fetch user from database using the ID from the token
    // We only search by user_id (other parameters are None)
    // This ensures the user still exists and hasn't been deleted
    let user = app_state
        .db_client
        .get_user(Some(user_id), None, None, None)
        .await
        .map_err(|_| HttpError::unauthorized(ErrorMessage::UserNoLongerExist.to_string()))?;

    // Handle case where user was found in token but not in database
    // This can happen if the user was deleted after the token was issued
    let user =
        user.ok_or_else(|| HttpError::unauthorized(ErrorMessage::UserNoLongerExist.to_string()))?;

    // Insert authenticated user into request extensions
    // This makes the user available to all downstream handlers and middleware
    // without needing to re-authenticate or query the database
    req.extensions_mut()
        .insert(JWTAuthMiddleware { user: user.clone() });

    // Pass the request to the next middleware/handler in the chain
    Ok(next.run(req).await)
}

/// Role-based access control (RBAC) middleware
///
/// This middleware checks if the authenticated user has one of the required roles
/// to access a protected route. It must be used after the `auth` middleware.
///
/// ```
///
/// # Parameters
/// - `req`: The incoming request (must have been processed by auth middleware)
/// - `next`: The next middleware/handler in the chain
/// - `required_roles`: List of roles allowed to access this route
///
/// # Errors
/// Returns 401 if user is not authenticated
/// Returns 403 if user doesn't have any of the required roles
pub async fn role_check(
    req: Request,
    next: Next,
    required_roles: Vec<UserRole>,
) -> Result<impl IntoResponse, HttpError> {
    // Extract authenticated user from request extensions
    // This was inserted by the auth middleware earlier in the chain
    let user = req
        .extensions()
        .get::<JWTAuthMiddleware>()
        .ok_or_else(|| HttpError::unauthorized(ErrorMessage::UserNotAuthenticated.to_string()))?;

    // Check if user's role matches any of the required roles
    // For example, if required_roles = [Admin, Moderator] and user is Admin, allow access
    if !required_roles.contains(&user.user.role) {
        return Err(HttpError::new(
            ErrorMessage::PermissionDenied.to_string(),
            StatusCode::FORBIDDEN, // 403: User is authenticated but lacks permissions
        ));
    }

    // User has required role - proceed to the next handler
    Ok(next.run(req).await)
}
