use crate::models::NewsletterEmail;
use uuid::Uuid;

use super::DBClient;

pub trait NewsletterExt {
    async fn add_newsletter_email(&self, email: &str) -> Result<NewsletterEmail, sqlx::Error>;
    async fn delete_newsletter_email(&self, email: &str) -> Result<(), sqlx::Error>;
    async fn get_all_newsletter_emails(&self) -> Result<Vec<NewsletterEmail>, sqlx::Error>;
}

impl NewsletterExt for DBClient {
    async fn add_newsletter_email(&self, email: &str) -> Result<NewsletterEmail, sqlx::Error> {
        let newsletter_email = sqlx::query_as!(
            NewsletterEmail,
            r#"
            INSERT INTO newsletter_emails (email)
            VALUES ($1)
            RETURNING id, email, created_at
            "#,
            email
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(newsletter_email)
    }

    async fn delete_newsletter_email(&self, email: &str) -> Result<(), sqlx::Error> {
        let result = sqlx::query!("DELETE FROM newsletter_emails WHERE email = $1", email)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn get_all_newsletter_emails(&self) -> Result<Vec<NewsletterEmail>, sqlx::Error> {
        let emails = sqlx::query_as!(
            NewsletterEmail,
            "SELECT id, email, created_at FROM newsletter_emails"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(emails)
    }
}
