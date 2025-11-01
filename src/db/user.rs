use super::DBClient;
use crate::models::{User, UserRole};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// User database operations trait
pub trait UserExt {
    /// Get single user by ID, username, email, or verification token
    /// Returns Option - Some(user) if found, None if not found
    async fn get_user(
        &self,
        user_id: Option<Uuid>,
        username: Option<&str>,
        email: Option<&str>,
        token: Option<&str>,
    ) -> Result<Option<User>, sqlx::Error>;

    /// Get paginated list of all users
    async fn get_users(&self, page: u32, limit: usize) -> Result<Vec<User>, sqlx::Error>;

    /// Create new user with verification token
    async fn save_user<T: Into<String> + Send>(
        &self,
        username: T,
        email: T,
        password: T,
        verification_token: T,
        token_expires_at: DateTime<Utc>,
    ) -> Result<User, sqlx::Error>;

    /// Delete user by ID
    async fn delete_user(&self, user_id: Uuid) -> Result<(), sqlx::Error>;

    /// Get total count of all users
    async fn get_user_count(&self) -> Result<i64, sqlx::Error>;

    /// Update user's display name
    async fn update_user_name<T: Into<String> + Send>(
        &self,
        user_id: Uuid,
        new_username: T,
    ) -> Result<User, sqlx::Error>;

    /// Update user's role (Admin or User)
    async fn update_user_role(&self, user_id: Uuid, role: UserRole) -> Result<User, sqlx::Error>;

    /// Update user's password
    async fn update_user_password(
        &self,
        user_id: Uuid,
        password: String,
    ) -> Result<User, sqlx::Error>;

    /// Update user's email address
    async fn update_user_email(&self, user_id: Uuid, new_email: &str) -> Result<User, sqlx::Error>;

    /// Mark verification token as used (verified email)
    async fn verifed_token(&self, token: &str) -> Result<(), sqlx::Error>;

    /// Store new verification token (for password reset or email change)
    async fn add_verifed_token(
        &self,
        user_id: Uuid,
        token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error>;

    /// Check if email is already in use by another user
    async fn check_email_duplicate(
        &self,
        user_id: Uuid,
        new_email: &str,
    ) -> Result<(), sqlx::Error>;
}

impl UserExt for DBClient {
    async fn get_user(
        &self,
        user_id: Option<Uuid>,
        username: Option<&str>,
        email: Option<&str>,
        token: Option<&str>,
    ) -> Result<Option<User>, sqlx::Error> {
        let mut user: Option<User> = None;

        // Find by user_id
        if let Some(user_id) = user_id {
            user = sqlx::query_as!(
                User,
                r#"SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" FROM users WHERE id = $1"#,
                user_id
            ).fetch_optional(&self.pool).await?;
            // fetch_optional returns Option<T>, fetch_one returns T, fetch_all returns Vec<T>, execute returns affected rows
        } else if let Some(username) = username {
            // Find by username
            user = sqlx::query_as!(
                User,
                r#"SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" FROM users WHERE username = $1"#,
                username
            ).fetch_optional(&self.pool).await?;
        } else if let Some(email) = email {
            // Find by email
            user = sqlx::query_as!(
                User,
                r#"SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" FROM users WHERE email = $1"#,
                email
            ).fetch_optional(&self.pool).await?;
        } else if let Some(token) = token {
            // Find by verification token (used in verify email and password reset flows)
            user = sqlx::query_as!(
                User,
                r#"
                SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" 
                FROM users 
                WHERE verification_token = $1"#,
                token
            )
            .fetch_optional(&self.pool)
            .await?;
        }

        Ok(user)
    }

    async fn get_users(&self, page: u32, limit: usize) -> Result<Vec<User>, sqlx::Error> {
        // Calculate OFFSET: page 1 = offset 0, page 2 = offset limit, etc.
        let offset = (page - 1) * limit as u32;

        let users = sqlx::query_as!(
            User,
            r#"SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" FROM users 
            ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
            limit as i64,
            offset as i64,
        ).fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    async fn save_user<T: Into<String> + Send>(
        &self,
        username: T,
        email: T,
        password: T,
        verification_token: T,
        token_expires_at: DateTime<Utc>,
    ) -> Result<User, sqlx::Error> {
        // Insert new user and return the created user record
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (username, email, password,verification_token, token_expires_at) 
            VALUES ($1, $2, $3, $4, $5) 
            RETURNING id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole"
            "#,
            username.into(),
            email.into(),
            password.into(),
            verification_token.into(),
            token_expires_at
        ).fetch_one(&self.pool)
        .await?;
        Ok(user)
    }

    async fn delete_user(&self, user_id: Uuid) -> Result<(), sqlx::Error> {
        let result = sqlx::query!("DELETE FROM users WHERE id = $1", user_id)
            .execute(&self.pool)
            .await?;

        // Check if user actually existed
        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn get_user_count(&self) -> Result<i64, sqlx::Error> {
        // COUNT(*) returns i64, wrapped in Option (hence unwrap_or(0))
        let count = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM users"#)
            .fetch_one(&self.pool)
            .await?;

        Ok(count.unwrap_or(0))
    }

    async fn update_user_name<T: Into<String> + Send>(
        &self,
        user_id: Uuid,
        new_username: T,
    ) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET username = $1, updated_at = Now()
            WHERE id = $2
            RETURNING id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole"
            "#,
            new_username.into(),
            user_id
        ).fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    // Separate update methods allow compile-time validation with query_as!
    // If combined with if/else branches, would need runtime validation with sqlx::query()

    async fn update_user_role(
        &self,
        user_id: Uuid,
        new_role: UserRole,
    ) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET role = $1, updated_at = Now()
            WHERE id = $2
            RETURNING id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole"
            "#,
            new_role as UserRole,
            user_id
        ).fetch_one(&self.pool)
       .await?;

        Ok(user)
    }

    async fn update_user_password(
        &self,
        user_id: Uuid,
        new_password: String,
    ) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET password = $1, updated_at = Now()
            WHERE id = $2
            RETURNING id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole"
            "#,
            new_password,
            user_id
        ).fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    async fn update_user_email(&self, user_id: Uuid, new_email: &str) -> Result<User, sqlx::Error> {
        let user = sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET email = $1, updated_at = Now()
            WHERE id = $2
            RETURNING id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole"
            "#,
            new_email,
            user_id
        ).fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    async fn verifed_token(&self, token: &str) -> Result<(), sqlx::Error> {
        // Mark email as verified and clear the verification token and expiration
        let _ = sqlx::query!(
            r#"
            UPDATE users
            SET verified = true, 
                updated_at = Now(),
                verification_token = NULL,
                token_expires_at = NULL
            WHERE verification_token = $1
            "#,
            token
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn add_verifed_token(
        &self,
        user_id: Uuid,
        token: &str,
        token_expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        // Store verification token for password reset or email change
        let _ = sqlx::query!(
            r#"
            UPDATE users
            SET verification_token = $1, token_expires_at = $2, updated_at = Now()
            WHERE id = $3
            "#,
            token,
            token_expires_at,
            user_id,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn check_email_duplicate(
        &self,
        user_id: Uuid,
        new_email: &str,
    ) -> Result<(), sqlx::Error> {
        // Check if email exists in database for a different user
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND id != $2)"#,
            new_email,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        // Return error if email is already in use
        if exists.unwrap_or(false) {
            return Err(sqlx::error::Error::Protocol("Email already exists".into()));
        }

        Ok(())
    }
}
