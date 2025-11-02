use crate::{
    AppState,
    db::UserExt,
    dtos::{
        ForgotPasswordRequestDto, LoginUserDto, RefreshResponseDto, RegisterUserDto,
        ResetPasswordRequestDto, Response, UserLoginResponseDto, VerifyEmailQueryDto,
    },
    error::{ErrorMessage, HttpError},
    mail::mails::{send_forgot_password_email, send_verification_email, send_welcome_email},
    utils::{password, token},
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::{Duration, Utc};
use validator::Validate;

use axum_client_ip::ClientIp;

use tracing::instrument;

/// Router for authentication endpoints
pub fn auth_handler(app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route(
            "/login",
            post(login).layer(app_state.ip_extraction.into_extension()),
        )
        .route("/verify", get(verify_email))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
        .route("/refresh", post(refresh))
}

/// Register new user account
/// Creates user, hashes password, sends verification email
/// The #[instrument] macro automatically:
/// - Creates a span with function name "register"
/// - Captures all function arguments as span fields
/// - Skips State and Extension (marked with skip)
/// - Works seamlessly with async
#[instrument(skip(app_state, body), fields(username = %body.username, email = %body.email))]
pub async fn register(
    State(app_state): State<AppState>,
    Json(body): Json<RegisterUserDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate input
    body.validate().map_err(|e| {
        tracing::error!("Invalid register input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    // Create verification token valid for 24 hours
    let verification_token = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(24);

    // Hash password before storing
    let hash_password = password::hash(&body.password).map_err(|e| {
        tracing::error!("Password hashing error: {}", e);
        HttpError::server_error(e.to_string())
    })?;

    // Save user to database with verification token
    let result = app_state
        .db_client
        .save_user(
            &body.username,
            &body.email,
            &hash_password,
            &verification_token,
            expires_at,
        )
        .await;

    match result {
        Ok(_user) => {
            // Send verification email (don't block if email fails)
            let send_email_result = send_verification_email(
                &body.email,
                &body.username,
                &verification_token,
                &app_state.env.frontend_url,
            )
            .await;

            if let Err(e) = send_email_result {
                tracing::error!("Failed to send verification email: {}", e);
            }

            tracing::info!(username = %body.username, email = %body.email, "Register Successful");
            Ok((
                StatusCode::CREATED,
                Json(Response {
                    status: "success",
                    message:
                        "Registration successful! Please check your email to verify your account."
                            .to_string(),
                }),
            ))
        }
        Err(sqlx::Error::Database(db_err)) => {
            // Email or username already exists
            if db_err.is_unique_violation() {
                tracing::error!("DB error, saving user, unique_violation: {}", db_err);
                Err(HttpError::unique_constraint_violation(db_err.to_string()))
            } else {
                tracing::error!("DB error, saving user: {}", db_err);
                Err(HttpError::server_error(
                    ErrorMessage::ServerError.to_string(),
                ))
            }
        }
        Err(e) => {
            tracing::error!("DB error, saving user: {}", e);
            Err(HttpError::server_error(
                ErrorMessage::ServerError.to_string(),
            ))
        }
    }
}

/// Login with rate limiting (100 attempts per IP per day, 10 per identifier per hour)
#[instrument(skip(app_state, body), fields(identifier = %body.identifier))]
pub async fn login(
    ClientIp(ip): ClientIp,
    State(app_state): State<AppState>,
    Json(body): Json<LoginUserDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Check IP attempt limit (max 100 per 24 hours)
    let ip_attempts = app_state
        .redis_client
        .get_ip_attempts(ip)
        .await
        .map_err(|e| {
            tracing::error!("RedisDB error, getting ip attempts: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?
        .unwrap_or(0);
    if ip_attempts >= 100 {
        tracing::error!("Login attempt exceeded the limit");
        return Err(HttpError::server_error("Login failed"));
    }

    // Check identifier+IP attempt limit (max 10 per hour)
    let identifier_ip_attempts = app_state
        .redis_client
        .get_identifier_ip_attempts(ip, &body.identifier)
        .await
        .map_err(|e| {
            tracing::error!("RedisDB error, getting identifier+ip attempts: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?
        .unwrap_or(0);

    if identifier_ip_attempts >= 10 {
        tracing::error!("Login attempt exceeded the limit");
        return Err(HttpError::server_error("Login failed"));
    }

    // Attempt authentication
    match authenticate_process(State(app_state.clone()), &body).await {
        Ok(response) => {
            // Clear rate limit on success
            if let Err(e) = app_state
                .redis_client
                .delete_identifier_ip_attempts(ip, &body.identifier)
                .await
            {
                tracing::warn!("Failed to clear rate limit: {:?}", e);
            }
            tracing::info!(identifier = %body.identifier, ip = %ip, "Login Successful");
            Ok(response)
        }
        Err(_) => {
            // Increment rate limit on failure
            if let Err(e) = app_state
                .redis_client
                .increment_attempts(ip, &body.identifier)
                .await
            {
                tracing::warn!("Failed to increment the rate {:?}", e);
            }
            Err(HttpError::server_error("Login failed"))
        }
    }
}

/// Authenticate user credentials
async fn authenticate_process(
    State(app_state): State<AppState>,
    body: &LoginUserDto,
) -> Result<impl IntoResponse + use<>, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid login input: {}", e);
        HttpError::server_error("Login failed")
    })?;

    // Find user by email or username (identifier contains '@' for email)
    let result = if body.identifier.contains('@') {
        app_state
            .db_client
            .get_user(None, None, Some(&body.identifier), None)
            .await
            .map_err(|e| {
                tracing::error!("DB error, getting user: {}", e);
                HttpError::server_error(ErrorMessage::ServerError.to_string())
            })?
    } else {
        app_state
            .db_client
            .get_user(None, Some(&body.identifier), None, None)
            .await
            .map_err(|e| {
                tracing::error!("DB error, getting user: {}", e);
                HttpError::server_error(ErrorMessage::ServerError.to_string())
            })?
    };

    let user = result.ok_or_else(|| {
        tracing::error!("User not found");
        HttpError::server_error("Login failed")
    })?;

    // Verify password hash
    let password_matched = password::compare(&body.password, &user.password).map_err(|e| {
        tracing::error!("Password error: {}", e);
        HttpError::server_error("Login failed")
    })?;

    if password_matched {
        // Create short-lived access token (15 minutes)
        let access_token = token::create_token(
            &user.id.to_string(),
            &app_state.env.jwt_secret.as_bytes(),
            app_state.env.jwt_maxage,
        )
        .map_err(|e| {
            tracing::error!("Access token creation error: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

        let access_cookie = Cookie::build(("access_token", access_token.clone()))
            .path("/")
            .http_only(true)
            .secure(true)
            .build();

        let response = axum::response::Json(UserLoginResponseDto {
            status: "success".to_string(),
            access_token,
            username: user.username,
        });

        // Create long-lived refresh token (7 days)
        let refresh_token = token::create_token(
            &user.id.to_string(),
            &app_state.env.jwt_secret.as_bytes(),
            app_state.env.refresh_token_maxage,
        )
        .map_err(|e| {
            tracing::error!("Refresh token creation error: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

        let refresh_cookie = Cookie::build(("refresh_token", &refresh_token))
            .path("/")
            .http_only(true)
            .secure(true)
            .build();

        let mut headers = HeaderMap::new();

        headers.append(
            header::SET_COOKIE,
            access_cookie.to_string().parse().unwrap(),
        );

        headers.append(
            header::SET_COOKIE,
            refresh_cookie.to_string().parse().unwrap(),
        );

        // Store refresh token in Redis for revocation support
        app_state
            .redis_client
            .save_refresh_token(
                &user.id.to_string(),
                &refresh_token,
                app_state.env.refresh_token_maxage,
            )
            .await
            .map_err(|e| {
                tracing::error!(user_id = %user.id, "RedisDB error, saving refresh token: {}", e);
                HttpError::server_error(ErrorMessage::ServerError.to_string())
            })?;

        let mut response = response.into_response();
        response.headers_mut().extend(headers);
        tracing::info!("authenticate_process succesful");
        Ok(response)
    } else {
        tracing::error!("password mismatch");
        Err(HttpError::server_error("Login failed"))
    }
}

/// Verify email via token from email link
#[instrument(skip(app_state))]
pub async fn verify_email(
    Query(query_params): Query<VerifyEmailQueryDto>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    query_params.validate().map_err(|e| {
        tracing::error!("Invalid verify email input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    // Find user by verification token
    let result = app_state
        .db_client
        .get_user(None, None, None, Some(&query_params.token))
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting user: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let user = result.ok_or({
        tracing::error!("User not found by verification token");
        HttpError::unauthorized(ErrorMessage::InvalidToken.to_string())
    })?;

    // Check token expiration
    if let Some(expires_at) = user.token_expires_at {
        if Utc::now() > expires_at {
            tracing::error!(user_id = %user.id, "Verification token expired");
            return Err(HttpError::bad_request(
                ErrorMessage::InvalidToken.to_string(),
            ));
        }
    } else {
        tracing::error!(user_id = %user.id, "Expire time not set");
        return Err(HttpError::bad_request(
            ErrorMessage::InvalidToken.to_string(),
        ));
    }

    // Mark token as verified in database
    app_state
        .db_client
        .verifed_token(&query_params.token)
        .await
        .map_err(|e| {
            tracing::error!(user_id = %user.id, "Verified status setting error: {}", e);
            HttpError::server_error(e.to_string())
        })?;

    // Token format: "UUID+newemail" indicates email change verification
    if query_params.token.contains('+') {
        let new_email = &query_params.token[37..];
        app_state
            .db_client
            .update_user_email(user.id, new_email)
            .await
            .map_err(|e| {
                tracing::error!(user_id = %user.id, new_email = %new_email, "Failed to update user email: {}", e);
                HttpError::server_error(e.to_string())})?;
    } else {
        // First-time verification, send welcome email
        let send_welcome_email_result = send_welcome_email(&user.email, &user.username).await;

        if let Err(e) = send_welcome_email_result {
            tracing::error!("Failed to send welcome email: {}", e);
        }
    }
    tracing::info!(user_id = %user.id, "Email verification successful");
    Ok((
        StatusCode::OK,
        Json(Response {
            status: "success",
            message: "Email verification successful.".to_string(),
        }),
    ))
}

/// Request password reset link (identifier can be email or username)
#[instrument(skip(app_state))]
pub async fn forgot_password(
    State(app_state): State<AppState>,
    Json(body): Json<ForgotPasswordRequestDto>,
) -> Result<impl IntoResponse, HttpError> {
    // Validate input
    body.validate().map_err(|e| {
        tracing::error!("Invalid forgot_password input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    // Find user by email or username
    let result = match body.identifier.as_str() {
        email if email.contains("@") => {
            app_state
                .db_client
                .get_user(None, None, Some(email), None)
                .await
        }
        username => {
            app_state
                .db_client
                .get_user(None, Some(username), None, None)
                .await
        }
    }
    .map_err(|e| {
        tracing::error!("Failed to fetch user: {}", e);
        HttpError::server_error(ErrorMessage::ServerError.to_string())
    })?;

    let user = result.ok_or_else(|| {
        tracing::error!("Email not found");
        HttpError::bad_request("Email not found".to_string())
    })?;

    // Create reset token valid for 30 minutes
    let verification_token = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(30);

    let user_id = uuid::Uuid::parse_str(&user.id.to_string()).unwrap();

    // Store reset token in database
    app_state
        .db_client
        .add_verifed_token(user_id, &verification_token, expires_at)
        .await
        .map_err(|e| {
            tracing::error!("DB error, adding verified token: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Build reset link with token
    let reset_link = format!(
        "{}/auth/password/reset/{}",
        app_state.env.frontend_url, &verification_token
    );

    // Send reset email
    let email_sent = send_forgot_password_email(&user.email, &reset_link, &user.username).await;

    if let Err(e) = email_sent {
        tracing::error!("Failed to send forgot password email: {}", e);
        return Err(HttpError::server_error("Failed to send email".to_string()));
    }

    let response = Response {
        message: "Password reset link has been sent to your email.".to_string(),
        status: "success",
    };
    tracing::info!(email = %user.email, "Forgot password email sent successfully");
    Ok(Json(response))
}

/// Reset password with token from email
#[instrument(skip(app_state, body))]
pub async fn reset_password(
    State(app_state): State<AppState>,
    Json(body): Json<ResetPasswordRequestDto>,
) -> Result<impl IntoResponse, HttpError> {
    body.validate().map_err(|e| {
        tracing::error!("Invalid reset_password input: {}", e);
        HttpError::bad_request(e.to_string())
    })?;

    // Find user by reset token
    let result = app_state
        .db_client
        .get_user(None, None, None, Some(&body.token))
        .await
        .map_err(|e| {
            tracing::error!("DB error, getting user by token: {}", e);
            HttpError::server_error(e.to_string())
        })?;

    let user = result.ok_or_else(|| {
        tracing::error!("User not found by reset token");
        HttpError::bad_request("Invalid or expired token".to_string())
    })?;

    // Check token expiration
    if let Some(expires_at) = user.token_expires_at {
        if Utc::now() > expires_at {
            tracing::error!(user_id = %user.id, "Verification token has expired");
            return Err(HttpError::bad_request(
                "Verification token has expired".to_string(),
            ));
        }
    } else {
        tracing::error!(user_id = %user.id, "Expire time not set for verification token");
        return Err(HttpError::bad_request(
            "Invalid verification token".to_string(),
        ));
    }

    let user_id = uuid::Uuid::parse_str(&user.id.to_string()).unwrap();

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

    // Mark token as used
    app_state
        .db_client
        .verifed_token(&body.token)
        .await
        .map_err(|e| {
            tracing::error!("DB error, nullifying token: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    let response = Response {
        message: "Password has been successfully reset.".to_string(),
        status: "success",
    };
    tracing::info!(user_id = %user.id, "Password reset successfully");
    Ok(Json(response))
}

/// Refresh access token using refresh token from cookie
#[instrument(skip(app_state, cookie_jar))]
pub async fn refresh(
    cookie_jar: CookieJar,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    // Extract refresh token from cookie
    let cookies = cookie_jar
        .get("refresh_token")
        .map(|cookie| cookie.value().to_string());

    let token = cookies.ok_or_else(|| {
        tracing::error!("Refresh token not provided");
        HttpError::unauthorized(ErrorMessage::TokenNotProvided.to_string())
    })?;

    // Decode and verify refresh token
    let token_details = match token::decode_token(&token, app_state.env.jwt_secret.as_bytes()) {
        Ok(token_details) => token_details,
        Err(e) => {
            tracing::error!("Invalid refresh token: {}", e);
            return Err(HttpError::unauthorized(
                ErrorMessage::InvalidToken.to_string(),
            ));
        }
    };

    // Verify refresh token exists in Redis (hasn't been revoked)
    let stored_refresh_token = app_state
        .redis_client
        .get_refresh_token(&token_details)
        .await
        .map_err(|e| {
            tracing::error!("RedisDB error, getting refresh token: {}", e);
            HttpError::server_error(ErrorMessage::ServerError.to_string())
        })?;

    // Ensure token matches stored value
    if stored_refresh_token.is_none() || stored_refresh_token.unwrap() != token {
        tracing::error!("Refresh token mismatch or not found in Redis");
        return Err(HttpError::server_error(
            "Refresh token mismatch".to_string(),
        ));
    }

    // Create new access token
    let access_token = token::create_token(
        &token_details,
        &app_state.env.jwt_secret.as_bytes(),
        app_state.env.jwt_maxage,
    )
    .map_err(|e| {
        tracing::error!("Access token creation error: {}", e);
        HttpError::server_error(ErrorMessage::ServerError.to_string())
    })?;

    let access_cookie = Cookie::build(("access_token", access_token.clone()))
        .path("/")
        .http_only(true)
        .secure(true)
        .build();

    let response = axum::response::Json(RefreshResponseDto {
        status: "access_token recreated".to_string(),
        access_token,
    });

    let mut headers = HeaderMap::new();

    headers.append(
        header::SET_COOKIE,
        access_cookie.to_string().parse().unwrap(),
    );

    let mut response = response.into_response();
    response.headers_mut().extend(headers);
    tracing::info!("Access token refreshed successfully");
    Ok(response)
}
