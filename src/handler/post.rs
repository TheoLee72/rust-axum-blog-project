use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::AppState;
use crate::db::PostExt;
use crate::dtos::{
    InputPostDto, Lang, LangQuery, PaginationDto, PostResponseDto, PostsPaginationResponseDto,
    PostsQueryParams, UploadResponse,
};
use crate::error::{ErrorMessage, HttpError};
use crate::handler::comment::comment_handler;
use crate::middleware::JWTAuthMiddleware;
use crate::middleware::{auth, role_check};
use crate::models::UserRole;
use axum::Extension;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post, put};
use axum::{Router, middleware};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

/// Router for blog post endpoints
///
/// **Middleware Execution Order:**
/// When multiple .route_layer() calls are used, they execute bottom-to-top.
/// In create_post: auth middleware runs first, then role_check.
/// This ensures user is authenticated before checking their role.
pub fn post_handler(app_state: AppState) -> Router<AppState> {
    Router::new()
        // GET /posts - List posts with pagination
        .route("/", get(get_posts))
        // POST /posts - Create new post (admin only, requires auth)
        .route(
            "/",
            post(create_post)
                .route_layer(middleware::from_fn(|req, next| {
                    role_check(req, next, vec![UserRole::Admin])
                }))
                .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)),
        )
        // GET /posts/{post_id} - Get single post
        .route("/{post_id}", get(get_post))
        // PUT /posts/{post_id} - Update post (admin only, requires auth)
        // DELETE /posts/{post_id} - Delete post (admin only, requires auth)
        .route(
            "/{post_id}",
            put(edit_post)
                .delete(delete_post)
                .route_layer(middleware::from_fn(|req, next| {
                    role_check(req, next, vec![UserRole::Admin])
                }))
                .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)),
        )
        // POST /posts/uploads - Upload image (admin only, requires auth)
        .route(
            "/uploads",
            post(upload_image)
                .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
                .route_layer(middleware::from_fn(|req, next| {
                    role_check(req, next, vec![UserRole::Admin])
                }))
                .route_layer(middleware::from_fn_with_state(app_state.clone(), auth)),
        )
        // Nest comments routes: /posts/{post_id}/comments/*
        .nest("/{post_id}/comments", comment_handler(app_state))
}

/// Get paginated list of posts
///
/// Defaults to posts from user "theolee72" if no username specified.
/// Query params: ?page=1&limit=10&user_username=username
#[instrument(skip(app_state))]
pub async fn get_posts(
    Query(params): Query<PostsQueryParams>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    params.validate().map_err(|e| {
        tracing::error!("Invalid get_posts input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);
    let username = params.user_username.unwrap_or("theolee72".to_string());
    let lang = params.lang.unwrap_or(Lang::En);

    // Fetch paginated posts
    let posts = app_state
        .db_client
        .get_posts(page, limit, &username, lang)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                tracing::warn!("No posts found for username: {}", username);
                HttpError::not_found("No posts found".to_string())
            }
            _ => {
                tracing::error!("DB error, getting posts: {}", e);
                HttpError::server_error(ErrorMessage::ServerError.to_string())
            }
        })?;

    // Get total post count for pagination metadata
    let total = app_state
        .db_client
        .get_user_post_count(&username)
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting user post count: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let total_pages = (total as f64 / limit as f64).ceil() as i32;

    let response = Json(PostsPaginationResponseDto {
        status: "success".to_string(),
        data: posts,
        pagination: Some(PaginationDto {
            page,
            limit,
            total: total as i32,
            total_pages,
        }),
    });
    tracing::info!("get_posts successful");
    Ok(response)
}

/// Get single post by ID
#[instrument(skip(app_state))]
pub async fn get_post(
    Path(post_id): Path<i32>,
    Query(q): Query<LangQuery>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    let lang = q.lang.unwrap_or(Lang::En);
    // Extract post_id from URL path
    let post = app_state
        .db_client
        .get_post(post_id, lang)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                tracing::warn!("Post with id {} not found", post_id);
                HttpError::not_found(format!("Post with id {} not found", post_id))
            }
            _ => {
                tracing::error!("DB error, getting post: {}", e);
                HttpError::server_error(ErrorMessage::ServerError.to_string())
            }
        })?;

    let response = Json(PostResponseDto {
        status: "success".to_string(),
        data: post,
    });
    tracing::info!("get_post successful");
    Ok(response)
}

