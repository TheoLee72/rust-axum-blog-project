use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::AppState;
use crate::db::PostExt;
use crate::dtos::{
    InputPostDto, PaginationDto, PostResponseDto, PostsPaginationResponseDto, PostsQueryParams,
    UploadResponse,
};
use crate::error::HttpError;
use crate::handler::comment::comment_handler;
use crate::middleware::JWTAuthMiddleware;
use crate::middleware::{auth, role_check};
use crate::models::UserRole;
use axum::Extension;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post, put};
use axum::{Router, middleware};
use uuid::Uuid;
use validator::Validate;

pub fn post_handler(app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(get_posts))
        .route(
            "/",
            post(create_post)
                .route_layer(middleware::from_fn(|req, next| {
                    role_check(req, next, vec![UserRole::Admin])
                }))
                .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)), //뒤에 오는게 더 먼저 실행되나봄. 위에 layer를 쌓는다고 생각해야하나?
        )
        .route("/{post_id}", get(get_post))
        .route(
            "/{post_id}",
            put(edit_post)
                .delete(delete_post)
                .route_layer(middleware::from_fn(|req, next| {
                    role_check(req, next, vec![UserRole::Admin])
                }))
                .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)),
        )
        .route(
            "/uploads",
            post(upload_image)
                .route_layer(middleware::from_fn(|req, next| {
                    role_check(req, next, vec![UserRole::Admin])
                }))
                .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)),
        )
        .nest("/{post_id}/comments", comment_handler(app_state))
}

