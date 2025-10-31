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
pub fn newsletter_handler() -> Router<AppState> {
    Router::new()
        .route("/", post(add_newsletter_email))
        .route("/", delete(delete_newsletter_email))
}
pub async fn add_newsletter_email(
    State(app_state): State<AppState>,
    Json(body): Json<NewsletterDto>,
) -> Result<impl IntoResponse, HttpError> {
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
pub async fn delete_newsletter_email(
    State(app_state): State<AppState>,
    Json(body): Json<NewsletterDto>,
) -> Result<impl IntoResponse, HttpError> {
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
        Err(sqlx::Error::RowNotFound) => Err(HttpError::not_found("Email not found.".to_string())),
        Err(e) => Err(HttpError::server_error(e.to_string())),
    }
}
