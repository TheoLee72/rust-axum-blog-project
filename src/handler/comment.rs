use crate::AppState;
use crate::db::CommentExt;
use crate::dtos::{
    CommentListResponse, GetcommentsQuery, InputcommentRequest, PaginationDto,
    SinglecommentResponse,
};
use crate::error::{ErrorMessage, HttpError};
use crate::middleware::JWTAuthMiddleware;
use crate::middleware::auth;
use axum::Extension;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post, put};
use axum::{Router, middleware};
use tracing::instrument;
use validator::Validate;

pub fn comment_handler(app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(get_comments))
        .route(
            "/",
            post(create_comment)
                .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)),
        )
        .route(
            "/{comment_id}",
            put(edit_comment)
                .delete(delete_comment)
                .route_layer(middleware::from_fn_with_state(app_state, auth)),
        )
}

#[instrument(skip(app_state))]
pub async fn get_comments(
    Query(params): Query<GetcommentsQuery>,
    Path(post_id): Path<i32>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params.validate().map_err(|e| {
        tracing::error!("Invalid get_comments input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);
    let sort = params.sort.unwrap_or("created_at_desc".to_string());

    let comments = app_state
        .db_client
        .get_comments(post_id, page, limit, &sort)
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting comments: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let total = app_state
        .db_client
        .get_post_comment_count(post_id)
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting post comment count: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    let response = Json(CommentListResponse {
        status: "success".to_string(),
        data: comments,
        pagination: PaginationDto {
            page,
            limit,
            total: total as i32,
            total_pages,
        },
    });
    tracing::info!("get_comments successful");
    Ok(response)
}

#[instrument(skip(app_state, body, jwt), fields(username = %jwt.user.username))]
pub async fn create_comment(
    Path(post_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Json(body): Json<InputcommentRequest>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid create_comment input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user_id = jwt.user.id;

    let comment = app_state
        .db_client
        .create_comment(user_id, post_id, &body.content)
        .await
        .map_err(|e| {
            tracing::error!("DB error, creating comment: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let response = Json(SinglecommentResponse {
        status: "success".to_string(),
        data: comment,
    });
    tracing::info!("create_comment successful");
    Ok((StatusCode::CREATED, response))
}

#[instrument(skip(app_state, body, jwt), fields(username = %jwt.user.username))]
pub async fn edit_comment(
    Path(comment_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Json(body): Json<InputcommentRequest>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid edit_comment input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user_id = jwt.user.id;

    let comment = app_state
        .db_client
        .edit_comment(user_id, comment_id, &body.content)
        .await
        .map_err(|e| {
            tracing::error!("DB error, editing comment: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let response = Json(SinglecommentResponse {
        status: "success".to_string(),
        data: comment,
    });
    tracing::info!("edit_comment successful");
    Ok(response)
}

#[instrument(skip(app_state, jwt))]
async fn delete_comment(
    Path(comment_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
) -> Result<impl IntoResponse, HttpError> {
    let user_id = jwt.user.id;

    app_state
        .db_client
        .delete_comment(user_id, comment_id)
        .await
        .map_err(|e| {
            tracing::error!("DB error, deleting comment: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;
    tracing::info!("delete_comment successful");
    Ok(StatusCode::NO_CONTENT)
}