/// Create new blog post
///
/// **Post Creation Process:**
/// 1. Validate and sanitize HTML content
/// 2. Extract plain text for full-text search
/// 3. Save post to database (with placeholder summary/embedding)
/// 4. Spawn background task to generate summary and embedding
///
/// **Why Background Task?**
/// Generating summary (LLM) and embedding (gRPC) are slow operations.
/// Returning response immediately improves user experience.
/// Background task updates database when complete.
#[instrument(skip(app_state, jwt, body))]
pub async fn create_post(
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Query(q): Query<LangQuery>,
    Json(body): Json<InputPostDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid create_post input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user_id = jwt.user.id;
    // Sanitize HTML content (remove dangerous tags/attributes)
    let content = secure_content(&body.content);
    let title = body.title;
    // Extract plain text from HTML (for full-text search)
    let raw_text = html2text::from_read(content.as_bytes(), 80).unwrap();

    // Placeholder values - will be updated by background task
    let summary_placeholder = "";
    let embedding_placeholder = vec![0.0; 768];
    let thumbnail_url = body.thumbnail_url;
    let lang = q.lang.unwrap_or(Lang::En);

    // Save post to database
    let result = app_state
        .db_client
        .create_post(
            user_id,
            &content,
            &title,
            &raw_text,
            summary_placeholder,
            embedding_placeholder,
            &thumbnail_url,
        )
        .await
        .map_err(|e| {
            tracing::error!("DB error, creating post: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let post_id = result.id;
    let app_state_clone = app_state.clone();
    let raw_text_clone = raw_text.clone();
    let title_clone = title.clone();

    // Spawn background task to generate summary and embedding
    // This runs concurrently and doesn't block the response
    tokio::spawn(async move {
        // Get summary from LLM service
        let summary = app_state_clone
            .http_client
            .get_summary(
                &app_state_clone.env.llm_url,
                &app_state_clone.env.model_name,
                &raw_text_clone,
                lang.clone(),
            )
            .await;

        // Get embedding from gRPC service
        let embedding = app_state_clone
            .grpc_client
            .get_embedding_docs(&raw_text_clone, &title_clone)
            .await;

        // Update post with summary and embedding if both succeeded
        if let (Ok(summary), Ok(embedding)) = (summary, embedding) {
            if let Err(e) = app_state_clone
                .db_client
                .update_post_summary_and_embedding(post_id, &summary, embedding, lang)
                .await
            {
                tracing::error!("Failed to update post with summary and embedding: {}", e);
            }
        }
    });

    let response = Json(PostResponseDto {
        status: "success".to_string(),
        data: result,
    });
    tracing::info!("create_post successful");
    Ok((StatusCode::CREATED, response))
}

/// Edit existing post
///
/// Updates content, title, and plain text.
/// Spawns background task to regenerate summary and embedding.
#[instrument(skip(app_state, jwt, body))]
pub async fn edit_post(
    Path(post_id): Path<i32>,
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Query(q): Query<LangQuery>,
    Json(body): Json<InputPostDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid edit_post input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user_id = jwt.user.id;
    let content = secure_content(&body.content);
    let title = body.title;
    let raw_text = html2text::from_read(content.as_bytes(), 80).unwrap();
    let thumbnail_url = body.thumbnail_url;
    let lang = q.lang.unwrap_or(Lang::En);

    // Update post in database
    let result = app_state
        .db_client
        .edit_post(
            user_id,
            post_id,
            &content,
            &title,
            &raw_text,
            &thumbnail_url,
            lang.clone(),
        )
        .await
        .map_err(|e| {
            tracing::error!("DB error, editing post: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Spawn background task to regenerate summary and embedding
    tokio::spawn(async move {
        let summary = app_state
            .http_client
            .get_summary(
                &app_state.env.llm_url,
                &app_state.env.model_name,
                &raw_text,
                lang.clone(),
            )
            .await;

        let embedding = app_state
            .grpc_client
            .get_embedding_docs(&raw_text, &title)
            .await;

        if let (Ok(summary), Ok(embedding)) = (summary, embedding) {
            if let Err(e) = app_state
                .db_client
                .update_post_summary_and_embedding(post_id, &summary, embedding, lang)
                .await
            {
                tracing::error!("Failed to update post with summary and embedding: {}", e);
            }
        } else {
            tracing::error!("Failed to get summary or embedding");
        }
    });

    let response = Json(PostResponseDto {
        status: "success".to_string(),
        data: result,
    });
    tracing::info!("edit_post successful");
    Ok(response)
}

/// Delete post
#[instrument(skip(app_state, jwt))]
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
        .map_err(|e| {
            tracing::error!("DB error, deleting post: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;
    tracing::info!("delete_post successful");
    Ok(StatusCode::NO_CONTENT)
}

/// Upload image for blog post
///
/// **Security Checks:**
/// 1. Filename sanitization (remove dangerous characters)
/// 2. Content-type validation (only images allowed)
/// 3. File size limit (10MB max)
/// 4. File extension validation
/// 5. Magic bytes verification (check actual file content)
///
/// **Storage:**
/// - Files saved to /opt/blog_backend_axum/uploads/
/// - Filename randomized (UUID) to prevent collisions
/// - Served via Nginx at https://theolee.net/static/uploads/
#[instrument(skip(multipart))]
pub async fn upload_image(mut multipart: Multipart) -> Result<impl IntoResponse, HttpError> {
    // Create upload directory if it doesn't exist
    let upload_dir = PathBuf::from("./uploads");
    fs::create_dir_all(&upload_dir).map_err(|e| {
        tracing::error!("Failed to create upload directory: {}", e);
        HttpError::server_error(format!("Failed to create upload directory: {}", e))
    })?;

    if let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Invalid multipart data: {}", e);
        HttpError::bad_request(format!("Invalid multipart data: {}", e))
    })? {
        let file_name = field
            .file_name()
            .ok_or_else(|| HttpError::bad_request("Missing filename"))?
            .to_string();

        // Sanitize filename - keep only safe characters
        let safe_filename: String = file_name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .collect();

        if safe_filename.is_empty() {
            tracing::error!("Invalid filename: {}", file_name);
            return Err(HttpError::bad_request("Invalid filename"));
        }

        // Check content-type header
        let content_type = field
            .content_type()
            .ok_or_else(|| HttpError::bad_request("Missing content type"))?
            .to_string();

        if !["image/jpeg", "image/png", "image/gif", "image/webp"].contains(&content_type.as_str())
        {
            tracing::error!("Invalid file type: {}", content_type);
            return Err(HttpError::bad_request("Invalid file type"));
        }

        // Read file bytes
        let bytes = field.bytes().await.map_err(|e| {
            tracing::error!("Failed to read file: {}", e);
            HttpError::bad_request(format!("Failed to read file: {}", e))
        })?;

        if bytes.is_empty() {
            tracing::error!("Empty file uploaded");
            return Err(HttpError::bad_request("Empty file"));
        }

        // Check file size (max 10MB)
        const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

        if bytes.len() > MAX_FILE_SIZE {
            tracing::error!("File too large: {} bytes", bytes.len());
            return Err(HttpError::bad_request(format!(
                "File too large. Max size: {}MB",
                MAX_FILE_SIZE / 1024 / 1024
            )));
        }

        // Extract and validate file extension
        let ext = safe_filename
            .rsplit('.')
            .next()
            .unwrap_or("bin")
            .to_lowercase();

        if !["jpg", "jpeg", "png", "gif", "webp"].contains(&ext.as_str()) {
            tracing::error!("File extension not allowed: {}", ext);
            return Err(HttpError::bad_request("File extension not allowed"));
        }

        // Verify file magic bytes (prevent uploading disguised files)
        if !verify_image_signature(&bytes, &ext) {
            tracing::error!("File content does not match extension: {}", ext);
            return Err(HttpError::bad_request(
                "File content does not match extension",
            ));
        }

        // Save file with randomized name
        let new_name = format!("{}.{}", Uuid::new_v4(), ext);

        let mut path = upload_dir;
        path.push(&new_name);

        let mut file = fs::File::create(&path).map_err(|e| {
            tracing::error!("Failed to create file: {}", e);
            HttpError::server_error(format!("Failed to create file: {}", e))
        })?;
        file.write_all(&bytes).map_err(|e| {
            tracing::error!("Failed to write to file: {}", e);
            HttpError::server_error(format!("Failed to write to file: {}", e))
        })?;

        // Return public URL (served by Nginx)
        let public_url = format!("https://theolee.net/static/uploads/{}", new_name);
        tracing::info!("Image uploaded successfully: {}", public_url);
        Ok(Json(UploadResponse {
            location: public_url,
        }))
    } else {
        tracing::error!("No file uploaded");
        Err(HttpError::bad_request("No file uploaded"))
    }
}

/// Verify file magic bytes to prevent uploading disguised files
///
/// Example: A file named "image.png" but actually contains executable code
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

/// Sanitize HTML content to prevent XSS attacks
///
/// Removes dangerous tags and only allows safe HTML attributes/CSS properties.
/// Whitelist approach: Only allows known-safe styles.
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
        "max-width",
        "display",
        "margin-left",
        "margin-right",
    ]);
    let secure_content = ammonia::Builder::default()
        .generic_attributes(HashSet::from(["style", "class"]))
        .filter_style_properties(properties)
        .clean(content)
        .to_string();
    secure_content
}
