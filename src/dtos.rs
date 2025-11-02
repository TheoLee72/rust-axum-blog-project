use crate::models::{User, UserRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

// DTOs (Data Transfer Objects) define the structure of data exchanged with clients
// They are separate from database models to control exactly what data is exposed

// ============================================================================
// Authentication DTOs
// ============================================================================

/// Registration request from client
/// Validates input and ensures passwords match
#[derive(Validate, Debug, Default, Clone, Serialize, Deserialize)]
pub struct RegisterUserDto {
    #[validate(length(min = 1, message = "Name is required"))]
    pub username: String,

    #[validate(
        length(min = 1, message = "Email is required"),
        email(message = "Email is invalid")  // Validates email format
    )]
    pub email: String,

    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,

    #[validate(
        length(min = 1, message = "Confirm Password is required"),
        must_match(other = "password", message = "passwords do not match")
    )]
    #[serde(rename = "confirmPassword")] // JSON field name differs from Rust field name
    pub password_confirm: String,
}

/// Login request - accepts email or username
#[derive(Validate, Debug, Default, Clone, Serialize, Deserialize)]
pub struct LoginUserDto {
    #[validate(length(min = 1, message = "Email or username is required"))]
    pub identifier: String, // Can be email or username

    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

/// Password verification for sensitive operations (delete account, etc.)
#[derive(Validate, Serialize, Deserialize)]
pub struct DoubleCheckDto {
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

// ============================================================================
// Pagination & Query DTOs
// ============================================================================

/// Generic pagination query parameters
#[derive(Serialize, Deserialize, Validate, Debug)]
pub struct RequestQueryDto {
    #[validate(range(min = 1))]
    pub page: Option<usize>,

    #[validate(range(min = 1, max = 50))] // Limit maximum results per page
    pub limit: Option<usize>,
}

// ============================================================================
// User Response DTOs (filtered data for client)
// ============================================================================

/// Filtered user data sent to clients (excludes sensitive fields like password)
#[derive(Debug, Serialize, Deserialize)]
pub struct FilterUserDto {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub verified: bool,
    #[serde(rename = "createdAt")] // Use camelCase for JavaScript clients
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl FilterUserDto {
    /// Convert database User model to client-safe FilterUserDto
    /// Excludes password hash and other sensitive fields
    pub fn filter_user(user: &User) -> Self {
        FilterUserDto {
            id: user.id.to_string(),
            name: user.username.to_owned(),
            email: user.email.to_owned(),
            verified: user.verified,
            role: user.role.to_str().to_string(),
            created_at: user.created_at.unwrap(),
            updated_at: user.updated_at.unwrap(),
        }
    }

    /// Convert multiple users at once
    pub fn filter_users(user: &[User]) -> Vec<FilterUserDto> {
        user.iter().map(FilterUserDto::filter_user).collect()
    }
}

/// Single user response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct UserData {
    pub user: FilterUserDto,
}

/// User profile with additional statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct UserMeData {
    pub user: FilterUserDto,
    pub post_count: i64,
    pub comment_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserMeResponseDto {
    pub status: String,
    pub data: UserMeData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponseDto {
    pub status: String,
    pub data: UserData,
}

/// User list with count
#[derive(Debug, Serialize, Deserialize)]
pub struct UserListResponseDto {
    pub status: String,
    pub users: Vec<FilterUserDto>,
    pub results: i64,
}

/// Login success response with JWT token
#[derive(Debug, Serialize, Deserialize)]
pub struct UserLoginResponseDto {
    pub status: String,
    pub access_token: String,
    pub username: String,
}

/// Token refresh response
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshResponseDto {
    pub status: String,
    pub access_token: String,
}

/// Generic success response
#[derive(Serialize, Deserialize)]
pub struct Response {
    pub status: &'static str,
    pub message: String,
}

// ============================================================================
// User Update DTOs
// ============================================================================

#[derive(Validate, Debug, Default, Clone, Serialize, Deserialize)]
pub struct NameUpdateDto {
    #[validate(length(min = 1, message = "Name is required"))]
    pub name: String,
}

#[derive(Validate, Debug, Default, Clone, Serialize, Deserialize)]
pub struct EmailUpdateDto {
    #[validate(length(min = 1, message = "Email is required"))]
    #[validate(email)]
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RoleUpdateDto {
    pub role: UserRole,
}

/// Password change request (requires old password verification)
#[derive(Debug, Validate, Default, Clone, Serialize, Deserialize)]
pub struct UserPasswordUpdateDto {
    #[validate(length(min = 6, message = "new password must be at least 6 characters"))]
    pub new_password: String,

    #[validate(
        length(
            min = 6,
            message = "new password confirm must be at least 6 characters"
        ),
        must_match(other = "new_password", message = "new passwords do not match")
    )]
    pub new_password_confirm: String,

    #[validate(length(min = 6, message = "Old password must be at least 6 characters"))]
    pub old_password: String,
}

// ============================================================================
// Email Verification & Password Reset DTOs
// ============================================================================

#[derive(Serialize, Deserialize, Validate, Debug)]
pub struct VerifyEmailQueryDto {
    #[validate(length(min = 1, message = "Token is required."))]
    pub token: String,
}

#[derive(Deserialize, Serialize, Validate, Debug, Clone)]
pub struct ForgotPasswordRequestDto {
    #[validate(length(min = 1, message = "Identifier is required"))]
    pub identifier: String,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct ResetPasswordRequestDto {
    #[validate(length(min = 1, message = "Token is required."))]
    pub token: String,

    #[validate(length(min = 6, message = "new password must be at least 6 characters"))]
    pub new_password: String,

    #[validate(
        length(
            min = 6,
            message = "new password confirm must be at least 6 characters"
        ),
        must_match(other = "new_password", message = "new passwords do not match")
    )]
    pub new_password_confirm: String,
}

// ============================================================================
// Post DTOs
// ============================================================================

/// Post creation/update request (used for both POST and PUT)
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct InputPostDto {
    #[validate(length(min = 1, message = "Content is required."))]
    pub content: String,

    #[validate(length(min = 1, message = "Title is required."))]
    pub title: String,
}

/// Full post data response
#[derive(Debug, Serialize, Deserialize)]
pub struct PostDto {
    pub id: i32,
    #[serde(rename = "userUsername")]
    pub user_username: String,
    pub content: String,
    pub summary: String,
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

/// Pagination metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationDto {
    pub page: i32,
    pub limit: i32,
    pub total: i32,
    #[serde(rename = "totalPages")]
    pub total_pages: i32,
}

/// Simplified post data for list views (excludes full content)
#[derive(Debug, Serialize, Deserialize)]
pub struct PostPaginationDto {
    pub id: i32,
    #[serde(rename = "userUsername")]
    pub user_username: String,
    pub summary: String, // Only summary, not full content
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

/// Paginated posts response
#[derive(Debug, Serialize, Deserialize)]
pub struct PostsPaginationResponseDto {
    pub status: String,
    pub data: Vec<PostPaginationDto>,
    pub pagination: Option<PaginationDto>,
}

/// Single post response
#[derive(Debug, Serialize, Deserialize)]
pub struct PostResponseDto {
    pub status: String,
    pub data: PostDto,
}

/// Query parameters for fetching posts
#[derive(Debug, Deserialize, Validate)]
pub struct PostsQueryParams {
    #[validate(range(min = 1))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 25))]
    pub limit: Option<i32>,

