use axum::http::StatusCode;
use axum::{middleware, Router};
use axum::routing::{get, post, put};
use axum::response::{IntoResponse, Json};
use axum::extract::{Path, Query, State};
use axum::Extension;
use validator::Validate;
use crate::dtos::{InputPostDto, PaginationDto, PostResponseDto, PostsPaginationResponseDto, PostsQueryParams};
use crate::db::PostExt;
use crate::AppState;
use crate::error::HttpError;
use crate::middleware::{auth, role_check};
use crate::models::UserRole;
use crate::middleware::JWTAuthMiddleware;
use crate::handler::review::review_handler;


pub fn post_handler(app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(get_posts))
        .route("/", post(create_post)
            .route_layer(middleware::from_fn(|req, next| {role_check(req, next, vec![UserRole::Admin])}))
            .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)) //뒤에 오는게 더 먼저 실행되나봄. 위에 layer를 쌓는다고 생각해야하나?
        )
        .route("/{post_id}", get(get_post))
        .route("/{post_id}", put(edit_post).delete(delete_post)
            .route_layer(middleware::from_fn(|req, next| {role_check(req, next, vec![UserRole::Admin])}))
            .route_layer(middleware::from_fn_with_state(app_state.clone(), auth))
        )
        .nest("/{post_id}/reviews", review_handler(app_state))
}


pub async fn get_posts(
    Query(params): Query<PostsQueryParams>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);
    let username = params.user_username.unwrap_or("theolee72".to_string());

    let posts = app_state.db_client
        .get_posts(page, limit, &username)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let total = app_state.db_client
        .get_user_post_count(&username)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    let response = Json(PostsPaginationResponseDto{
        status: "success".to_string(),
        data: posts,
        pagination: Some(PaginationDto {
            page: page,
            limit: limit,
            total: total as i32,
            total_pages,
        })
    });

    Ok(response)
}
pub async fn get_post(
    Path(post_id) : Path<i32>,
    State(app_state) : State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    //path param으로 post_id뽑기
    //이것도 바로 가져오면 됨.
    let post = app_state.db_client
        .get_post(post_id)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let response = Json(post);

    Ok(response)
}
pub async fn create_post(
    State(app_state): State<AppState>,
    Extension(jwt) : Extension<JWTAuthMiddleware>,
    Json(body): Json<InputPostDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let user_id = jwt.user.id;
    let content = body.content;
    let title = body.title;
    let raw_text =  html2text::from_read(content.as_bytes(), 80).unwrap();

    // Placeholder values
    let summary_placeholder = "";
    let embedding_placeholder = vec![0.0; 768];

    let result = app_state.db_client.create_post(user_id, &content, &title, &raw_text, summary_placeholder, embedding_placeholder).await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let post_id = result.id;
    let app_state_clone = app_state.clone();
    let raw_text_clone = raw_text.clone();
    let title_clone = title.clone();

    tokio::spawn(async move {
        let summary = app_state_clone.http_client
            .get_summary(&app_state_clone.env.llm_url, &app_state_clone.env.model_name, &raw_text_clone)
            .await;
        
        let embedding = app_state_clone.grpc_client.get_embedding_docs(&raw_text_clone, &title_clone)
            .await;

        if let (Ok(summary), Ok(embedding)) = (summary, embedding) {
            if let Err(e) = app_state_clone.db_client.update_post_summary_and_embedding(post_id, &summary, embedding).await {
                eprintln!("Failed to update post with summary and embedding: {}", e);
            }
        }
    });

    let response = Json(PostResponseDto{
        status: "success".to_string(),
        data: result,
    });

    Ok((StatusCode::CREATED, response))
}
pub async fn edit_post(
    Path(post_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Json(body): Json<InputPostDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let user_id = jwt.user.id;
    let content = body.content;
    let title = body.title;
    let raw_text =  html2text::from_read(content.as_bytes(), 80).unwrap();

    let result = app_state.db_client.edit_post(user_id, post_id, &content, &title, &raw_text).await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    tokio::spawn(async move {
        let summary = app_state.http_client
            .get_summary(&app_state.env.llm_url, &app_state.env.model_name, &raw_text)
            .await;
        
        let embedding = app_state.grpc_client.get_embedding_docs(&raw_text, &title)
            .await;

        if let (Ok(summary), Ok(embedding)) = (summary, embedding) {
            let _ = app_state.db_client.update_post_summary_and_embedding(post_id, &summary, embedding).await;
        }
    });

    let response = Json(PostResponseDto {
        status: "success".to_string(),
        data: result,
    });

    Ok(response)
}
pub async fn delete_post(
    Path(post_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
) -> Result<impl IntoResponse, HttpError> {
    let user_id = jwt.user.id;

    app_state.db_client.delete_post(user_id, post_id).await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}