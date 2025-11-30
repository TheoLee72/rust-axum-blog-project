use crate::db::CommentExt;
use crate::dtos::{EmailUpdateDto, UserMeData};
use crate::mail::mails::send_verification_email_newemail;
use crate::{
    AppState,
    db::PostExt,
    db::UserExt,
    dtos::{
        DoubleCheckDto, FilterUserDto, NameUpdateDto, RequestQueryDto, Response, RoleUpdateDto,
        UserData, UserListResponseDto, UserMeResponseDto, UserPasswordUpdateDto, UserResponseDto,
    },
    error::{ErrorMessage, HttpError},
    middleware::{JWTAuthMiddleware, role_check},
    models::UserRole,
    utils::password,
};
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use axum_extra::extract::cookie::Cookie;
use chrono::{Duration, Utc};
use tracing::instrument;
use validator::Validate;

/// Router for user management endpoints
///
/// All routes are protected by the auth middleware (applied in routes.rs).
/// Some routes have additional role-based restrictions.
pub fn users_handler() -> Router<AppState> {
    Router::new()
        // GET /me - Get current user's profile with statistics
        // Accessible by both Admin and User roles
        .route(
            "/me",
            get(get_me).layer(middleware::from_fn(|req, next| {
                role_check(req, next, vec![UserRole::Admin, UserRole::User])
            })),
        )
        // GET /users - List all users (admin only)
        .route(
            "/users",
            get(get_users).layer(middleware::from_fn(|req, next| {
                role_check(req, next, vec![UserRole::Admin])
            })),
        )
        // PUT /username - Update user's display name
        .route("/username", put(update_user_name))
        // PUT /role - Update user's role (commented out - requires admin)
        //.route("/role", put(update_user_role))
        // PUT /password - Change password (requires old password)
        .route("/password", put(update_user_password))
        // PUT /email - Change email address (sends verification)
        .route("/email", put(update_user_email))
        // POST /logout - Logout user (clears tokens)
        .route("/logout", post(logout))
        // DELETE /delete-me - Delete user account (requires password confirmation)
        .route("/delete-me", delete(delete_me))
}

/// Get current user's profile with post and comment counts
///
/// Returns the authenticated user's information plus statistics.
#[instrument(skip(user, app_state), fields(username = %user.user.username))]
pub async fn get_me(
    Extension(user): Extension<JWTAuthMiddleware>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    // Filter user data (remove sensitive fields like password hash)
    let filtered_user = FilterUserDto::filter_user(&user.user);

    // Get user's post count
    let post_count = app_state
        .db_client
        .get_user_post_count(&user.user.username)
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting user post count: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Get user's comment count
    let comment_count = app_state
        .db_client
        .get_user_comment_count(&user.user.id)
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting user comment count: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let response_data = UserMeResponseDto {
        status: "success".to_string(),
        data: UserMeData {
            user: filtered_user,
            post_count,
            comment_count,
        },
    };
    tracing::info!("get_me successful");
    Ok(Json(response_data))
}

