use super::DBClient;
use tokio_cron_scheduler::{Job, JobScheduler};
impl DBClient {
    pub async fn start_cleanup_task(&self) {
        let sched = JobScheduler::new().await.unwrap();
        let pool = self.pool.clone();

        let job = Job::new_async("0 0 1 * * *", move |uuid, _l| {
            let pool = pool.clone();
            Box::pin(async move {
                println!("Running cleanup job {:?}", uuid);

                let result = sqlx::query!(
                    "DELETE FROM users
                WHERE verified = false
                    AND token_expires_at < NOW();"
                )
                .execute(&pool)
                .await;

                match result {
                    Ok(r) => {
                        println!(
                            "Cleanup job {:?} finished successfully, deleted {} rows",
                            uuid,
                            r.rows_affected()
                        );
                    }
                    Err(e) => {
                        eprintln!("Cleanup job {:?} failed: {:?}", uuid, e);
                    }
                }
            })
        })
        .unwrap();

        sched.add(job).await.unwrap();
        //It doesn't block.
        sched.start().await.unwrap();
    }
}
