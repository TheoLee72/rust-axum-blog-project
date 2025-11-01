// Important: JWT enables stateless authentication, but that doesn't mean eliminating ALL state
//
// What this means:
// - Stateless: Server doesn't need to store session data or look up tokens in database
// - But we still store refresh tokens in Redis for security (revocation, logout)
// - We still query the database to verify the user exists
// - "Stateless" refers to the authentication mechanism, not the entire application

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::error::{ErrorMessage, HttpError};

/// JWT token claims (payload)
///
/// **What is a JWT?**
/// JWT (JSON Web Token) is a compact, URL-safe token format for securely transmitting
/// information between parties. It consists of three parts separated by dots:
///
/// ```
/// header.payload.signature
/// ```
///
/// Example JWT:
/// ```
/// eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c
/// ```
///
/// **Three Parts:**
/// 1. **Header**: Algorithm and token type
///    ```
///    {"alg": "HS256", "typ": "JWT"}
///    ```
///
/// 2. **Payload** (this struct): The claims (actual data)
///    ```
///    {"sub": "user_id", "iat": 1234567890, "exp": 1234571490}
///    ```
///
/// 3. **Signature**: Cryptographic signature to verify integrity
///    ```
///    HMACSHA256(base64(header) + "." + base64(payload), secret)
///    ```
///
/// **Standard JWT Claims:**
/// - `sub` (subject): Identifies the principal (usually user ID)
/// - `iat` (issued at): Timestamp when token was created
/// - `exp` (expiration): Timestamp when token expires
///
/// Other standard claims not used here:
/// - `iss` (issuer): Who issued the token
/// - `aud` (audience): Who the token is intended for
/// - `nbf` (not before): Token is not valid before this time
/// - `jti` (JWT ID): Unique identifier for the token
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String, // Subject: User ID (UUID as string)
    pub iat: usize,  // Issued At: Unix timestamp when token was created
    pub exp: usize,  // Expiration: Unix timestamp when token expires
}

/// Create a signed JWT token
///
/// **How JWT Signing Works:**
/// 1. Create the payload (claims) with user data and timestamps
/// 2. Encode header and payload as base64
/// 3. Create signature: HMAC-SHA256(header.payload, secret)
/// 4. Combine all three parts: header.payload.signature
///
/// **Why Sign Tokens?**
/// Signing ensures:
/// - **Integrity**: Token hasn't been tampered with
/// - **Authenticity**: Token was issued by our server (only we have the secret)
/// - **Non-repudiation**: Can prove the token came from us
///
/// **Security Notes:**
/// - The secret MUST be kept secure (environment variable, never in code)
/// - Use a strong secret (at least 32 random bytes)
/// - Tokens are NOT encrypted - anyone can read the payload (base64 decode)
/// - Never put sensitive data in tokens (passwords, credit cards, etc.)
/// - Token contents are visible but tamper-proof
///
/// **Access vs Refresh Tokens:**
/// Typically used for two types of tokens:
/// - **Access Token**: Short-lived (15-60 minutes), used for API requests
/// - **Refresh Token**: Long-lived (7-30 days), used to get new access tokens
///
/// # Parameters
/// - `data`: User ID or other identifier to embed in the token (usually UUID)
/// - `secret`: Secret key for signing (from environment variable)
/// - `expires_in_seconds`: How long until the token expires
///   - Access tokens: 900-3600 seconds (15-60 minutes)
///   - Refresh tokens: 604800-2592000 seconds (7-30 days)
///
/// # Returns
/// - `Ok(String)`: The complete JWT token (header.payload.signature)
/// - `Err(jsonwebtoken::errors::Error)`: If encoding fails or data is invalid
///
/// # Example
/// ```
/// // Create 15-minute access token
/// let access_token = create_token(&user.id.to_string(), secret.as_bytes(), 900)?;
///
/// // Create 7-day refresh token
/// let refresh_token = create_token(&user.id.to_string(), secret.as_bytes(), 604800)?;
/// ```
pub fn create_token(
    data: &str,
    secret: &[u8],
    expires_in_seconds: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    // Validation: Reject empty subjects
    // The subject (user ID) is critical - without it, we can't identify who the token belongs to
    if data.is_empty() {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidSubject.into());
    }

    // Get current timestamp
    let now = Utc::now();

    // Calculate issued at time (iat)
    // Unix timestamp = seconds since January 1, 1970 00:00:00 UTC
    let iat = now.timestamp() as usize;

    // Calculate expiration time (exp)
    // Add the specified duration to current time
    let exp = (now + Duration::seconds(expires_in_seconds)).timestamp() as usize;

    // Build the token claims (payload)
    let claims = TokenClaims {
        sub: data.to_string(),
        iat,
        exp,
    };

    // Encode and sign the JWT
    //
    // Header::default() creates:
    // {"alg": "HS256", "typ": "JWT"}
    //
    // Algorithm HS256 (HMAC-SHA256):
    // - Symmetric algorithm (same key for signing and verification)
    // - Fast and secure for server-to-server or server-to-client tokens
    // - Not suitable if clients need to verify tokens (use RS256 for that)
    //
    // Process:
    // 1. Serialize header and claims to JSON
    // 2. Base64-encode both
    // 3. Create signature: HMAC-SHA256(base64(header).base64(claims), secret)
    // 4. Base64-encode signature
    // 5. Concatenate: base64(header).base64(claims).base64(signature)
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
}

