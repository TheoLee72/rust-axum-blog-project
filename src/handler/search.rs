use axum::Router;
use axum::routing::get;
use axum::response::{IntoResponse, Json};
use axum::extract::{Query, State};
use validator::Validate;
use crate::dtos::{GetSearchQuery, PostsPaginationResponseDto};
use crate::db::PostExt;
use crate::AppState;
use crate::error::HttpError;
pub fn search_handler() -> Router<AppState> {
    Router::new()
        .route("/", get(get_hybrid_search))
}

pub async fn get_hybrid_search(
    Query(params): Query<GetSearchQuery>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params.validate().map_err(|e| HttpError::bad_request(e.to_string()))?;

    let q = params.q.unwrap();

    let embedding = app_state.grpc_client.get_embedding_query(&q).await?;
    //embedding가지고 sqlx 검색.
    let search_result = app_state.db_client.hybrid_search_posts(&q, embedding)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let response = Json(PostsPaginationResponseDto{
        status: "success".to_string(),
        data: search_result,
        pagination: None,
        });
    
    Ok(response)
}