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

/// Router for search endpoints
pub fn search_handler() -> Router<AppState> {
    Router::new().route("/", get(get_hybrid_search))
}

/// Hybrid search combining full-text and semantic search
///
/// **Hybrid Search Approach:**
/// Combines two search methods:
/// 1. Full-text search: Keyword matching (fast, exact matches)
/// 2. Semantic search: Vector similarity (slow, meaning-based)
///
/// Results include both methods, ranked by relevance.
/// This gives better search quality than either method alone.
///
/// **Search Flow:**
/// 1. Generate embedding (vector) from search query using gRPC
/// 2. Query database for paginated posts matching search criteria
/// 3. Count total matching posts (for pagination metadata)
/// 4. Return paginated results with pagination info
///
/// **Why Two Separate Queries?**
/// - `hybrid_search_posts`: Returns the actual post data (limited by LIMIT/OFFSET)
/// - `hybrid_search_posts_count`: Returns total count of ALL matching posts
///
/// With pagination, we need to know:
/// - What posts to display on this page (requires LIMIT/OFFSET)
/// - Total number of posts for pagination UI (requires COUNT)
///
/// These require separate queries because the LIMIT/OFFSET prevents
/// the count query from seeing all results.
///
/// # Query Parameters
/// - `q`: Search query string (required)
/// - `page`: Page number for pagination (optional, default: 1)
/// - `limit`: Results per page (optional, default: 10)
///
/// # Returns
/// - `Ok(Json)`: Paginated search results with pagination info
/// - `Err(HttpError)`: If validation fails (400) or database error (500)
#[instrument(skip(app_state))]
pub async fn get_hybrid_search(
    Query(params): Query<GetSearchQuery>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate query parameters (q must not be empty)
    params.validate().map_err(|e| {
        tracing::error!("Invalid get_hybrid_search input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    // Extract search parameters with defaults
    let q = params.q;
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);
    let lang = params.lang.unwrap_or(Lang::En);

    // Generate embedding for the search query using gRPC
    // Converts text query into 768-dimensional vector (embeddinggemma output)
    // This vector is used for semantic similarity search in database
    let embedding = app_state.grpc_client.get_embedding_query(&q).await?;

    // Query 1: Fetch paginated results
    // Database combines full-text search and vector similarity search,
    // returns paginated results (LIMIT/OFFSET applied)
    let search_result = app_state
        .db_client
        .hybrid_search_posts(&q, embedding.clone(), page, limit, lang)
        .await
        .map_err(|e| {
            tracing::error!("DB error, hybrid searching posts: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Query 2: Count total matching posts
    // Gets total count without pagination limits (no LIMIT/OFFSET)
    // Needed to calculate total pages and show pagination metadata
    let total = app_state
        .db_client
        .hybrid_search_posts_count(&q, embedding)
        .await
        .map_err(|e| {
            tracing::error!("DB error, hybrid searching posts count: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Calculate total pages for frontend pagination UI
    // Ceiling division: (10 results, 3 per page) = 4 pages
    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    // Build paginated response with metadata
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
