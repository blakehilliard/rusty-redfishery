use axum::{
    routing::{get, MethodRouter},
    response::Json,
    Router,
};
use tower_http::normalize_path::{NormalizePath, NormalizePathLayer};
use tower::layer::Layer;
use serde_json::{Value, json};

pub fn app(redfish_router: MethodRouter) -> NormalizePath<Router> {
    let layer = NormalizePathLayer::trim_trailing_slash();
    let app = Router::new()
        .route("/redfish", get(get_redfish))
        .route("/redfish/*path", redfish_router);
    layer.layer(app)
}

async fn get_redfish() -> Json<Value> {
    Json(json!({ "v1": "/redfish/v1/" }))
}