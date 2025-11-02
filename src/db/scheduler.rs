use super::DBClient;
use tokio_cron_scheduler::{Job, JobScheduler};

impl DBClient {
    /// Start background cleanup task that runs on a schedule
    ///
    /// Removes unverified users whose verification tokens have expired.
    /// This prevents accumulation of inactive registration attempts.
    pub async fn start_cleanup_task(&self) {
        // Create a new job scheduler for managing cron jobs
        let sched = JobScheduler::new().await.unwrap();

        // **First clone: Move pool into the closure**
        // We need to clone pool here because:
        // 1. self.pool is part of &self (borrowed reference)
        // 2. The closure needs to take ownership of the pool to move it into the async block
        // 3. We can't move &self into the closure (self reference would outlive the method)
        // 4. SqlxPool uses Arc internally, so cloning is cheap (just increments ref count)
        let pool = self.pool.clone();

        // Create cron job with schedule "0 0 1 * * *" (1 AM on first day of each month)
        // Cron format: second minute hour day month day_of_week
        let job = Job::new_async("0 0 1 * * *", move |uuid, _l| {
            // **Second clone: Move pool into the async block**
            // We need to clone pool again because:
            // 1. The outer closure captured `pool` with `move` (took ownership)
            // 2. Each time the job runs (every month), it needs a copy of pool
            // 3. If we used `pool` directly, the first execution would consume it
            // 4. Cloning allows the job to run repeatedly without issues
            // 5. The closure is invoked multiple times over the scheduler's lifetime
            // 6. Without cloning, move semantics would prevent reuse
            let pool = pool.clone();

            Box::pin(async move {
                tracing::info!("Running cleanup job {:?}", uuid);

                // Delete unverified users whose verification tokens have expired
                // Now we have owned access to pool for this specific execution
                let result = sqlx::query!(
                    "DELETE FROM users
                WHERE verified = false
                    AND token_expires_at < NOW();"
                )
                .execute(&pool)
                .await;

                // Log result of cleanup job
                match result {
                    Ok(r) => {
                        tracing::info!(
                            "Cleanup job {:?} finished successfully, deleted {} rows",
                            uuid,
                            r.rows_affected()
                        );
                    }
                    Err(e) => {
                        tracing::error!("Cleanup job {:?} failed: {}", uuid, e);
                    }
                }
            })
        })
        .unwrap();

        // Add the job to the scheduler
        sched.add(job).await.unwrap();
        // Start the scheduler (runs in background, doesn't block)
        // The job will execute repeatedly according to the cron schedule
        sched.start().await.unwrap();
    }
}