/// Decode and verify a JWT token
///
/// **How JWT Verification Works:**
/// 1. Split token into header, payload, signature
/// 2. Decode header and payload from base64
/// 3. Recompute signature using the secret
/// 4. Compare computed signature with token signature (constant-time)
/// 5. Check expiration time (exp) against current time
/// 6. If all checks pass, return the claims
///
/// **What Gets Validated:**
/// - Signature is valid (token hasn't been tampered with)
/// - Token was signed with our secret (authentication)
/// - Token hasn't expired (exp < now)
/// - Token format is correct (proper JWT structure)
///
/// **Why Validation Fails:**
/// - Token was modified (signature won't match)
/// - Token was signed with wrong secret (signature won't match)
/// - Token has expired (exp is in the past)
/// - Token is malformed (invalid base64, missing parts)
///
/// **Security Model:**
/// JWTs rely on the secret key. If an attacker:
/// - Knows the secret: Can create valid tokens (CRITICAL: protect your secret!)
/// - Doesn't know secret: Cannot create or modify tokens (signatures fail)
/// - Steals a token: Can use it until expiration (why we use short-lived access tokens)
///
/// # Parameters
/// - `token`: The JWT token string to verify
/// - `secret`: Secret key used to sign the token (must match the one used in create_token)
///
/// # Returns
/// - `Ok(String)`: The subject (user ID) extracted from the token
/// - `Err(HttpError)`: If token is invalid, expired, or signature doesn't match
///
/// # Example
/// ```
/// // In authentication middleware:
/// let token = extract_token_from_header(req)?;
/// let user_id_str = decode_token(token, secret.as_bytes())?;
/// let user_id = Uuid::parse_str(&user_id_str)?;
///
/// // Now fetch user from database
/// let user = db.get_user(user_id).await?;
/// ```
pub fn decode_token<T: Into<String>>(token: T, secret: &[u8]) -> Result<String, HttpError> {
    // Decode and verify the token
    //
    // Validation::new(Algorithm::HS256) creates a validator that:
    // - Verifies the signature using HMAC-SHA256
    // - Checks that exp (expiration) hasn't passed
    // - Validates the token structure
    //
    // Why use the same algorithm (HS256)?
    // - Must match the algorithm used to create the token
    // - Prevents algorithm substitution attacks (attacker changes "HS256" to "none")
    // - The validation ensures the algorithm in the header matches what we expect
    let decode = decode::<TokenClaims>(
        &token.into(),
        &DecodingKey::from_secret(secret),
        &Validation::new(Algorithm::HS256), // Also validates expiration automatically
    );

    // Handle the result
    // - If successful: Extract and return the subject (user ID)
    // - If failed: Return 401 Unauthorized error
    match decode {
        Ok(token) => Ok(token.claims.sub),
        Err(_) => Err(HttpError::new(
            ErrorMessage::InvalidToken.to_string(),
            StatusCode::UNAUTHORIZED,
        )),
    }
}
