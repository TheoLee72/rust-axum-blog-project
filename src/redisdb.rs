use std::net::IpAddr;

use redis::{AsyncCommands, aio::ConnectionManager};
#[derive(Clone)]
pub struct RedisClient {
    pub conn: ConnectionManager,
}

impl RedisClient {
    pub fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }

    pub async fn save_refresh_token(
        &self, //redis는 pool안쓰고 바로 connection을 직접 써서 연결하기 때문에 mut 써야됨.
        user_id: &str,
        refresh_token: &str,
        expires_in_seconds: i64,
    ) -> redis::RedisResult<()> {
        let key = format!("refresh:{}", user_id);
        let ttl_secs = expires_in_seconds;
        let mut conn = self.conn.clone(); //connectionmanager cloning is cheap. clone은 원래 immutable reference로 대상을 받음.
        conn.set_ex(key, refresh_token, ttl_secs as u64).await
    }

    pub async fn get_refresh_token(&self, user_id: &str) -> redis::RedisResult<Option<String>> {
        let key = format!("refresh:{}", user_id);
        let mut conn = self.conn.clone();
        conn.get(key).await
    }

    pub async fn delete_refresh_token(&self, user_id: &str) -> redis::RedisResult<()> {
        let key = format!("refresh:{}", user_id);
        let mut conn = self.conn.clone();
        conn.del(key).await
    }

    pub async fn get_ip_attempts(&self, ip: IpAddr) -> redis::RedisResult<Option<u32>> {
        let key = format!("login_fail_ip:{}", ip);
        let mut conn = self.conn.clone();
        conn.get(key).await
    }
    pub async fn get_identifier_ip_attempts(
        &self,
        ip: IpAddr,
        identifier: &str,
    ) -> redis::RedisResult<Option<u32>> {
        let key = format!("login_fail_identifier_ip:{}_{}", identifier, ip);
        let mut conn = self.conn.clone();
        conn.get(key).await
    }
    pub async fn delete_identifier_ip_attempts(
        &self,
        ip: IpAddr,
        identifier: &str,
    ) -> redis::RedisResult<()> {
        let key = format!("login_fail_identifier_ip:{}_{}", identifier, ip);
        let mut conn = self.conn.clone();
        conn.del(key).await
    }
    pub async fn increment_attempts(&self, ip: IpAddr, identifier: &str) -> redis::RedisResult<()> {
        let ip_key = format!("login_fail_ip:{}", ip);
        let identifier_ip_key = format!("login_fail_identifier_ip:{}_{}", identifier, ip);
        let mut conn = self.conn.clone();
        redis::pipe()
            .atomic()
            .incr(&ip_key, 1)
            .expire(&ip_key, 86400)
            .incr(&identifier_ip_key, 1)
            .expire(&identifier_ip_key, 3600)
            .query_async(&mut conn)
            .await
    }
}
