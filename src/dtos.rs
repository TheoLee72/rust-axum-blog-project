use crate::models::{User, UserRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;
//dto는 client와 통신하는 형태를 정해두는 느낌.
#[derive(Validate, Debug, Default, Clone, Serialize, Deserialize)]
//validate는 validator crate때문에 쓰는거고, Default는 기본값 설정 할 수 있대.
//clone은 일단 써두고 안필요하면 그냥 참조하고, 굳이 필요하다면(멀티스레드 환경) .clone호출 하면 됨.
pub struct RegisterUserDto {
    #[validate(length(min = 1, message = "Name is required"))]
    pub username: String,
    #[validate(
        length(min = 1, message = "Email is required"),
        email(message = "Email is invalid") //email 형식인지 검증.
    )]
    pub email: String,
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,

    #[validate(
        length(min = 1, message = "Confirm Password is required"),
        must_match(other = "password", message = "passwords do not match")
    )]
    #[serde(rename = "confirmPassword")]
    pub password_confirm: String,
}

#[derive(Validate, Debug, Default, Clone, Serialize, Deserialize)]
pub struct LoginUserDto {
    #[validate(length(min = 1, message = "Email or username is required"))]
    pub identifier: String, //email이랑 username 둘다 되게.
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

#[derive(Validate, Serialize, Deserialize)]
pub struct DoubleCheckDto {
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

#[derive(Serialize, Deserialize, Validate)]
pub struct RequestQueryDto {
    #[validate(range(min = 1))]
    pub page: Option<usize>,
    #[validate(range(min = 1, max = 50))]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)] //user data output용도
pub struct FilterUserDto {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub verified: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

impl FilterUserDto {
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

    pub fn filter_users(user: &[User]) -> Vec<FilterUserDto> {
        user.iter().map(FilterUserDto::filter_user).collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserData {
    pub user: FilterUserDto,
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct UserListResponseDto {
    pub status: String,
    pub users: Vec<FilterUserDto>,
    pub results: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserLoginResponseDto {
    pub status: String,
    pub access_token: String,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshResponseDto {
    pub status: String,
    pub access_token: String,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub status: &'static str,
    pub message: String,
}

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

#[derive(Serialize, Deserialize, Validate)]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct InputPostDto {
    //post, put 둘다 쓸거임.
    #[validate(length(min = 1, message = "Content is required."))]
    pub content: String,
    #[validate(length(min = 1, message = "Title is required."))]
    pub title: String,
}

/// 단일 Post 데이터
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

/// 페이지네이션 정보
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationDto {
    pub page: i32,
    pub limit: i32,
    pub total: i32,
    #[serde(rename = "totalPages")]
    pub total_pages: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostPaginationDto {
    pub id: i32,
    #[serde(rename = "userUsername")]
    pub user_username: String,
    pub summary: String,
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

/// 전체 응답 구조
#[derive(Debug, Serialize, Deserialize)]
pub struct PostsPaginationResponseDto {
    pub status: String,
    pub data: Vec<PostPaginationDto>,
    pub pagination: Option<PaginationDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostResponseDto {
    pub status: String,
    pub data: PostDto,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PostsQueryParams {
    #[validate(range(min = 1))]
    pub page: Option<i32>,
    #[validate(range(min = 1, max = 25))]
    pub limit: Option<i32>,
    #[validate(length(min = 1))]
    pub user_username: Option<String>,
}

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
    pub sort: Option<String>, // created_at_desc, created_at_asc
}

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

#[derive(Debug, Validate, Deserialize)]
pub struct GetSearchQuery {
    #[validate(length(min = 1))]
    pub q: String,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}
#[derive(Debug, Serialize)]
pub struct LLMReqeustTextInput {
    pub model: String,
    pub input: String,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub location: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct NewsletterDto {
    #[validate(email)]
    pub email: String,
}
