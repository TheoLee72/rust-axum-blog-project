use crate::models::NewsletterEmail;

use super::DBClient;

/// Newsletter subscription database operations trait
pub trait NewsletterExt {
    /// Add email to newsletter subscribers
    async fn add_newsletter_email(&self, email: &str) -> Result<NewsletterEmail, sqlx::Error>;

    /// Remove email from newsletter subscribers
    async fn delete_newsletter_email(&self, email: &str) -> Result<(), sqlx::Error>;

    /// Get all newsletter subscriber emails
    async fn get_all_newsletter_emails(&self) -> Result<Vec<NewsletterEmail>, sqlx::Error>;
}

impl NewsletterExt for DBClient {
    async fn add_newsletter_email(&self, email: &str) -> Result<NewsletterEmail, sqlx::Error> {
        // Insert email and return the created record
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
        // Delete email from subscribers
        let result = sqlx::query!("DELETE FROM newsletter_emails WHERE email = $1", email)
            .execute(&self.pool)
            .await?;

        // Return RowNotFound if email doesn't exist
        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn get_all_newsletter_emails(&self) -> Result<Vec<NewsletterEmail>, sqlx::Error> {
        // Fetch all newsletter subscribers
        let emails = sqlx::query_as!(
            NewsletterEmail,
            "SELECT id, email, created_at FROM newsletter_emails"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(emails)
    }
}
