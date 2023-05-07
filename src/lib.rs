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

mod json;
use json::JsonGetResponse;

pub trait RedfishNode {
    fn get_uri(&self) -> &str;
    fn get_body(&self) -> serde_json::Value;
    fn can_post(&self) -> bool;
    fn can_delete(&self) -> bool;
    fn can_patch(&self) -> bool;
}

// TODO: Should all these methods be async?
pub trait RedfishTree {
    // Return Some(RedfishNode) matching the given URI, or None if it doesn't exist
    fn get(&self, uri: &str) -> Option<&dyn RedfishNode>;

    // Create a resource, given the collction URI and JSON input.
    // Return Some(RedfishNode) of the new resource, or None on fail.
    // TODO: Properly handle various error cases.
    fn create(&mut self, uri: &str, req: serde_json::Value) -> Option<&dyn RedfishNode>;
}

pub fn app<T: RedfishTree + Send + Sync + 'static>(tree: T) -> NormalizePath<Router> {
    let state = AppState {
        tree: Arc::new(Mutex::new(tree)),
    };

    let app = Router::new()
        .route("/redfish",
               get(get_redfish))
        .route("/redfish/*path",
               get(getter).post(poster))
        .with_state(state);

    NormalizePathLayer::trim_trailing_slash()
        .layer(app)
}

//FIXME: Figure out right kind of mutex: https://docs.rs/tokio/1.25.0/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
//TODO: Is it necessary to wrap the tree in this struct at all?
#[derive(Clone)]
struct AppState {
    tree: Arc<Mutex<dyn RedfishTree + Send + Sync>>,
}

async fn getter(
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let uri = "/redfish/".to_owned() + &path;
    let tree = state.tree.lock().unwrap();
    if let Some(node) = tree.get(uri.as_str()) {
        let mut allow = String::from("GET,HEAD");
        if node.can_delete() {
            allow.push_str(",DELETE");
        }
        if node.can_patch() {
            allow.push_str(",PATCH");
        }
        if node.can_post() {
            allow.push_str(",POST");
        }
        return JsonGetResponse {
            data: node.get_body(),
            allow: allow,
        }.into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn poster(
    Path(path): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let uri = "/redfish/".to_owned() + &path;
    if let Some(node) = state.tree.lock().unwrap().create(uri.as_str(), payload) {
        return (StatusCode::CREATED, Json(node.get_body())).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn get_redfish() -> JsonGetResponse<Value> {
    JsonGetResponse {
        data: json!({ "v1": "/redfish/v1/" }),
        allow: String::from("GET,HEAD"),
    }
}