use crate::{
    AppState,
    db::NewsletterExt,
    dtos::{NewsletterDto, Response},
    error::{ErrorMessage, HttpError},
};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, post},
};
use tracing::instrument;
use validator::Validate;

pub fn newsletter_handler() -> Router<AppState> {
    Router::new()
        .route("/", post(add_newsletter_email))
        .route("/", delete(delete_newsletter_email))
}

#[instrument(skip(app_state, body), fields(email = %body.email))]
pub async fn add_newsletter_email(
    State(app_state): State<AppState>,
    Json(body): Json<NewsletterDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid add_newsletter_email input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let result = app_state.db_client.add_newsletter_email(&body.email).await;

    match result {
        Ok(_) => {
            let response = Response {
                status: "success",
                message: "Successfully subscribed to the newsletter.".to_string(),
            };
            tracing::info!("Successfully subscribed to the newsletter.");
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(sqlx::Error::Database(db_err)) => {
            // Handle duplicate email (unique constraint violation)
            if db_err.is_unique_violation() {
                tracing::error!("DB error, unique_violation: {}", db_err);
                Err(HttpError::unique_constraint_violation(
                    "Email already exists.".to_string(),
                ))
            } else {
                tracing::error!("DB error, adding newsletter email: {}", db_err);
                Err(HttpError::server_error(db_err.to_string()))
            }
        }
        Err(e) => {
            tracing::error!("DB error, adding newsletter email: {}", e);
            Err(HttpError::server_error(
                ErrorMessage::ServerError.to_string(),
            ))
        }
    }
}

/// Unsubscribe email from newsletter
///
/// Request body: { email }
/// Returns 404 if email not found in subscriptions.
#[instrument(skip(app_state, body), fields(email = %body.email))]
pub async fn delete_newsletter_email(
    State(app_state): State<AppState>,
    Json(body): Json<NewsletterDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid delete_newsletter_email input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

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
            tracing::info!("Successfully unsubscribed from the newsletter.");
            Ok((StatusCode::OK, Json(response)))
        }
        Err(sqlx::Error::RowNotFound) => {
            tracing::error!("Email not found in newsletter subscriptions");
            // Email not in newsletter subscriptions
            Err(HttpError::not_found("Email not found.".to_string()))
        }
        Err(e) => {
            tracing::error!("DB error, deleting newsletter email: {}", e);
            Err(HttpError::server_error(
                ErrorMessage::ServerError.to_string(),
            ))
        }
    }
}
