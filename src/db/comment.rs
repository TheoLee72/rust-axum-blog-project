use super::DBClient;
use crate::dtos::CommentDto;
use uuid::Uuid;
pub trait CommentExt {
    async fn get_comments(
        &self,
        post_id: i32,
        page: i32,
        limit: i32,
        sort: &str,
    ) -> Result<Vec<CommentDto>, sqlx::Error>;

    async fn create_comment(
        &self,
        user_id: Uuid,
        post_id: i32,
        content: &str,
    ) -> Result<CommentDto, sqlx::Error>;

    async fn edit_comment(
        &self,
        user_id: Uuid,
        comment_id: i32,
        content: &str,
    ) -> Result<CommentDto, sqlx::Error>;

    async fn delete_comment(&self, user_id: Uuid, comment_id: i32) -> Result<(), sqlx::Error>;

    async fn get_post_comment_count(&self, post_id: i32) -> Result<i64, sqlx::Error>;

    async fn get_user_comment_count(&self, user_id: &Uuid) -> Result<i64, sqlx::Error>;
}
impl CommentExt for DBClient {
    async fn get_comments(
        &self,
        post_id: i32,
        page: i32,
        limit: i32,
        sort: &str,
    ) -> Result<Vec<CommentDto>, sqlx::Error> {
        let offset = (page - 1) * limit;
        let order_by = if sort == "created_at_asc" {
            "r.created_at ASC"
        } else {
            "r.created_at DESC"
        };

        let query = format!(
            r#"
            SELECT r.id, u.username as "user_username", r.post_id, r.content, r.created_at, r.updated_at
            FROM comment r
            INNER JOIN users u ON r.user_id = u.id
            WHERE r.post_id = $1
            ORDER BY {}
            LIMIT $2 OFFSET $3
            "#,
            order_by
        );

        let comments = sqlx::query_as(&query)
            .bind(post_id)
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        Ok(comments)
    }

    async fn create_comment(
        &self,
        user_id: Uuid,
        post_id: i32,
        content: &str,
    ) -> Result<CommentDto, sqlx::Error> {
        let comment = sqlx::query_as!(
            CommentDto,
            r#"
            WITH new_comment AS (
                INSERT INTO comment (user_id, post_id, content)
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
            FROM new_comment nr
            JOIN users u ON nr.user_id = u.id
            "#,
            user_id,
            post_id,
            content
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(comment)
    }

    async fn edit_comment(
        &self,
        user_id: Uuid,
        comment_id: i32,
        content: &str,
    ) -> Result<CommentDto, sqlx::Error> {
        let comment = sqlx::query_as!(
            CommentDto,
            r#"
            WITH updated_comment AS (
                UPDATE comment
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
            FROM updated_comment ur
            JOIN users u ON ur.user_id = u.id
            "#,
            content,
            comment_id,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(comment)
    }

    async fn delete_comment(&self, user_id: Uuid, comment_id: i32) -> Result<(), sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM comment WHERE id = $1 AND user_id = $2",
            comment_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn get_post_comment_count(&self, post_id: i32) -> Result<i64, sqlx::Error> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(id)
            FROM comment
            WHERE post_id = $1
            "#,
            post_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0))
    }

    async fn get_user_comment_count(&self, user_id: &Uuid) -> Result<i64, sqlx::Error> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)
            FROM comment
            WHERE user_id = $1
            "#,
            user_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0))
    }
}
