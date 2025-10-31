// Module declarations - each module handles a specific domain of the application
mod config; // Application configuration (env variables, settings)
mod db; // Database client and connection pool management
mod dtos; // Data Transfer Objects for request/response serialization
mod error; // Custom error types and error handling
mod grpc; // gRPC client for communicating with embedding service
mod handler; // Request handlers (business logic for each endpoint)
mod http; // HTTP client wrapper for external API calls
mod mail; // Email sending functionality
mod middleware; // Custom middleware (auth, role_check etc.)
mod models; // Database models representing table structures
mod redisdb; // Redis client for session storage and managing login attempts
mod routes; // Route definitions and router configuration
mod utils; // Utility functions and helpers (password, token)

use axum::http::{
    HeaderValue, Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use config::Config;
use db::DBClient;
use dotenv::dotenv;
use http::HttpClient;
use redisdb::RedisClient;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::filter::LevelFilter;

use axum_client_ip::ClientIpSource;
use std::net::SocketAddr;

// gRPC proto file integration
// This module contains auto-generated code from the .proto file
pub mod embed {
    tonic::include_proto!("embed"); // Package name from the .proto file
}
use crate::grpc::GRPCClient;
use embed::embed_service_client::EmbedServiceClient;

/// Application state shared across all request handlers
///
/// This struct holds all dependencies needed by route handlers.
/// It's cloned for each request (cheaply, due to Arc and internal Arc usage).
///
/// Key components:
/// - `env`: Application configuration loaded from environment variables
/// - `db_client`: PostgreSQL connection pool for database operations
/// - `redis_client`: Redis connection for caching and session management
/// - `grpc_client`: Client for vector embedding service
/// - `http_client`: HTTP client for making external API requests
/// - `ip_extraction`: Strategy for extracting client IP (varies by deployment)
#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Config>,
    pub db_client: db::DBClient,
    pub redis_client: redisdb::RedisClient,
    pub grpc_client: grpc::GRPCClient,
    pub http_client: http::HttpClient,
    pub ip_extraction: ClientIpSource,
}

#[tokio::main]
async fn main() {
    // Initialize tracing for structured logging
    // DEBUG level provides detailed information during development
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();

    // Load environment variables from .env file (if it exists)
    // This is useful for local development
    dotenv().ok();

    // Load application configuration from environment variables
    let config = Config::init();

    // Determine IP extraction strategy based on build configuration
    // - In debug mode (local development): extract from socket connection info
    // - In release mode (production with Cloudflare): use CF-Connecting-IP header
    let ip_source = if cfg!(debug_assertions) {
        ClientIpSource::ConnectInfo
    } else {
        ClientIpSource::CfConnectingIp
    };

    // Create PostgreSQL connection pool
    // max_connections(10): Limits concurrent database connections to prevent overload
    // Connection pooling improves performance by reusing connections
    let pool = match PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
    {
        Ok(pool) => {
            println!("✅Connection to the database is successful!");
            pool
        }
        Err(err) => {
            // Fatal error: cannot start server without database
            println!("🔥 Failed to connect to the database: {:?}", err);
            std::process::exit(1);
        }
    };

    // Configure CORS (Cross-Origin Resource Sharing)
    // This allows the frontend to make requests from a different origin
    // Essential for modern web applications with separate frontend/backend
    let cors = CorsLayer::new()
        .allow_origin(config.frontend_url.parse::<HeaderValue>().unwrap())
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
        .allow_credentials(true) // Allow cookies and authorization headers
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS, // Preflight requests
        ]);

    // Initialize database client wrapper
    let db_client = DBClient::new(pool);

    // Start background task for periodic cleanup operations
    // Example: removing not verified accounts, etc.
    db_client.start_cleanup_task().await;

    // Initialize Redis connection
    let manager = redis::Client::open(config.redis_url.clone())
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap(); // Server cannot function without Redis, so panic is acceptable

    let redis_client = RedisClient::new(manager);

    // Initialize gRPC client for embedding service
    // This service converts text to vector embeddings for semantic search
    let embed_client = EmbedServiceClient::connect(config.grpc_url.clone())
        .await
        .unwrap();
    let grpc_client = GRPCClient { embed_client };

    // Initialize HTTP client for external API calls
    // reqwest::Client maintains a connection pool internally
    let http_client = HttpClient {
        conn: reqwest::Client::new(),
    };

    // Assemble application state with all initialized components
    // This state will be cloned and passed to each request handler
    let app_state = AppState {
        env: Arc::new(config.clone()),
        db_client,
        redis_client,
        grpc_client,
        http_client,
        ip_extraction: ip_source,
    };

    // Create the main router with all routes and apply CORS middleware
    // Note: Wrapping in Arc might be redundant here since db_client's pool
    // is already Arc-based internally.
    let app = routes::create_router(app_state).layer(cors.clone());

    println!(
        "{}",
        format!("🚀 Server is running on http://localhost:{}", config.port)
    );

    // Bind TCP listener to all interfaces (0.0.0.0) on the configured port
    // This allows connections from any network interface
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", &config.port))
        .await
        .unwrap();

    // Start the Axum server
    // into_make_service_with_connect_info captures socket address for IP extraction
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
