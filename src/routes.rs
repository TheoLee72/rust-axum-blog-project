use axum::{Router, middleware};
use tower_http::trace::TraceLayer;

use crate::{
    AppState,
    handler::{
        auth::auth_handler, newsletter::newsletter_handler, post::post_handler,
        comment::comment_handler, search::search_handler, users::users_handler,
    },
    middleware::auth,
};

pub fn create_router(app_state: AppState) -> Router {
    let api_route = Router::new()
        .nest("/search", search_handler())
        .nest("/auth", auth_handler(app_state.clone()))
        .nest(
            "/users",
            users_handler().layer(middleware::from_fn_with_state(app_state.clone(), auth)), //여기서 middleware.rs에서 만든 middleware를 적용하는거임.
                                                                                            //nest는 prefix임. auth_handler()로 정의한 route까지 내려감. route를 더이상 못 내려감.
        )
        .nest("/posts", post_handler(app_state.clone()))
        .nest("/comments", comment_handler(app_state.clone()))
        .nest("/newsletter", newsletter_handler())
        .layer(TraceLayer::new_for_http())
        //layer는 미들웨어 적용
        .with_state(app_state);
    //app_state를 전역변수로 설정. Extension이 전역으로 접근할 수 있게 설정해줌. req.extension에 붙이는거임. 근데 이건 .route_layer는 불가능.
    //layer(Extension())했던걸 with_state로 바꿔버림. with_state가 더 최신에 나온 접근법인듯.

    Router::new().nest("/api", api_route)
}
