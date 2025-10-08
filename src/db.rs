use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};
use uuid::Uuid;
use pgvector::Vector;

use crate::models::{User, UserRole};
use crate::dtos::{PostDto, PostPaginationDto, ReviewDto};

#[derive(Debug, Clone)]
pub struct DBClient {
    pool: Pool<Postgres>,
}
//pool 은 main.rs에서 만들어줌
impl DBClient {
    pub fn new(pool: Pool<Postgres>) -> Self {
        DBClient { pool }
    }
}
//async_trait을 trait정의랑, 구현부분 둘다 써야됨. 
//그래야 async fn을 쓸 수 있음.
//최신 Rust + 단일 스레드/dyn 안 쓰는 구조 → #[async_trait] 없이 가능
//멀티스레드 + dyn Trait 필요 → 여전히 #[async_trait] 쓰는 게 안전
//async문법은 rust가 제공하는데 이를 실행하는 런타임이 없음. 직접 pool하는 코드를 짜야함.
//이 런타임을 tokio가 제공. 
//rust최신버전에서는 async_trait안쓰고 그냥 async써도 method에서 가능. 단 Send만 잘 신경써주면 됨. (관련된 모든게 send)
//(rc, refcell 이런거 쓰지 마라)

pub trait UserExt {
    async fn get_user(
        &self,
        user_id: Option<Uuid>,
        username: Option<&str>,
        email: Option<&str>,
        token: Option<&str>,
    ) -> Result<Option<User>, sqlx::Error>;

    async fn get_users(
        &self,
        page: u32,
        limit: usize,
    ) -> Result<Vec<User>, sqlx::Error>;

    async fn save_user<T: Into<String> + Send>(
        &self,
        username: T,
        email: T,
        password: T,
        verification_token: T,
        token_expires_at: DateTime<Utc>,
    ) -> Result<User, sqlx::Error>;

