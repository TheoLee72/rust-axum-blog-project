use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: String,
    pub message: String,
}

impl fmt::Display for ErrorResponse {
    //{}에 어떻게 보여질지 정하는 것
    //serde_json::to_string하면 struct를 json 직렬화하는 것처럼 만들어줌. 
    //f가 println!하기 전 버퍼라는데
    //버퍼에 써야하니까 &mut

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //fmt::Result는 문제 없으면 Ok(()), 문제있으면  Err(fmt::Error)
        //f에 쓰겠다. write!는 버퍼에 쓰는 macro, println!은 버퍼에서 가져와서 출력하는 macro
        //write!(f, "{}", serde_json::to_string(self).unwrap())
        match serde_json::to_string(self) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => Err(fmt::Error)
        }

    }
}

#[derive(Debug, PartialEq)]
pub enum ErrorMessage {
    EmptyPassword,
    ExceededMaxPasswordLength(usize),
    InvalidHashFormat,
    HashingError,
    InvalidToken,
    WrongCredentials,
    EmailExist,
    UserNoLongerExist,
    TokenNotProvided,
    PermissionDenied,
    UserNotAuthenticated,
}

impl ToString for ErrorMessage {
    fn to_string(&self) -> String {
        match self {
            ErrorMessage::WrongCredentials => "Email or password is wrong".to_string(),
            ErrorMessage::EmailExist => "A user with this email already exists".to_string(),
            ErrorMessage::UserNoLongerExist => "User belonging to this token no longer exists".to_string(),
            ErrorMessage::EmptyPassword => "Password cannot be empty".to_string(),
            ErrorMessage::HashingError => "Error while hashing password".to_string(),
            ErrorMessage::InvalidHashFormat => "Invalid password hash format".to_string(),
            ErrorMessage::ExceededMaxPasswordLength(max_length) => format!("Password must not be more than {} characters", max_length),
            ErrorMessage::InvalidToken => "Authentication token is invalid or expired".to_string(),
            ErrorMessage::TokenNotProvided => "You are not logged in, please provide a token".to_string(),
            ErrorMessage::PermissionDenied => "You are not allowed to perform this action".to_string(),
            ErrorMessage::UserNotAuthenticated => "Authentication required. Please log in.".to_string(),
        }
    }
}


#[derive(Debug,Clone)]
pub struct HttpError {
    pub message: String,
    pub status: StatusCode,
}
//이건 내부에서 쓰는 errortype

impl HttpError {
    //Into<String>은 string으로 바뀔 수 있는 type 다 받는다는 거임. 
    //from이랑 into가 보통 같이 쌍으로 다님. 
    pub fn new(message: impl Into<String>, status: StatusCode) -> Self {
        HttpError {
            message: message.into(),
            status,
        }
    }

    pub fn server_error(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::BAD_REQUEST,
        }
    }

    pub fn unique_constraint_violation(message: impl Into<String>) -> Self {
        HttpError { 
            message: message.into(), 
            status: StatusCode::CONFLICT 
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        HttpError {
            message: message.into(),
            status: StatusCode::UNAUTHORIZED,
        }
    }

    pub fn into_http_response(self) -> Response {
        let json_response = Json(ErrorResponse {
            status: "fail".to_string(),
            message: self.message.clone(),
        });

        (self.status, json_response).into_response()
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HttpError: message: {}, status: {}",
            self.message, self.status
        )
    }
}
//rust 통합 error에 이제 httperror가 포함됨.
//그런데 왜 errorresponse는 안했냐? -> 이건 client에 응답을 보내는 용도이기 때문에.

impl std::error::Error for HttpError {}
//이 type을 http 응답으로 바꿀 수 있다. 
impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        self.into_http_response()
    }
}
