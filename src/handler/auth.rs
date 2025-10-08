use axum::{extract::{Query, State}, http::{header, HeaderMap, StatusCode}, response::{IntoResponse, Redirect}, routing::{get, post}, Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::{Utc, Duration};
use validator::Validate;
use crate::{db::UserExt, dtos::{ForgotPasswordRequestDto, LoginUserDto, RegisterUserDto, ResetPasswordRequestDto, Response, UserLoginResponseDto, VerifyEmailQueryDto}, error::{ErrorMessage, HttpError}, mail::mails::{send_forgot_password_email, send_verification_email, send_welcome_email}, utils::{password, token}, AppState};

pub fn auth_handler() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/verify", get(verify_email))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
        .route("/refresh", post(refresh))
        //route가 끝. 경로추가
        //merge는 라우터끼리 합침. 
}   

pub async fn register(
    State(app_state): State<AppState>,
    Json(body): Json<RegisterUserDto>
) -> Result<impl IntoResponse, HttpError> {
    body.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let verification_token = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::hours(24);

    let hash_password = password::hash(&body.password)
            .map_err(|e| HttpError::server_error(e.to_string()))?;

    let result = app_state.db_client
        .save_user(&body.username, &body.email, &hash_password, &verification_token, expires_at)
        .await;

    match result {
        Ok(_user) => {
            let send_email_result = send_verification_email(&body.email, &body.username, &verification_token).await;

            if let Err(e) = send_email_result {
                eprintln!("Failed to send verification email: {}", e);
            }

            Ok((StatusCode::CREATED, Json(Response {
                status: "success",
                message: "Registration successful! Please check your email to verify your account.".to_string()
            })))
        },
        Err(sqlx::Error::Database(db_err)) => {
            if db_err.is_unique_violation() {
                Err(HttpError::unique_constraint_violation(
                    ErrorMessage::EmailExist.to_string(),
                ))
            } else {
                Err(HttpError::server_error(db_err.to_string()))
            }
        }
        Err(e) => Err(HttpError::server_error(e.to_string()))
    }
}

pub async fn login(
    State(app_state): State<AppState>,
    Json(body): Json<LoginUserDto>
) -> Result<impl IntoResponse, HttpError> {
    body.validate()
       .map_err(|e| HttpError::bad_request(e.to_string()))?;
    let result = if body.identifier.contains('@') {
        app_state.db_client
        .get_user(None, None, Some(&body.identifier), None)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?
    }
    else {
    app_state.db_client
        .get_user(None, Some(&body.identifier), None, None)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?
    };


    let user = result.ok_or(HttpError::bad_request(ErrorMessage::WrongCredentials.to_string()))?;

    let password_matched = password::compare(&body.password, &user.password)
        .map_err(|_| HttpError::bad_request(ErrorMessage::WrongCredentials.to_string()))?;

    if password_matched {
        //access_token
        let access_token = token::create_token(
            &user.id.to_string(), 
            &app_state.env.jwt_secret.as_bytes(), 
            app_state.env.jwt_maxage
        )
        .map_err(|e| HttpError::server_error(e.to_string()))?;

        let access_cookie = Cookie::build(("access_token", access_token.clone()))
            .path("/")
            .http_only(true)
            .secure(true)
            .build();

        let response = axum::response::Json(UserLoginResponseDto {
            status: "success".to_string(),
            access_token,
        });

        //refresh_token
        let refresh_token = token::create_token(
            &user.id.to_string(),
            &app_state.env.jwt_secret.as_bytes(),
            app_state.env.refresh_token_maxage,
        )
        .map_err(|e| HttpError::server_error(e.to_string()))?;

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

        //redis
        app_state.redis_client.save_refresh_token(
            &user.id.to_string(), 
            &refresh_token, 
            app_state.env.refresh_token_maxage).await.map_err(|e| HttpError::server_error(e.to_string()))?;

        

        let mut response = response.into_response();
        response.headers_mut().extend(headers);

        Ok(response)
    } else {
        Err(HttpError::bad_request(ErrorMessage::WrongCredentials.to_string()))
    }
}