/// Get paginated list of all users (admin only)
///
/// Query params: ?page=1&limit=10
#[instrument(skip(app_state))]
pub async fn get_users(
    Query(query_params): Query<RequestQueryDto>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate query parameters (page >= 1, limit between 1-50)
    query_params.validate().map_err(|e| {
        tracing::error!("Invalid get_users input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let page = query_params.page.unwrap_or(1);
    let limit = query_params.limit.unwrap_or(10);

    // Fetch users from database with pagination
    let users = app_state
        .db_client
        .get_users(page as u32, limit)
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting users: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Get total user count for pagination metadata
    let user_count = app_state.db_client.get_user_count().await.map_err(|e| {
        tracing::error!("DB error, getting user count: {}", e);
        HttpError::server_error(ErrorMessage::ServerError.to_string())
    })?;

    let response = UserListResponseDto {
        status: "success".to_string(),
        users: FilterUserDto::filter_users(&users),
        results: user_count,
    };
    tracing::info!("get_users successful");
    Ok(Json(response))
}

/// Update user's display name
#[instrument(skip(app_state, user, body), fields(username = %user.user.username))]
pub async fn update_user_name(
    State(app_state): State<AppState>,
    Extension(user): Extension<JWTAuthMiddleware>,
    Json(body): Json<NameUpdateDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate input (name must not be empty)
    body.validate().map_err(|e| {
        tracing::error!("Invalid update_user_name input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user = &user.user;
    let user_id = uuid::Uuid::parse_str(&user.id.to_string()).unwrap();

    // Update name in database
    let result = app_state
        .db_client
        .update_user_name(user_id.clone(), &body.name)
        .await
        .map_err(|e| {
            tracing::error!("DB error, updating user name: {}", e);
            // Postgres unique violation has SQLSTATE code 23505
            if let sqlx::Error::Database(ref db_err) = e {
                if let Some(code) = db_err.code() {
                    if code == "23505" {
                        return HttpError::new(
                            "Username already exists".to_string(),
                            StatusCode::BAD_REQUEST,
                        );
                    }
                }
            }
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let filtered_user = FilterUserDto::filter_user(&result);

    let response = UserResponseDto {
        data: UserData {
            user: filtered_user,
        },
        status: "success".to_string(),
    };
    tracing::info!("update_user_name successful");
    Ok(Json(response))
}

/// Update user's role (admin only - currently disabled)
///
/// This endpoint is commented out in the router but included for reference.
#[instrument(skip(app_state, user, body), fields(username = %user.user.username))]
pub async fn update_user_role(
    State(app_state): State<AppState>,
    Extension(user): Extension<JWTAuthMiddleware>,
    Json(body): Json<RoleUpdateDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid update_user_role input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user = &user.user;
    let user_id = uuid::Uuid::parse_str(&user.id.to_string()).unwrap();

    let result = app_state
        .db_client
        .update_user_role(user_id.clone(), body.role)
        .await
        .map_err(|e| {
            tracing::error!("DB error, updating user role: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let filtered_user = FilterUserDto::filter_user(&result);

    let response = UserResponseDto {
        data: UserData {
            user: filtered_user,
        },
        status: "success".to_string(),
    };
    tracing::info!("update_user_role successful");
    Ok(Json(response))
}

/// Update user's password
///
/// Requires old password verification before allowing change.
/// Request body: { old_password, new_password, new_password_confirm }
#[instrument(skip(app_state, user, body), fields(username = %user.user.username))]
pub async fn update_user_password(
    State(app_state): State<AppState>,
    Extension(user): Extension<JWTAuthMiddleware>,
    Json(body): Json<UserPasswordUpdateDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate input (password length, confirmation match)
    body.validate().map_err(|e| {
        tracing::error!("Invalid update_user_password input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user = &user.user;
    let user_id = uuid::Uuid::parse_str(&user.id.to_string()).unwrap();

    // Fetch current user to verify old password
    let result = app_state
        .db_client
        .get_user(Some(user_id.clone()), None, None, None)
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting user: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let user = result.ok_or_else(|| {
        tracing::error!("User not found");
        HttpError::unauthorized(ErrorMessage::InvalidToken.to_string())
    })?;

    // Verify old password matches
    let password_match = password::compare(&body.old_password, &user.password).map_err(|e| {
        tracing::error!("Password comparison error: {}", e);
        HttpError::server_error(ErrorMessage::ServerError.to_string())
    })?;

    if !password_match {
        tracing::error!("Old password is incorrect");
        return Err(HttpError::bad_request(
            "Old password is incorrect".to_string(),
        ));
    }

    // Hash new password
    let hash_password = password::hash(&body.new_password).map_err(|e| {
        tracing::error!("Password hashing error: {}", e);
        HttpError::server_error(ErrorMessage::ServerError.to_string())
    })?;

    // Update password in database
    app_state
        .db_client
        .update_user_password(user_id.clone(), hash_password)
        .await
        .map_err(|e| {
            tracing::error!("DB error, updating user password: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;
    // Force logout everywhere
    app_state
        .redis_client
        .delete_refresh_token(&user_id.to_string())
        .await
        .map_err(|e| {
            tracing::error!("RedisDB error, deleting refresh token: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let response = Response {
        message: "Password updated Successfully".to_string(),
        status: "success",
    };
    tracing::info!("update_user_password successful");
    Ok(Json(response))
}

/// Update user's email address
///
/// Sends verification email to new address. Email is not changed until verified.
/// Request body: { email }
#[instrument(skip(user, app_state), fields(username = %user.user.username))]
pub async fn update_user_email(
    Extension(user): Extension<JWTAuthMiddleware>,
    State(app_state): State<AppState>,
    Json(body): Json<EmailUpdateDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate email format
    body.validate().map_err(|e| {
        tracing::error!("Invalid update_user_email input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    // Create verification token: UUID + new email
    let email_token = format!("{}+{}", uuid::Uuid::new_v4(), &body.email);
    let expires_at = Utc::now() + Duration::hours(24);
    let user_id = user.user.id;

    // Check if email is already in use by another user
    app_state
        .db_client
        .check_email_duplicate(user_id, &body.email)
        .await
        .map_err(|e| {
            tracing::error!("DB error, checking email duplicate: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Store verification token in database
    app_state
        .db_client
        .add_verifed_token(user_id, &email_token, expires_at)
        .await
        .map_err(|e| {
            tracing::error!("DB error, adding verified token: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Send verification email to new address
    send_verification_email_newemail(
        &body.email,
        &user.user.username,
        &email_token,
        &app_state.env.frontend_url,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to send verification email: {}", e);
        HttpError::server_error(ErrorMessage::ServerError.to_string())
    })?;

    let response = Response {
        message: "Please verify your email".to_string(),
        status: "success",
    };
    tracing::info!("update_user_email successful");
    Ok(Json(response))
}

/// Logout user by clearing tokens
///
/// Deletes refresh token from Redis and sets cookies to expire immediately.
#[instrument(skip(user, app_state), fields(username = %user.user.username))]
pub async fn logout(
    Extension(user): Extension<JWTAuthMiddleware>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    let user = user.user;

    // Delete refresh token from Redis
    app_state
        .redis_client
        .delete_refresh_token(&user.id.to_string())
        .await
        .map_err(|e| {
            tracing::error!("RedisDB error, deleting refresh token: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Create expired cookies to clear client-side tokens
    let access_cookie = Cookie::build(("access_token", ""))
        .path("/")
        .max_age(time::Duration::ZERO) // Expire immediately
        .http_only(true)
        .build();

    let refresh_cookie = Cookie::build(("refresh_token", ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .http_only(true)
        .build();

    // Build response with Set-Cookie headers
    let mut headers = HeaderMap::new();
    headers.append(
        header::SET_COOKIE,
        access_cookie.to_string().parse().unwrap(),
    );
    headers.append(
        header::SET_COOKIE,
        refresh_cookie.to_string().parse().unwrap(),
    );

    let json_response = axum::response::Json(Response {
        status: "success",
        message: "Logout successful".to_string(),
    });

    let mut response = json_response.into_response();
    response.headers_mut().extend(headers);
    tracing::info!("logout successful");
    Ok(response)
}

/// Delete user account
///
/// Requires password confirmation. Permanently deletes the user and all associated data.
/// Request body: { password }
#[instrument(skip(app_state, jwt, body), fields(username = %jwt.user.username))]
pub async fn delete_me(
    State(app_state): State<AppState>,
    Extension(jwt): Extension<JWTAuthMiddleware>,
    Json(body): Json<DoubleCheckDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate password input
    body.validate().map_err(|e| {
        tracing::error!("Invalid delete_me input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    let user = jwt.user;

    // Verify password before allowing deletion
    let passwords_match = password::compare(&body.password, &user.password).map_err(|e| {
        tracing::error!("Password comparison error: {}", e);
        HttpError::server_error("Error while comparing passwords".to_string())
    })?;

    if passwords_match {
        // Delete user from database
        app_state
            .db_client
            .delete_user(user.id)
            .await
            .map_err(|e| {
                if let sqlx::Error::RowNotFound = e {
                    tracing::error!("User not found for deletion");
                    HttpError::new("User not found".to_string(), StatusCode::NOT_FOUND)
                } else {
                    tracing::error!("DB error, deleting user: {}", e);
                    HttpError::server_error(ErrorMessage::ServerError.to_string())
                }
            })?;

        // Return 204 No Content on successful deletion
        tracing::info!("delete_me successful");
        Ok(StatusCode::NO_CONTENT)
    } else {
        // Password verification failed
        tracing::error!("Invalid password for delete_me");
        Err(HttpError::unauthorized("Invalid password".to_string()))
    }
}
