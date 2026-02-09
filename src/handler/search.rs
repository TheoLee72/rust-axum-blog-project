use crate::AppState;
use crate::db::PostExt;
use crate::dtos::{GetSearchQuery, Lang, PaginationDto, PostsPaginationResponseDto};
use crate::error::{ErrorMessage, HttpError};
use axum::Router;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use tracing::instrument;
use validator::Validate;

pub fn search_handler() -> Router<AppState> {
    Router::new().route("/", get(get_hybrid_search))
}

#[instrument(skip(app_state))]
pub async fn get_hybrid_search(
    Query(params): Query<GetSearchQuery>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params.validate().map_err(|e| {
        tracing::error!("Invalid get_hybrid_search input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let q = params.q;
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);
    let lang = params.lang.unwrap_or(Lang::En);

    let embedding = app_state.grpc_client.get_embedding_query(&q).await?;

    let search_result = app_state
        .db_client
        .hybrid_search_posts(&q, embedding.clone(), page, limit, lang)
        .await
        .map_err(|e| {
            tracing::error!("DB error, hybrid searching posts: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let total = app_state
        .db_client
        .hybrid_search_posts_count(&q, embedding)
        .await
        .map_err(|e| {
            tracing::error!("DB error, hybrid searching posts count: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    let response = Json(PostsPaginationResponseDto {
        status: "success".to_string(),
        data: search_result,
        pagination: Some(PaginationDto {
            page,
            limit,
            total: total as i32,
            total_pages,
        }),
    });
    tracing::info!("get_hybrid_search successful");
    Ok(response)
}
