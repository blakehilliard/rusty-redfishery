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
use http::{header::{self}, HeaderMap, HeaderValue, HeaderName};
use redfish_data::{
    RedfishCollectionType,
    RedfishResourceType,
    get_odata_metadata_document, get_odata_service_document,
};

mod json;
use json::JsonResponse;

// TODO: Is this a better fit for redfish-data?
// TODO: This is nice for straight-forward cases, but how will I allow any custom error response?
#[derive(Debug)]
pub enum RedfishErr {
    NotFound,
    Unauthorized,
}

pub trait RedfishNode {
    fn get_uri(&self) -> &str;
    fn get_body(&self) -> serde_json::Value;
    fn can_post(&self) -> bool;
    fn can_delete(&self) -> bool;
    fn can_patch(&self) -> bool;
    fn described_by(&self) -> Option<&str>; // TODO: Stricter URL type???
}

// TODO: Should all these methods be async?
pub trait RedfishTree {
    // Return Some(RedfishNode) matching the given URI, or None if it doesn't exist
    fn get(&self, uri: &str) -> Result<&dyn RedfishNode, RedfishErr>;

    // Create a resource, given the collction URI and JSON input.
    // Return Ok(RedfishNode) of the new resource, or Err.
    // TODO: Properly handle various error cases.
    fn create(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, RedfishErr>;

    // Delete a resource, given its URI.
    // Return Ok after it has been deleted, or Error if it cannot be deleted.
    fn delete(&mut self, uri: &str) -> Result<(), RedfishErr>;

    // Patch a resource.
    // Return the patched resource on success, or Error.
    fn patch(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, RedfishErr>;

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
    /*
    let mut session_id: Option<String> = None;
    if let Some(token) = headers.get("x-auth-token") {
        session_id = get_token_session(token);
        if session_id.is_none() {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    match tree.get(uri.as_str(), session_id) {
    */
    match tree.get(uri.as_str()) {
        Ok(node) => get_node_get_response(node),
        Err(error) => get_error_response(error),
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
        // FIXME: Take advantage of RedfishErr
        Err(_) => {
            match tree.get(uri.as_str()) {
                Ok(node) => (
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
    match tree.create(uri.as_str(), payload) {
        Ok(node) => get_node_created_response(node),
        Err(error) => match error {
            RedfishErr::Unauthorized => get_error_response(error),
            // FIXME: Take advantage of RedfishErr
            _ => match tree.get(uri.as_str()) {
                Ok(node) => (
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
        Ok(node) => get_node_get_response(node),
        // FIXME: Take advantage of RedfishErr
        Err(_) => {
            match tree.get(uri.as_str()) {
                Ok(node) => (
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
    get_non_node_json_response(
        StatusCode::OK,
        json!({ "v1": "/redfish/v1/" }),
        "GET,HEAD",
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
    get_non_node_json_response(
        StatusCode::OK,
        get_odata_service_document(service_root.unwrap().get_body().as_object().unwrap()),
        "GET,HEAD",
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

fn add_described_by_header(headers: &mut HeaderMap, node: &dyn RedfishNode) -> () {
    if let Some(described_by) = node.described_by() {
        let val = format!("<{}>; rel=describedby", described_by);
        let val = HeaderValue::from_str(val.as_str()).expect("FIXME");
        headers.insert(header::LINK, val);
    }
}

fn get_node_get_response(node: &dyn RedfishNode) -> Response {
    let mut headers = get_standard_headers(node_to_allow(node).as_str());
    add_described_by_header(&mut headers, node);
    JsonResponse::new(
        StatusCode::OK,
        headers,
        node.get_body(),
    ).into_response()
}

fn get_node_created_response(node: &dyn RedfishNode) -> Response {
    let mut headers = get_standard_headers(node_to_allow(node).as_str());
    add_described_by_header(&mut headers, node);
    headers.insert(header::LOCATION, HeaderValue::from_str(node.get_uri()).expect("FIXME"));
    JsonResponse::new(
        StatusCode::CREATED,
        headers,
        node.get_body(),
    ).into_response()
}

fn get_non_node_json_response(status: StatusCode, data: serde_json::Value, allow: &str) -> Response {
    JsonResponse::new(
        status,
        get_standard_headers(allow),
        data,
    ).into_response()
}

fn get_standard_headers(allow: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ALLOW, HeaderValue::from_str(allow).expect("FIXME"));
    headers.insert(HeaderName::from_static("odata-version"), HeaderValue::from_static("4.0"));
    headers.insert(HeaderName::from_static("cache-control"), HeaderValue::from_static("no-cache"));
    headers
}

fn get_error_response(error: RedfishErr) -> Response {
    match error {
        RedfishErr::NotFound => (
            StatusCode::NOT_FOUND,
            // FIXME: Avoid repeating this everywhere
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")]
        ).into_response(),
        RedfishErr::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")]
        ).into_response(),
    }
}