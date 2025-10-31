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