    async fn delete_user(
        &self,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error>;

    async fn get_user_count(&self) -> Result<i64, sqlx::Error>;

    async fn update_user_name<T: Into<String> + Send>(
        &self,
        user_id: Uuid,
        new_username: T,
    ) -> Result<User, sqlx::Error>;

    async fn update_user_role(
        &self,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<User, sqlx::Error>;

    async fn update_user_password(
        &self,
        user_id: Uuid,
        password: String,
    ) -> Result<User, sqlx::Error>;

    async fn verifed_token(
        &self,
        token: &str,
    ) -> Result<(), sqlx::Error>;

    async fn add_verifed_token(
        &self,
        user_id: Uuid,
        token: &str,
        expires_at: DateTime<Utc>,
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

        if let Some(user_id) = user_id {
            user = sqlx::query_as!(
                User,
                r#"SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" FROM users WHERE id = $1"#,
                user_id
            ).fetch_optional(&self.pool).await?; //fetch_optional, fetch_one, fetch_all, execute 있음, fetch시리즈는 SELECT할 때, execute는 db바꾸기만 할 때
        } else if let Some(username) = username {
            user = sqlx::query_as!(
                User,
                r#"SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" FROM users WHERE username = $1"#,
                username
            ).fetch_optional(&self.pool).await?;
        } else if let Some(email) = email {
            user = sqlx::query_as!(
                User,
                r#"SELECT id, username, email, password, verified, created_at, updated_at, verification_token, token_expires_at, role as "role: UserRole" FROM users WHERE email = $1"#,
                email
            ).fetch_optional(&self.pool).await?;
        } else if let Some(token) = token {
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

    async fn get_users(
        &self,
        page: u32,
        limit: usize,
    ) -> Result<Vec<User>, sqlx::Error> {
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
        let result = sqlx::query!(
            "DELETE FROM users WHERE id = $1",
            user_id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn get_user_count(&self) -> Result<i64, sqlx::Error> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM users"#
        )
       .fetch_one(&self.pool)
       .await?;

        Ok(count.unwrap_or(0))
    }

    async fn update_user_name<T: Into<String> + Send>(
        &self,
        user_id: Uuid,
        new_username: T
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
    //이렇게 update logic을 쪼개놓은건 query_as로 컴파일타임에 검증을 하려고, 
    //한번에 update하게 바꾸면 if분기로 조건마다 다르게 처리해야하니, query_as!못쓰고
    //sqlx::query()이거로 런타임 검증만 가능
    

    async fn update_user_role(
        &self,
        user_id: Uuid,
        new_role: UserRole
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
        new_password: String
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

    async fn verifed_token(
        &self,
        token: &str,
    ) -> Result<(), sqlx::Error> {
        let _ =sqlx::query!(
            r#"
            UPDATE users
            SET verified = true, 
                updated_at = Now(),
                verification_token = NULL,
                token_expires_at = NULL
            WHERE verification_token = $1
            "#,
            token
        ).execute(&self.pool)
       .await?;

        Ok(())
    }

    async fn add_verifed_token(
        &self,
        user_id: Uuid,
        token: &str,
        token_expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let _ = sqlx::query!(
            r#"
            UPDATE users
            SET verification_token = $1, token_expires_at = $2, updated_at = Now()
            WHERE id = $3
            "#,
            token,
            token_expires_at,
            user_id,
        ).execute(&self.pool)
       .await?;

        Ok(())
    }
}

pub trait PostExt {
    async fn get_post(
        &self,
        post_id: i32,
    ) -> Result<PostDto, sqlx::Error>;

    async fn get_posts(
        &self,
        page: i32,
        limit: i32,
        user_username: &str,
    ) -> Result<Vec<PostPaginationDto>, sqlx::Error>;

    async fn create_post(
        &self,
        user_id: Uuid,
        content: &str,
        title: &str,
        raw_text: &str,
        summary: &str,
        embedding: Vec<f32>,
    ) -> Result<PostDto, sqlx::Error>;

    async fn edit_post(
        &self,
        user_id: Uuid,
        post_id: i32,
        content: &str,
        title: &str,
        raw_text: &str,
    ) -> Result<PostDto, sqlx::Error>;

    async fn delete_post(
        &self,
        user_id: Uuid,
        post_id: i32,
    ) -> Result<(), sqlx::Error>;

    async fn get_user_post_count(
        &self,
        user_username: &str,
    ) -> Result<i64, sqlx::Error>;

    async fn hybrid_search_posts(
        &self,
        query_text: &str,
        embedding: Vec<f32>,
    ) -> Result<Vec<PostPaginationDto>, sqlx::Error>;

    async fn update_post_summary_and_embedding(
        &self,
        post_id: i32,
        summary: &str,
        embedding: Vec<f32>,
    ) -> Result<(), sqlx::Error>;
}

impl PostExt for DBClient{
    async fn get_post(
        &self,
        post_id: i32,
    ) -> Result<PostDto, sqlx::Error> {
        let post = sqlx::query_as!(
            PostDto,
            r#"
            SELECT p.id, u.username as "user_username", p.content, p.summary, p.title, p.created_at, p.updated_at
            FROM post p
            INNER JOIN users u ON p.user_id = u.id
            WHERE p.id = $1
            "#,
            post_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(post)
    }

    async fn get_posts(
        &self,
        page: i32,
        limit: i32,
        user_username: &str,
    ) -> Result<Vec<PostPaginationDto>, sqlx::Error> {
        let offset = (page - 1) * limit;

        let posts = sqlx::query_as!(
            PostPaginationDto,
            r#"
            SELECT p.id, u.username as "user_username", p.summary, p.title, p.created_at, p.updated_at
            FROM post p
            INNER JOIN users u ON p.user_id = u.id
            WHERE u.username = $1
            ORDER BY p.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_username,
            limit as i64,
            offset as i64
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(posts)
    }

    async fn create_post(
        &self,
        user_id: Uuid,
        content: &str,
        title: &str,
        raw_text: &str,
        summary: &str,
        embedding: Vec<f32>,
    ) -> Result<PostDto, sqlx::Error> {
        let embedding = Vector::from(embedding);
        let post = sqlx::query_as!(
            PostDto,
            r#"
            WITH new_post AS (
                INSERT INTO post (user_id, content, title, raw_text, summary, embedding)
                VALUES ($1, $2, $3, $4, $5, $6::vector)
                RETURNING id, user_id, content, summary, title, created_at, updated_at
            )
            SELECT
                np.id,
                u.username as "user_username",
                np.content,
                np.summary,
                np.title,
                np.created_at,
                np.updated_at
            FROM new_post np
            JOIN users u ON np.user_id = u.id
            "#,
            user_id,
            content,
            title,
            raw_text,
            summary,
            embedding as _,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(post)
    }

    async fn edit_post(
        &self,
        user_id: Uuid,
        post_id: i32,
        content: &str,
        title: &str,
        raw_text: &str,
    ) -> Result<PostDto, sqlx::Error> {
        let post = sqlx::query_as!(
            PostDto,
            r#"
            WITH updated_post AS (
                UPDATE post
                SET content = $1, title = $2, raw_text = $3, updated_at = NOW()
                WHERE id = $4 AND user_id = $5
                RETURNING *
            )
            SELECT
                up.id,
                u.username as "user_username",
                up.content,
                up.summary,
                up.title,
                up.created_at,
                up.updated_at
            FROM updated_post up
            JOIN users u ON up.user_id = u.id
            "#,
            content,
            title,
            raw_text,
            post_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(post)
    }

    async fn delete_post(
        &self,
        user_id: Uuid,
        post_id: i32,
    ) -> Result<(), sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM post WHERE id = $1 AND user_id = $2",
            post_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn get_user_post_count(
            &self,
            user_username: &str,
        ) -> Result<i64, sqlx::Error> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(p.id)
            FROM post p
            INNER JOIN users u ON p.user_id = u.id
            WHERE u.username = $1
            "#,
            user_username
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0))
    }

    async fn hybrid_search_posts(
        &self,
        query_text: &str,
        embedding: Vec<f32>,
    ) -> Result<Vec<PostPaginationDto>, sqlx::Error> {
        let embedding = Vector::from(embedding);
        let match_count: i32 = 10;

        let posts = sqlx::query_as!(
            PostPaginationDto,
            r#"
            SELECT p.id as "id!", u.username as "user_username!", p.summary as "summary!", p.title as "title!", p.created_at as "created_at!", p.updated_at as "updated_at!"
            FROM hybrid_search($1, $2, $3) p
            JOIN users u ON p.user_id = u.id
            "#,
            query_text,
            embedding as _,
            match_count
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(posts)
    }

    async fn update_post_summary_and_embedding(
        &self,
        post_id: i32,
        summary: &str,
        embedding: Vec<f32>,
    ) -> Result<(), sqlx::Error> {
        let embedding = Vector::from(embedding);
        sqlx::query!(
            r#"
            UPDATE post
            SET summary = $1, embedding = $2::vector, updated_at = NOW()
            WHERE id = $3
            "#,
            summary,
            embedding as _,
            post_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

}
pub trait ReviewExt {
    async fn get_reviews(
        &self,
        post_id: i32,
        page: i32,
        limit: i32,
        sort: &str,
    ) -> Result<Vec<ReviewDto>, sqlx::Error>;

    async fn create_review(
        &self,
        user_id: Uuid,
        post_id: i32,
        content: &str,
    ) -> Result<ReviewDto, sqlx::Error>;

    async fn edit_review(
        &self,
        user_id: Uuid,
        review_id: i32,
        content: &str,
    ) -> Result<ReviewDto, sqlx::Error>;

    async fn delete_review(
        &self,
        user_id: Uuid,
        review_id: i32,
    ) -> Result<(), sqlx::Error>;

    async fn get_post_review_count(
        &self,
        post_id: i32,
    ) -> Result<i64, sqlx::Error>;

}
impl ReviewExt for DBClient {
    async fn get_reviews(
        &self,
        post_id: i32,
        page: i32,
        limit: i32,
        sort: &str,
    ) -> Result<Vec<ReviewDto>, sqlx::Error> {
        let offset = (page - 1) * limit;
        let order_by = if sort == "created_at_asc" { "r.created_at ASC" } else { "r.created_at DESC" };

        let query = format!(
            r#"
            SELECT r.id, u.username as "user_username", r.post_id, r.content, r.created_at, r.updated_at
            FROM review r
            INNER JOIN users u ON r.user_id = u.id
            WHERE r.post_id = $1
            ORDER BY {}
            LIMIT $2 OFFSET $3
            "#,
            order_by
        );

        let reviews = sqlx::query_as(&query)
            .bind(post_id)
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        Ok(reviews)
    }

    async fn create_review(
        &self,
        user_id: Uuid,
        post_id: i32,
        content: &str,
    ) -> Result<ReviewDto, sqlx::Error> {
        let review = sqlx::query_as!(
            ReviewDto,
            r#"
            WITH new_review AS (
                INSERT INTO review (user_id, post_id, content)
                VALUES ($1, $2, $3)
                RETURNING *
            )
            SELECT
                nr.id,
                u.username as "user_username",
                nr.post_id,
                nr.content,
                nr.created_at,
                nr.updated_at
            FROM new_review nr
            JOIN users u ON nr.user_id = u.id
            "#,
            user_id,
            post_id,
            content
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(review)
    }

    async fn edit_review(
        &self,
        user_id: Uuid,
        review_id: i32,
        content: &str,
    ) -> Result<ReviewDto, sqlx::Error> {
        let review = sqlx::query_as!(
            ReviewDto,
            r#"
            WITH updated_review AS (
                UPDATE review
                SET content = $1, updated_at = NOW()
                WHERE id = $2 AND user_id = $3
                RETURNING *
            )
            SELECT
                ur.id,
                u.username as "user_username",
                ur.post_id,
                ur.content,
                ur.created_at,
                ur.updated_at
            FROM updated_review ur
            JOIN users u ON ur.user_id = u.id
            "#,
            content,
            review_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(review)
    }

    async fn delete_review(
        &self,
        user_id: Uuid,
        review_id: i32,
    ) -> Result<(), sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM review WHERE id = $1 AND user_id = $2",
            review_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn get_post_review_count(
        &self,
        post_id: i32,
    ) -> Result<i64, sqlx::Error> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(id)
            FROM review
            WHERE post_id = $1
            "#,
            post_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0))
    }
}