pub async fn get_posts(
    Query(params): Query<PostsQueryParams>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params
        .validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);
    let username = params.user_username.unwrap_or("theolee72".to_string());

    let posts = app_state
        .db_client
        .get_posts(page, limit, &username)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => HttpError::not_found("No posts found".to_string()),
            _ => HttpError::server_error(e.to_string()),
        })?;

    let total = app_state
        .db_client
        .get_user_post_count(&username)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    let response = Json(PostsPaginationResponseDto {
        status: "success".to_string(),
        data: posts,
        pagination: Some(PaginationDto {
            page: page,
            limit: limit,
            total: total as i32,
            total_pages,
        }),
    });

    Ok(response)
}
pub async fn get_post(
    Path(post_id): Path<i32>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    //path param으로 post_id뽑기
    //이것도 바로 가져오면 됨.
    let post = app_state
        .db_client
        .get_post(post_id)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                HttpError::not_found(format!("Post with id {} not found", post_id))
            }
            _ => HttpError::server_error(e.to_string()),
        })?;

    let response = Json(PostResponseDto {
        status: "success".to_string(),
        data: post,
    });

    Ok(response)
}
pub async fn create_post(
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Json(body): Json<InputPostDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let user_id = jwt.user.id;
    let content = secure_content(&body.content);
    let title = body.title;
    let raw_text = html2text::from_read(content.as_bytes(), 80).unwrap();

    // Placeholder values
    let summary_placeholder = "";
    let embedding_placeholder = vec![0.0; 768];

    let result = app_state
        .db_client
        .create_post(
            user_id,
            &content,
            &title,
            &raw_text,
            summary_placeholder,
            embedding_placeholder,
        )
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let post_id = result.id;
    let app_state_clone = app_state.clone();
    let raw_text_clone = raw_text.clone();
    let title_clone = title.clone();

    tokio::spawn(async move {
        let summary = app_state_clone
            .http_client
            .get_summary(
                &app_state_clone.env.llm_url,
                &app_state_clone.env.model_name,
                &raw_text_clone,
            )
            .await;

        let embedding = app_state_clone
            .grpc_client
            .get_embedding_docs(&raw_text_clone, &title_clone)
            .await;

        if let (Ok(summary), Ok(embedding)) = (summary, embedding) {
            if let Err(e) = app_state_clone
                .db_client
                .update_post_summary_and_embedding(post_id, &summary, embedding)
                .await
            {
                eprintln!("Failed to update post with summary and embedding: {}", e);
            }
        }
    });

    let response = Json(PostResponseDto {
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
    let content = secure_content(&body.content);
    let title = body.title;
    let raw_text = html2text::from_read(content.as_bytes(), 80).unwrap();

    let result = app_state
        .db_client
        .edit_post(user_id, post_id, &content, &title, &raw_text)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    tokio::spawn(async move {
        let summary = app_state
            .http_client
            .get_summary(&app_state.env.llm_url, &app_state.env.model_name, &raw_text)
            .await;

        let embedding = app_state
            .grpc_client
            .get_embedding_docs(&raw_text, &title)
            .await;

        if let (Ok(summary), Ok(embedding)) = (summary, embedding) {
            let _ = app_state
                .db_client
                .update_post_summary_and_embedding(post_id, &summary, embedding)
                .await;
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

    app_state
        .db_client
        .delete_post(user_id, post_id)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn upload_image(mut multipart: Multipart) -> Result<impl IntoResponse, HttpError> {
    // 업로드 저장 경로
    let upload_dir = PathBuf::from("/opt/blog_backend_axum/uploads");
    fs::create_dir_all(&upload_dir).map_err(|e| {
        HttpError::server_error(format!("Failed to create upload directory: {}", e))
    })?;

    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| HttpError::bad_request(format!("Invalid multipart data: {}", e)))?
    {
        let file_name = field
            .file_name()
            .ok_or_else(|| HttpError::bad_request("Missing filename"))?
            .to_string();

        // 파일명에서 위험한 문자 제거
        let safe_filename: String = file_name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .collect();

        if safe_filename.is_empty() {
            return Err(HttpError::bad_request("Invalid filename"));
        }

        let content_type = field
            .content_type()
            .ok_or_else(|| HttpError::bad_request("Missing content type"))?
            .to_string();

        if !["image/jpeg", "image/png", "image/gif", "image/webp"].contains(&content_type.as_str())
        {
            return Err(HttpError::bad_request("Invalid file type"));
        }

        let bytes = field
            .bytes()
            .await
            .map_err(|e| HttpError::bad_request(format!("Failed to read file: {}", e)))?;

        if bytes.is_empty() {
            return Err(HttpError::bad_request("Empty file"));
        }

        const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

        if bytes.len() > MAX_FILE_SIZE {
            return Err(HttpError::bad_request(format!(
                "File too large. Max size: {}MB",
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }

        // 확장자 추출
        let ext = safe_filename
            .rsplit('.')
            .next()
            .unwrap_or("bin")
            .to_lowercase();

        if !["jpg", "jpeg", "png", "gif", "webp"].contains(&ext.as_str()) {
            return Err(HttpError::bad_request("File extension not allowed"));
        }

        // 파일 시그니처 검증 (magic bytes)
        if !verify_image_signature(&bytes, &ext) {
            return Err(HttpError::bad_request(
                "File content does not match extension",
            ));
        }

        let new_name = format!("{}.{}", Uuid::new_v4(), ext);

        let mut path = upload_dir;
        path.push(&new_name);

        let mut file = fs::File::create(&path)
            .map_err(|e| HttpError::server_error(format!("Failed to create file: {}", e)))?;
        file.write_all(&bytes)
            .map_err(|e| HttpError::server_error(format!("Failed to write to file: {}", e)))?;

        // Nginx에서 /static/uploads/ 로 매핑했다고 가정
        let public_url = format!("https://theolee.net/static/uploads/{}", new_name);

        Ok(Json(UploadResponse {
            location: public_url,
        }))
    } else {
        Err(HttpError::bad_request("No file uploaded"))
    }
}

fn verify_image_signature(bytes: &[u8], ext: &str) -> bool {
    if bytes.len() < 4 {
        return false;
    }

    match ext {
        "jpg" | "jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    }
}

fn secure_content(content: &str) -> String {
    let properties = HashSet::from([
        "border-collapse",
        "width",
        "border",
        "border-width",
        "border-style",
        "border-color",
        "background-color",
        "padding",
        "padding-left",
        "padding-right",
        "padding-top",
        "padding-bottom",
        "text-align",
        "vertical-align",
        "height",
        "color",
        "font-weight",
        "font-size",
    ]);
    let secure_content = ammonia::Builder::default()
        .generic_attributes(HashSet::from(["style", "class"]))
        .filter_style_properties(properties)
        .clean(content)
        .to_string();
    secure_content
}
