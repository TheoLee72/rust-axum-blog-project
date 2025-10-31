use super::DBClient;
use crate::dtos::{PostDto, PostPaginationDto};
use pgvector::Vector;
use uuid::Uuid;
pub trait PostExt {
    async fn get_post(&self, post_id: i32) -> Result<PostDto, sqlx::Error>;

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

    async fn delete_post(&self, user_id: Uuid, post_id: i32) -> Result<(), sqlx::Error>;

    async fn get_user_post_count(&self, user_username: &str) -> Result<i64, sqlx::Error>;

    async fn hybrid_search_posts(
        &self,
        query_text: &str,
        embedding: Vec<f32>,
        page: i32,
        limit: i32,
    ) -> Result<Vec<PostPaginationDto>, sqlx::Error>;

    async fn hybrid_search_posts_count(
        &self,
        query_text: &str,
        embedding: Vec<f32>,
    ) -> Result<i32, sqlx::Error>;

    async fn update_post_summary_and_embedding(
        &self,
        post_id: i32,
        summary: &str,
        embedding: Vec<f32>,
    ) -> Result<(), sqlx::Error>;
}
impl PostExt for DBClient {
    async fn get_post(&self, post_id: i32) -> Result<PostDto, sqlx::Error> {
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

        if posts.is_empty() {
            return Err(sqlx::Error::RowNotFound);
        }

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

    async fn delete_post(&self, user_id: Uuid, post_id: i32) -> Result<(), sqlx::Error> {
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

    async fn get_user_post_count(&self, user_username: &str) -> Result<i64, sqlx::Error> {
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
        page: i32,
        limit: i32,
    ) -> Result<Vec<PostPaginationDto>, sqlx::Error> {
        let embedding = Vector::from(embedding);
        let offset = (page - 1) * limit;

        let posts = sqlx::query_as!(
            PostPaginationDto,
            r#"
            SELECT p.id as "id!", u.username as "user_username!", p.summary as "summary!", p.title as "title!", p.created_at as "created_at!", p.updated_at as "updated_at!"
            FROM hybrid_search($1, $2, $3, $4) p
            JOIN users u ON p.user_id = u.id
            "#,
            query_text,
            embedding as _,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(posts)
    }

    async fn hybrid_search_posts_count(
        &self,
        query_text: &str,
        embedding: Vec<f32>,
    ) -> Result<i32, sqlx::Error> {
        let embedding = Vector::from(embedding);

        let count = sqlx::query_scalar!(
            r#"SELECT hybrid_search_count($1, $2)"#,
            query_text,
            embedding as _
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0) as i32)
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
