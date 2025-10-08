use axum::http::StatusCode;
use axum::{middleware, Router};
use axum::routing::{get, post, put};
use axum::response::{IntoResponse, Json};
use axum::extract::{Path, Query, State};
use axum::Extension;
use validator::Validate;
use crate::dtos::{GetReviewsQuery, PaginationDto, ReviewListResponse, SingleReviewResponse, InputReviewRequest};
use crate::db::ReviewExt;
use crate::AppState;
use crate::error::HttpError;
use crate::middleware::auth;
use crate::middleware::JWTAuthMiddleware;


pub fn review_handler(app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(get_reviews))
        .route("/", post(create_review)
            .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)))
        .route("/{review_id}", put(edit_review).delete(delete_review)
            .route_layer(middleware::from_fn_with_state(app_state, auth)))
}

pub async fn get_reviews(
    Query(params): Query<GetReviewsQuery>,
    Path(post_id): Path<i32>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);
    let sort = params.sort.unwrap_or("created_at_desc".to_string());

    let reviews = app_state.db_client
        .get_reviews(post_id, page, limit, &sort)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let total = app_state.db_client
        .get_post_review_count(post_id)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    let response = Json(ReviewListResponse{
        status: "success".to_string(),
        data: reviews,
        pagination: PaginationDto {
            page: page,
            limit: limit,
            total: total as i32,
            total_pages,
        }
    });

    Ok(response)
}

pub async fn create_review(                                                         
    Path(post_id): Path<i32>,                                                            
    State(app_state): State<AppState>,                                                   
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Json(body): Json<InputReviewRequest>,                                                
) -> Result<impl IntoResponse, HttpError> {                                              
    body.validate().map_err(|e| HttpError::bad_request(e.to_string()))?;                 
    let user_id = jwt.user.id;                                                           
    let review = app_state.db_client                                                     
        .create_review(user_id, post_id, &body.content)                                  
        .await                                                                           
        .map_err(|e| HttpError::server_error(e.to_string()))?;                                                                                                                 
    let response = Json(SingleReviewResponse {                                          
        status: "success".to_string(),                                                  
        data: review,                                                                  
        });                                                                                 
    Ok((StatusCode::CREATED, response))                                                
}

pub async fn edit_review(
    Path(review_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Json(body): Json<InputReviewRequest>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| HttpError::bad_request(e.to_string()))?;

    let user_id = jwt.user.id;

    let review = app_state.db_client
        .edit_review(user_id, review_id, &body.content)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let response = Json(SingleReviewResponse{
        status: "success".to_string(),
        data: review,
    });

    Ok(response)
}

async fn delete_review(
    Path(review_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
) -> Result<impl IntoResponse, HttpError> {
    let user_id = jwt.user.id;

    app_state.db_client
        .delete_review(user_id, review_id)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}