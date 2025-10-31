//JWT는 “인증을 stateless하게 처리한다”는 거지, 모든 상태를 없앤다는 뜻은 아니야.
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode,
    encode,
    Algorithm,
    DecodingKey,
    EncodingKey,
    Header,
    Validation
};
use serde::{Deserialize, Serialize};

use crate::error::{ErrorMessage, HttpError};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims{//subject => 보통 user_id, iat => issued at, 발급시간, exp => expiration, 
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}

pub fn create_token(
    user_id: &str,
    secret: &[u8],
    expires_in_seconds: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    if user_id.is_empty() {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidSubject.into());
    }

    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + Duration::seconds(expires_in_seconds)).timestamp() as usize;
    let claims = TokenClaims {
        sub: user_id.to_string(),
        iat,
        exp,
    };

    encode(
        &Header::default(), 
        &claims, 
        &EncodingKey::from_secret(secret)
    )
}

pub fn decode_token<T: Into<String>>(
    token: T,
    secret: &[u8]
) -> Result<String, HttpError> {
    let decode = decode::<TokenClaims>(
        &token.into(), 
        &DecodingKey::from_secret(secret), 
        &Validation::new(Algorithm::HS256), //Validation::new()여기서 만료된것도 체크.
    );

    match decode {
        Ok(token) => Ok(token.claims.sub),
        Err(_) => Err(HttpError::new(ErrorMessage::InvalidToken.to_string(), StatusCode::UNAUTHORIZED))
    }
}