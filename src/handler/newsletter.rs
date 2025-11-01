use crate::{
    AppState,
    db::NewsletterExt,
    dtos::{NewsletterDto, Response},
    error::HttpError,
};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, post},
};
use validator::Validate;

/// Router for newsletter subscription endpoints
pub fn newsletter_handler() -> Router<AppState> {
    Router::new()
        // POST / - Subscribe to newsletter
        .route("/", post(add_newsletter_email))
        // DELETE / - Unsubscribe from newsletter
        .route("/", delete(delete_newsletter_email))
}

/// Subscribe email to newsletter
///
/// Request body: { email }
/// Returns 201 Created on success or 409 if already subscribed.
pub async fn add_newsletter_email(
    State(app_state): State<AppState>,
    Json(body): Json<NewsletterDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate email format
    body.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let result = app_state.db_client.add_newsletter_email(&body.email).await;

    match result {
        Ok(_) => {
            let response = Response {
                status: "success",
                message: "Successfully subscribed to the newsletter.".to_string(),
            };
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(sqlx::Error::Database(db_err)) => {
            // Handle duplicate email (unique constraint violation)
            if db_err.is_unique_violation() {
                Err(HttpError::unique_constraint_violation(
                    "Email already exists.".to_string(),
                ))
            } else {
                Err(HttpError::server_error(db_err.to_string()))
            }
        }
        Err(e) => Err(HttpError::server_error(e.to_string())),
    }
}

/// Unsubscribe email from newsletter
///
/// Request body: { email }
/// Returns 404 if email not found in subscriptions.
pub async fn delete_newsletter_email(
    State(app_state): State<AppState>,
    Json(body): Json<NewsletterDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate email format
    body.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let result = app_state
        .db_client
        .delete_newsletter_email(&body.email)
        .await;

    match result {
        Ok(_) => {
            let response = Response {
                status: "success",
                message: "Successfully unsubscribed from the newsletter.".to_string(),
            };
            Ok((StatusCode::OK, Json(response)))
        }
        Err(sqlx::Error::RowNotFound) => {
            // Email not in newsletter subscriptions
            Err(HttpError::not_found("Email not found.".to_string()))
        }
        Err(e) => Err(HttpError::server_error(e.to_string())),
    }
}
