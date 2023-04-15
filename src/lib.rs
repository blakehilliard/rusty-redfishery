use axum::{
    routing::{get, MethodRouter},
    response::Json,
    Router,
};
use tower_http::normalize_path::{NormalizePath, NormalizePathLayer};
use tower::layer::Layer;
use serde_json::{Value, json};

/// Create the Router object for axum.
///
/// You will need to pass this to layer() before using it.
/// But the calls are separate so you can attach custom states
/// and whatever else you wish before transforming it with the
/// redfish layers.
pub fn router(redfish_router: MethodRouter) -> Router {
    Router::new()
        .route("/redfish", get(get_redfish))
        .route("/redfish/*path", redfish_router)
}

/// Take the Router from router() above, return a new service
/// that adds a layer to normalize paths. This ensures that URIs
/// will be treated the same regardless of whether this is a trailing slash.
pub fn layer(router: Router) -> NormalizePath<Router> {
    NormalizePathLayer::trim_trailing_slash()
        .layer(router)
}

//FIXME: Need to figure out how to let people just use router() and not use this directly
pub async fn get_redfish() -> Json<Value> {
    Json(json!({ "v1": "/redfish/v1/" }))
}