pub async fn verify_email(
    Query(query_params): Query<VerifyEmailQueryDto>,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    query_params.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let result = app_state.db_client
        .get_user(None, None, None, Some(&query_params.token))
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let user = result.ok_or(HttpError::unauthorized(ErrorMessage::InvalidToken.to_string()))?;

    if let Some(expires_at) = user.token_expires_at {
        if Utc::now() > expires_at {
            return Err(HttpError::bad_request("Verification token has expired".to_string()))?;
        }
    } else {
        return Err(HttpError::bad_request("Invalid verification token".to_string()))?;
    }

    app_state.db_client.verifed_token(&query_params.token).await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let send_welcome_email_result = send_welcome_email(&user.email, &user.username).await;

    if let Err(e) = send_welcome_email_result {
        eprintln!("Failed to send welcome email: {}", e);
    }

    let token = token::create_token(
        &user.id.to_string(), 
        app_state.env.jwt_secret.as_bytes(),
        app_state.env.jwt_maxage 
    ).map_err(|e| HttpError::server_error(e.to_string()))?;

    let cookie_duration = time::Duration::minutes(app_state.env.jwt_maxage * 60);
    let cookie = Cookie::build(("token", token.clone()))
        .path("/")
        .max_age(cookie_duration)
        .http_only(true)
        .build();

    let mut headers = HeaderMap::new();

    headers.append(
        header::SET_COOKIE,
        cookie.to_string().parse().unwrap() 
    );

    let frontend_url = format!("http://localhost:5173/settings"); //수정해라.

    let redirect = Redirect::to(&frontend_url);

    let mut response = redirect.into_response();

    response.headers_mut().extend(headers);

    Ok(response)
}

pub async fn forgot_password(
    State(app_state): State<AppState>,
    Json(body): Json<ForgotPasswordRequestDto>
) -> Result<impl IntoResponse, HttpError> {
    body.validate()
       .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let result = app_state.db_client
            .get_user(None, None, Some(&body.email), None)
            .await
            .map_err(|e| HttpError::server_error(e.to_string()))?;

    let user = result.ok_or(HttpError::bad_request("Email not found!".to_string()))?;

    let verification_token = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::minutes(30);

    let user_id = uuid::Uuid::parse_str(&user.id.to_string()).unwrap();

    app_state.db_client
        .add_verifed_token(user_id, &verification_token, expires_at)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let reset_link = format!("http://localhost:5173/reset-password?token={}", &verification_token);

    let email_sent = send_forgot_password_email(&user.email, &reset_link, &user.username).await;

    if let Err(e) = email_sent {
        eprintln!("Failed to send forgot password email: {}", e);
        return Err(HttpError::server_error("Failed to send email".to_string()));
    }

    let response = Response {
        message: "Password reset link has been sent to your email.".to_string(),
        status: "success",
    };

    Ok(Json(response))
}

pub async fn reset_password(
    State(app_state): State<AppState>,
    Json(body): Json<ResetPasswordRequestDto>
) -> Result<impl IntoResponse, HttpError> {
    body.validate()
        .map_err(|e| HttpError::bad_request(e.to_string()))?;

    let result = app_state.db_client
        .get_user(None, None, None, Some(&body.token))
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let user = result.ok_or(HttpError::bad_request("Invalid or expired token".to_string()))?;

    if let Some(expires_at) = user.token_expires_at {
        if Utc::now() > expires_at {
            return Err(HttpError::bad_request("Verification token has expired".to_string()))?;
        }
    }else {
        return Err(HttpError::bad_request("Invalid verification token".to_string()))?;
    }

    let user_id = uuid::Uuid::parse_str(&user.id.to_string()).unwrap();

    let hash_password = password::hash(&body.new_password)
            .map_err(|e| HttpError::server_error(e.to_string()))?;

    app_state.db_client
        .update_user_password(user_id.clone(), hash_password)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    app_state.db_client
        .verifed_token(&body.token)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let response = Response {
        message: "Password has been successfully reset.".to_string(),
        status: "success",
    };

    Ok(Json(response))
}

pub async fn refresh(
    cookie_jar: CookieJar,
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, HttpError> {
    let cookies = cookie_jar
            .get("refresh_token")
            .map(|cookie| cookie.value().to_string());
    
    let token = cookies.ok_or_else(|| {
        HttpError::unauthorized(ErrorMessage::TokenNotProvided.to_string())}
    )?;

    let token_details = 
        match token::decode_token(&token, app_state.env.jwt_secret.as_bytes()) {
            Ok(token_details) => token_details,
            Err(_) => {
                return Err(HttpError::unauthorized(ErrorMessage::InvalidToken.to_string()))
            }
        };
    
    let stored_refresh_token = app_state.redis_client
        .get_refresh_token(&token_details)
        .await
        .map_err(|e| HttpError::server_error(e.to_string()))?;
    
    if stored_refresh_token.is_none() || stored_refresh_token.unwrap() != token {
        return Err(HttpError::server_error("Refresh token mismatch".to_string()));
    }

    //통과, accesstoken 발급하자.
    let access_token = token::create_token(
            &token_details, 
            &app_state.env.jwt_secret.as_bytes(), 
            app_state.env.jwt_maxage
        )
        .map_err(|e| HttpError::server_error(e.to_string()))?;

    let access_cookie = Cookie::build(("access_token", access_token.clone()))
        .path("/")
        .http_only(true)
        .secure(true)
        .build();

    let response = axum::response::Json(UserLoginResponseDto {
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

    Ok(response)
    
    
}