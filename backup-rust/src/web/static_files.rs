use axum::{
    routing::get_service,
    Router,
};
use tower_http::services::ServeDir;
use tower::ServiceExt;

pub fn routes<S>() -> Router<S> 
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .fallback_service(get_service(ServeDir::new("static")).handle_error(|_| async {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error",
            )
        }))
}

pub fn static_handler() -> axum::routing::MethodRouter {
    get_service(ServeDir::new("static")).handle_error(|_| async {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error",
        )
    })
}