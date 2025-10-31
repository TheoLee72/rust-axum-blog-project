mod config;
mod db;
mod dtos;
mod error;
mod grpc;
mod handler;
mod http;
mod mail;
mod middleware;
mod models;
mod redisdb;
mod routes;
mod utils;

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

use axum_client_ip::{ClientIp, ClientIpSource};
use std::net::SocketAddr;

pub mod embed {
    tonic::include_proto!("embed"); // .proto 파일의 패키지명
}
use crate::grpc::GRPCClient;
use embed::embed_service_client::EmbedServiceClient;

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
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();

    dotenv().ok();

    let config = Config::init();

    let ip_source = if cfg!(debug_assertions) {
        ClientIpSource::ConnectInfo
    } else {
        ClientIpSource::CfConnectingIp
    };

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
            println!("🔥 Failed to connect to the database: {:?}", err);
            std::process::exit(1);
        }
    };

    let cors = CorsLayer::new()
        .allow_origin(config.frontend_url.parse::<HeaderValue>().unwrap())
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ]);

    let db_client = DBClient::new(pool);

    //scheduler
    db_client.start_cleanup_task().await;

    //redis
    let manager = redis::Client::open(config.redis_url.clone()) //일부러 &String이 &str으로 deref coercion되는거 막음.
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap(); //어짜피 연결실패하면 서버 꺼져.

    let redis_client = RedisClient::new(manager);

    //gRPC
    let embed_client = EmbedServiceClient::connect(config.grpc_url.clone())
        .await
        .unwrap();
    let grpc_client = GRPCClient { embed_client };

    //http
    let http_client = HttpClient {
        conn: reqwest::Client::new(),
    };

    let app_state = AppState {
        env: Arc::new(config.clone()),
        db_client,
        redis_client,
        grpc_client,
        http_client,
        ip_extraction: ip_source,
    };

    let app = routes::create_router(app_state).layer(cors.clone());
    //여기서 Arc 쓰는건 중복일 가능성이 있다. db_client, 즉 pool은 이미 Arc로 구현되어 있다.

    println!(
        "{}",
        format!("🚀 Server is running on http://localhost:{}", config.port)
    );

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", &config.port))
        .await
        .unwrap();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
