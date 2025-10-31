use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User role enumeration for role-based access control (RBAC)
///
/// This enum represents different permission levels in the application.
/// It's stored in the database as a PostgreSQL ENUM type called "user_role".
///
/// Derive macros explained:
/// - `Debug`: Enables debug printing with {:?}
/// - `Deserialize/Serialize`: Enables JSON conversion (for API requests/responses)
/// - `Clone, Copy`: Allows cheap copying (enums are small)
/// - `sqlx::Type`: Maps this Rust enum to a PostgreSQL ENUM type
/// - `PartialEq`: Enables equality comparisons (user.role == UserRole::Admin)
///
/// The `#[sqlx(type_name = "user_role", rename_all = "lowercase")]` attribute:
/// - Maps to the "user_role" ENUM in PostgreSQL
/// - Converts variants to lowercase in the database (Admin -> "admin")
#[derive(Debug, Deserialize, Serialize, Clone, Copy, sqlx::Type, PartialEq)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum UserRole {
    Admin, // Full system access
    User,  // Standard user permissions
}

impl UserRole {
    /// Convert role to string representation
    ///
    /// This is useful for display purposes or when you need a &str
    /// instead of the enum variant itself.
    pub fn to_str(&self) -> &str {
        match self {
            UserRole::Admin => "admin",
            UserRole::User => "user",
        }
    }
}

/// User model representing the users table
///
/// This struct maps directly to database rows using SQLx's FromRow derive macro.
/// Each field corresponds to a column in the "users" table.
///
/// Derive macros explained:
/// - `Debug`: For debugging and logging
/// - `Deserialize/Serialize`: JSON serialization for API responses
/// - `sqlx::FromRow`: Automatically maps database rows to this struct
/// - `Clone`: Allows creating copies (needed when passing user data around)
///
/// Security notes:
/// - `password`: Stores hashed password (never plain text)
/// - `verification_token`: Used for email verification workflow, etc.
/// - `token_expires_at`: Ensures verification tokens expire after a certain time
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Clone)]
pub struct User {
    pub id: uuid::Uuid, // Primary key (UUID for better security than sequential IDs)
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: UserRole,
    pub verified: bool,                     // Whether email has been verified
    pub verification_token: Option<String>, // Token sent via email for verification (None after verification)
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Post model representing blog posts/articles
///
/// This struct stores blog post data with multiple representations of content:
/// - `content`: Full HTML content for rendering
/// - `raw_text`: Plain text extracted from HTML (used for full-text search, semantic embedding, summary)
/// - `summary`: Brief description or excerpt
///
/// Advanced features (stored in database but not in this struct):
/// - `content_tsv`: tsvector column for PostgreSQL full-text search
/// - `embedding`: pgvector column for semantic/vector similarity search
///
/// Note: These special columns are handled separately because they require
/// custom types that aren't represented in standard Rust structs.
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Clone)]
pub struct Post {
    pub id: i64,
    pub user_id: uuid::Uuid,
    pub content: String,
    pub raw_text: String,
    pub summary: String,
    pub title: String,
    // Note: content_tsv (tsvector) and embedding (pgvector) columns exist in DB
    // but are handled separately for full-text search and semantic search
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Comment model representing user comments on blog posts
///
/// This creates a one-to-many relationship:
/// - One post can have many comments
/// - One user can write many comments
///
/// The relationship is established through foreign keys:
/// - `user_id`: References users.id
/// - `post_id`: References posts.id
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Clone)]
pub struct Comment {
    pub id: i64,       // Primary key (auto-incrementing)
    pub user_id: Uuid, // Foreign key: which user wrote this comment
    pub post_id: i64,  // Foreign key: which post this comment belongs to
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Newsletter subscription model
///
/// Stores email addresses of users who subscribed to the newsletter.
/// This is separate from the User model because:
/// - Newsletter subscribers don't need full accounts
/// - It's simpler and respects privacy (minimal data collection)
/// - Allows non-registered users to subscribe
///
/// GDPR compliance note: Make sure to implement unsubscribe functionality
/// and honor deletion requests for these email addresses.
#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Clone)]
pub struct NewsletterEmail {
    pub id: Uuid,                  // Primary key (UUID for better security)
    pub email: String,             // Subscriber's email address
    pub created_at: DateTime<Utc>, // When they subscribed
}
