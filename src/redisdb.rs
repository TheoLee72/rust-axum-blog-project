use std::net::IpAddr;

use redis::{AsyncCommands, aio::ConnectionManager};

/// Redis client wrapper for caching and session management
///
/// This client handles:
/// - JWT refresh token storage (with automatic expiration)
/// - Login attempt tracking for rate limiting and security
/// - IP-based and identifier-based (email/username) tracking
///
/// Why use Redis?
/// - In-memory storage provides extremely fast read/write operations
/// - Built-in TTL (Time To Live) automatically expires old data
/// - Atomic operations prevent race conditions in concurrent environments
/// - Persistence options available for data durability
///
/// The `Clone` derive allows cheap cloning because ConnectionManager
/// uses Arc internally, making it safe to share across threads.
#[derive(Clone)]
pub struct RedisClient {
    pub conn: ConnectionManager,
}

impl RedisClient {
    /// Create a new RedisClient instance
    ///
    /// # Parameters
    /// - `conn`: Pre-configured ConnectionManager (established during app startup)
    pub fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }

    /// Store a refresh token for a user with automatic expiration
    ///
    /// Refresh tokens are stored separately from access tokens and have longer lifespans.
    /// When an access token expires, the client can use the refresh token to obtain
    /// a new access token without re-authentication.
    ///
    /// Key pattern: "refresh:{user_id}"
    ///
    /// # Parameters
    /// - `user_id`: User's unique identifier (typically a UUID string)
    /// - `refresh_token`: The JWT refresh token to store
    /// - `expires_in_seconds`: TTL for the token (e.g., 7 days = 604800 seconds)
    ///
    /// # Why clone ConnectionManager?
    /// Redis commands require a mutable reference, but `self` is immutable. (&mut self is impossible since app_state is immutable)
    /// Cloning ConnectionManager is cheap (it's Arc-based internally) and allows
    /// us to get mutable access without requiring &mut self.
    pub async fn save_refresh_token(
        &self,
        user_id: &str,
        refresh_token: &str,
        expires_in_seconds: i64,
    ) -> redis::RedisResult<()> {
        let key = format!("refresh:{}", user_id);
        let ttl_secs = expires_in_seconds;
        let mut conn = self.conn.clone(); // Cheap clone - ConnectionManager uses Arc internally

        // set_ex: Set key with expiration in one atomic operation
        // Redis will automatically delete this key after ttl_secs
        conn.set_ex(key, refresh_token, ttl_secs as u64).await
    }

    /// Retrieve a user's refresh token from Redis
    ///
    /// Returns None if:
    /// - Token was never stored
    /// - Token has expired (Redis auto-deleted it)
    /// - Token was manually deleted (logout)
    pub async fn get_refresh_token(&self, user_id: &str) -> redis::RedisResult<Option<String>> {
        let key = format!("refresh:{}", user_id);
        let mut conn = self.conn.clone();
        conn.get(key).await
    }

    /// Delete a user's refresh token (used during logout)
    ///
    /// This invalidates the refresh token, forcing re-authentication.
    /// This is crucial for security when a user logs out.
    pub async fn delete_refresh_token(&self, user_id: &str) -> redis::RedisResult<()> {
        let key = format!("refresh:{}", user_id);
        let mut conn = self.conn.clone();
        conn.del(key).await
    }

    /// Get total failed login attempts from an IP address
    ///
    /// This tracks all failed login attempts from a specific IP, regardless
    /// of which account they tried to access. Useful for detecting:
    /// - Brute force attacks from a single source
    /// - Credential stuffing attempts
    ///
    /// Key pattern: "login_fail_ip:{ip_address}"
    /// TTL: 24 hours (86400 seconds) - see increment_attempts()
    pub async fn get_ip_attempts(&self, ip: IpAddr) -> redis::RedisResult<Option<u32>> {
        let key = format!("login_fail_ip:{}", ip);
        let mut conn = self.conn.clone();
        conn.get(key).await
    }

    /// Get failed login attempts for a specific identifier (email/username) from an IP
    ///
    /// This tracks attempts to access a specific account from a specific IP.
    /// More granular than IP-only tracking. Useful for:
    /// - Account-specific rate limiting
    /// - Detecting targeted attacks on high-value accounts
    /// - Different thresholds per account vs per IP
    ///
    /// Key pattern: "login_fail_identifier_ip:{identifier}_{ip_address}"
    /// Example: "login_fail_identifier_ip:user@example.com_192.168.1.1"
    /// TTL: 1 hour (3600 seconds) - see increment_attempts()
    pub async fn get_identifier_ip_attempts(
        &self,
        ip: IpAddr,
        identifier: &str,
    ) -> redis::RedisResult<Option<u32>> {
        let key = format!("login_fail_identifier_ip:{}_{}", identifier, ip);
        let mut conn = self.conn.clone();
        conn.get(key).await
    }

    /// Delete failed login attempt counter for a specific identifier + IP combination
    ///
    /// Called after successful login to reset the counter for that specific
    /// account/IP combination. This allows legitimate users to continue
    /// logging in even if they had previous failed attempts.
    pub async fn delete_identifier_ip_attempts(
        &self,
        ip: IpAddr,
        identifier: &str,
    ) -> redis::RedisResult<()> {
        let key = format!("login_fail_identifier_ip:{}_{}", identifier, ip);
        let mut conn = self.conn.clone();
        conn.del(key).await
    }

    /// Increment failed login attempt counters atomically
    ///
    /// This increments both tracking metrics in a single atomic operation:
    /// 1. IP-based counter (24-hour TTL) - broader protection
    /// 2. Identifier+IP counter (1-hour TTL) - specific account protection
    ///
    /// Why use Redis pipeline?
    /// - `.atomic()`: Ensures all commands execute together (like a transaction)
    /// - Prevents race conditions when multiple login attempts happen simultaneously
    /// - More efficient than separate commands (single network round-trip)
    ///
    /// TTL Strategy:
    /// - IP tracking: 24 hours (86400s) - longer window for detecting persistent attacks
    /// - Identifier+IP: 1 hour (3600s) - shorter window, less restrictive for legitimate users
    ///
    /// # Security Note
    /// The asymmetric TTLs provide balanced protection:
    /// - Attackers targeting many accounts face IP-level blocking (24h)
    /// - Legitimate users who forget passwords aren't locked out long (1h per account)
    pub async fn increment_attempts(&self, ip: IpAddr, identifier: &str) -> redis::RedisResult<()> {
        let ip_key = format!("login_fail_ip:{}", ip);
        let identifier_ip_key = format!("login_fail_identifier_ip:{}_{}", identifier, ip);
        let mut conn = self.conn.clone();

        // Build an atomic pipeline to increment both counters and set/refresh TTLs
        redis::pipe()
            .atomic() // Execute all commands atomically
            .incr(&ip_key, 1) // Increment IP counter by 1
            .expire(&ip_key, 86400) // Set/refresh TTL to 24 hours
            .incr(&identifier_ip_key, 1) // Increment identifier+IP counter by 1
            .expire(&identifier_ip_key, 3600) // Set/refresh TTL to 1 hour
            .query_async(&mut conn) // Execute pipeline asynchronously
            .await
    }
}