    #[validate(length(min = 1))]
    pub user_username: Option<String>, // Filter by author
}

// ============================================================================
// Comment DTOs
// ============================================================================

#[derive(Debug, Deserialize, Validate)]
pub struct InputcommentRequest {
    #[validate(length(
        min = 1,
        max = 1000,
        message = "Content must be between 1 and 1000 characters"
    ))]
    pub content: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct GetcommentsQuery {
    #[validate(range(min = 1, message = "Page must be greater than 0"))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 100, message = "Limit must be between 1 and 100"))]
    pub limit: Option<i32>,

    #[validate(custom(function = "validate_sort"))]
    pub sort: Option<String>, // created_at_desc or created_at_asc
}

/// Custom validator for sort parameter
fn validate_sort(sort: &String) -> Result<(), validator::ValidationError> {
    if sort == "created_at_desc" || sort == "created_at_asc" {
        Ok(())
    } else {
        Err(validator::ValidationError::new("invalid_sort"))
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommentDto {
    pub id: i32,
    #[serde(rename = "userUsername")]
    pub user_username: String,
    pub post_id: i32,
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentListResponse {
    pub status: String,
    pub data: Vec<CommentDto>,
    pub pagination: PaginationDto,
}

#[derive(Debug, Serialize)]
pub struct SinglecommentResponse {
    pub status: String,
    pub data: CommentDto,
}

// ============================================================================
// Search & Misc DTOs
// ============================================================================

#[derive(Debug, Validate, Deserialize)]
pub struct GetSearchQuery {
    #[validate(length(min = 1))]
    pub q: String, // Search query
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// LLM API request structure
#[derive(Debug, Serialize)]
pub struct LLMReqeustTextInput {
    pub model: String,
    pub input: String,
}

/// Image upload response
#[derive(Serialize)]
pub struct UploadResponse {
    pub location: String, // URL of uploaded image
}

/// Newsletter subscription request
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct NewsletterDto {
    #[validate(email)]
    pub email: String,
}
