use std::sync::{Arc, Mutex};
use axum::{
    extract::{
        Path,
        State,
    },
    http::StatusCode,
    routing::{get},
    response::{Json, Response, IntoResponse},
    Router,
};
use tower_http::normalize_path::{NormalizePath, NormalizePathLayer};
use tower::layer::Layer;
use serde_json::{Value, json};

pub trait RedfishNode {
    fn get_uri(&self) -> &str;
    fn get_body(&self) -> serde_json::Value;
}

pub trait RedfishTree {
    // Return Some(RedfishNode) matching the given URI, or None if it doesn't exist
    // TODO: async???
    fn get(&self, uri: &str) -> Option<&Box<dyn RedfishNode + Send + Sync>>;
}

pub fn app(
    tree: Arc<dyn RedfishTree + Send + Sync>,
 ) -> NormalizePath<Router> {
    let state = AppState {
        tree,
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

//TODO: Is it necessary to wrap the tree in this struct at all?
struct AppState {
    tree: Arc<dyn RedfishTree + Send + Sync>,
}

async fn getter(
    Path(path): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Response {
    let uri = "/redfish/".to_owned() + &path;
    //FIXME: Does using a mutex here defeat the purpose of async?
    let state = state.lock().unwrap();
    if let Some(node) = state.tree.get(uri.as_str()) {
        return Json(node.get_body()).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn get_redfish() -> Json<Value> {
    Json(json!({ "v1": "/redfish/v1/" }))
}