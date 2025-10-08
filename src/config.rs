#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_maxage: i64,
    pub refresh_token_maxage: i64,
    pub redis_url: String,
    pub port: u16,
    pub llm_url: String,
    pub model_name: String,
    pub grpc_url: String,
    pub frontend_url: String,
}

impl Config {

    pub fn init() -> Config {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let jwt_secret = std::env::var("JWT_SECRET_KEY").expect("JWT_SECRET_KEY must be set");
        let jwt_maxage = std::env::var("JWT_MAXAGE").expect("JWT_MAXAGE must be set");
        let refresh_token_maxage = std::env::var("REFRESH_TOKEN_MAXAGE").expect("REFRESH_TOKEN_MAXAGE must be set");
        let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
        let llm_url = std::env::var("LLM_URL").expect("LLM_URL must be set");
        let model_name = std::env::var("MODEL_NAME").expect("MODEL_NAME must be set");
        let grpc_url = std::env::var("GRPC_URL").expect("GRPC_URL must be set");
        let frontend_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL must be set");

        Config {
            database_url,
            jwt_secret,
            jwt_maxage: jwt_maxage.parse::<i64>().unwrap(),
            refresh_token_maxage: refresh_token_maxage.parse::<i64>().unwrap(),
            redis_url,
            port: 8000,
            llm_url,
            model_name,
            grpc_url,
            frontend_url,
        }
    }
    
}