use sqlx::{Pool, Postgres};

pub mod scheduler;

mod newsletter;
pub use newsletter::NewsletterExt;

mod user;
pub use user::UserExt;

mod post;
pub use post::PostExt;

mod comment;
pub use comment::CommentExt;

#[derive(Debug, Clone)]
pub struct DBClient {
    pool: Pool<Postgres>,
}
impl DBClient {
    pub fn new(pool: Pool<Postgres>) -> Self {
        DBClient { pool }
    }
}
