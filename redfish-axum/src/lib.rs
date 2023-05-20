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
use serde_json::{json};
use http::{header, HeaderMap};
use redfish_data::{
    RedfishCollectionType,
    RedfishResourceType,
    get_odata_metadata_document, get_odata_service_document,
};

mod json;
use json::JsonResponse;

pub trait RedfishNode {
    fn get_uri(&self) -> &str;
    fn get_body(&self) -> serde_json::Value;
    fn can_post(&self) -> bool;
    fn can_delete(&self) -> bool;
    fn can_patch(&self) -> bool;
    fn described_by(&self) -> Option<&str>; // TODO: Stricter type???
}

// TODO: Should all these methods be async?
pub trait RedfishTree {
    // Return Some(RedfishNode) matching the given URI, or None if it doesn't exist
    fn get(&self, uri: &str) -> Option<&dyn RedfishNode>;

    // Create a resource, given the collction URI and JSON input.
    // Return Ok(RedfishNode) of the new resource, or Err.
    // TODO: Properly handle various error cases.
    fn create(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, ()>;

    // Delete a resource, given its URI.
    // Return Ok after it has been deleted, or Error if it cannot be deleted.
    fn delete(&mut self, uri: &str) -> Result<(), ()>;

    // Patch a resource.
    // Return the patched resource on success, or Error.
    fn patch(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, ()>;

    fn get_collection_types(&self) -> &[RedfishCollectionType];

    fn get_resource_types(&self) -> &[RedfishResourceType];
}

pub fn app<T: RedfishTree + Send + Sync + 'static>(tree: T) -> NormalizePath<Router> {
    let state = AppState {
        tree: Arc::new(Mutex::new(tree)),
    };

    let app = Router::new()
        .route("/redfish",
               get(get_redfish))
        .route("/redfish/v1/$metadata",
               get(get_odata_metadata_doc))
        .route("/redfish/v1/odata", get(get_odata_service_doc))
        .route("/redfish/*path",
               get(getter).post(poster).delete(deleter).patch(patcher))
        .with_state(state);

    NormalizePathLayer::trim_trailing_slash()
        .layer(app)
}

//FIXME: Figure out right kind of mutex: https://docs.rs/tokio/1.25.0/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
//TODO: Is it necessary to wrap the tree in this struct at all?
#[derive(Clone)]
struct AppState {
    tree: Arc<Mutex<dyn RedfishTree + Send>>,
}

async fn getter(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Response {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return bad_odata_version_response();
        }
    }
    let uri = "/redfish/".to_owned() + &path;
    let tree = state.tree.lock().unwrap();
    match tree.get(uri.as_str()) {
        Some(node) => JsonResponse::new(
            node.get_body(),
            node_to_allow(node),
            node.described_by(),
        ).into_response(),
        _ => StatusCode::NOT_FOUND.into_response()
    }
}

async fn deleter(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Response {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return bad_odata_version_response();
        }
    }
    let uri = "/redfish/".to_owned() + &path;
    let mut tree = state.tree.lock().unwrap();
    match tree.delete(uri.as_str()) {
        Ok(_) => (
            StatusCode::NO_CONTENT,
            [("Cache-Control", "no-cache")],
        ).into_response(),
        Err(_) => {
            match tree.get(uri.as_str()) {
                Some(node) => (
                    StatusCode::METHOD_NOT_ALLOWED,
                    [(header::ALLOW, node_to_allow(node))],
                    [("OData-Version", "4.0")],
                    [("Cache-Control", "no-cache")],
                ).into_response(),
                _ => (
                    StatusCode::NOT_FOUND,
                    [("Cache-Control", "no-cache")],
                ).into_response(),
            }
        }
    }
}

async fn poster(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return bad_odata_version_response();
        }
    }

    let mut uri = "/redfish/".to_owned() + &path;
    if let Some(stripped) = uri.strip_suffix("/Members") {
        uri = stripped.to_string();
    }

    let mut tree = state.tree.lock().unwrap();
    // FIXME: Do I need to set described_by here???
    if let Ok(node) = tree.create(uri.as_str(), payload) {
        return (
            StatusCode::CREATED,
            [(header::LOCATION, node.get_uri())],
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")],
            Json(node.get_body()),
        ).into_response();
    }
    match tree.get(uri.as_str()) {
        Some(node) => (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, node_to_allow(node))],
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")],
        ).into_response(),
        _ => (
            StatusCode::NOT_FOUND,
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")],
        ).into_response(),
    }
}

async fn patcher(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return bad_odata_version_response();
        }
    }
    let uri = "/redfish/".to_owned() + &path;
    let mut tree = state.tree.lock().unwrap();
    match tree.patch(uri.as_str(), payload) {
        Ok(node) => JsonResponse::new(
            node.get_body(),
            node_to_allow(node),
            node.described_by(),
        ).into_response(),
        Err(_) => {
            match tree.get(uri.as_str()) {
                Some(node) => (
                    StatusCode::METHOD_NOT_ALLOWED,
                    [(header::ALLOW, node_to_allow(node))],
                    [("Cache-Control", "no-cache")],
                ).into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}

async fn get_redfish(headers: HeaderMap) -> Response {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return bad_odata_version_response();
        }
    }
    JsonResponse::new(
        json!({ "v1": "/redfish/v1/" }),
        String::from("GET,HEAD"),
        None,
    ).into_response()
}

async fn get_odata_metadata_doc(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return bad_odata_version_response();
        }
    }
    let tree = state.tree.lock().unwrap();
    let body = get_odata_metadata_document(tree.get_collection_types(), tree.get_resource_types());
    (
        [(header::CONTENT_TYPE, "application/xml")],
        [(header::ALLOW, "GET,HEAD")],
        [("OData-Version", "4.0")],
        [("Cache-Control", "no-cache")],
        body,
    ).into_response()
}

async fn get_odata_service_doc(
    State(state): State<AppState>,
) -> Response {
    let tree = state.tree.lock().unwrap();
    let service_root = tree.get("/redfish/v1");
    JsonResponse::new(
        get_odata_service_document(service_root.unwrap().get_body().as_object().unwrap()),
        String::from("GET,HEAD"),
        None,
    ).into_response()
}

fn node_to_allow(node: &dyn RedfishNode) -> String {
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
    allow
}

fn bad_odata_version_response() -> Response {
    (
        StatusCode::PRECONDITION_FAILED,
        [("OData-Version", "4.0")],
        [("Cache-Control", "no-cache")],
    ).into_response()
}