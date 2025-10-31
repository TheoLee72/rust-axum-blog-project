use crate::AppState;
use crate::db::PostExt;
use crate::dtos::{GetSearchQuery, PaginationDto, PostsPaginationResponseDto};
use crate::error::HttpError;
use axum::Router;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use validator::Validate;
pub fn search_handler() -> Router<AppState> {
    Router::new().route("/", get(get_hybrid_search))
}

pub async fn get_hybrid_search(
    Query(params): Query<GetSearchQuery>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params
        .validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let q = params.q;
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);

    let embedding = app_state.grpc_client.get_embedding_query(&q).await?;
    //embedding가지고 sqlx 검색.
    let search_result = app_state
        .db_client
        .hybrid_search_posts(&q, embedding.clone(), page, limit)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let total = app_state
        .db_client
        .hybrid_search_posts_count(&q, embedding)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;
    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    let response = Json(PostsPaginationResponseDto {
        status: "success".to_string(),
        data: search_result,
        pagination: Some(PaginationDto {
            page: page,
            limit: limit,
            total: total as i32,
            total_pages,
        }),
    });

    Ok(response)
}
