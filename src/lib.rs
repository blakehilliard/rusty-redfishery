use std::sync::{Arc, Mutex};
use axum::{
    extract::{
        Path,
        State,
    },
    http::StatusCode,
    routing::{get},
    response::{Json, IntoResponse},
    Router,
};
use tower_http::normalize_path::{NormalizePath, NormalizePathLayer};
use tower::layer::Layer;
use serde_json::{Value, json};

pub trait RedfishNode {
    fn get_uri(&self) -> &str;
    fn get_body(&self) -> serde_json::Value;
}

//TODO: Can I use simple pointer instead of Box?
pub fn app(
    node_getter: Arc<fn(&str) -> Option<Box<dyn RedfishNode>>>,
 ) -> NormalizePath<Router> {
    let state = AppState {
        node_getter: *node_getter,
    };
    let state = Arc::new(Mutex::new(state));

    let app = Router::new()
        .route("/redfish",
               get(get_redfish))
        .route("/redfish/*path",
               get(getter))
        .with_state(state);

    NormalizePathLayer::trim_trailing_slash()
        .layer(app)
}

struct AppState {
    node_getter: fn(&str) -> Option<Box<dyn RedfishNode>>,
}

async fn getter(
    Path(path): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    let uri = "/redfish/".to_owned() + &path;
    //FIXME: Does using a mutex here defeat the purpose of async?
    let state = state.lock().unwrap();
    if let Some(node) = (state.node_getter)(uri.as_str()) {
        return (StatusCode::OK, Json(node.get_body()));
    }
    (StatusCode::NOT_FOUND, Json(json!({"TODO": "FIXME"}))) //FIXME
}

async fn get_redfish() -> Json<Value> {
    Json(json!({ "v1": "/redfish/v1/" }))
}