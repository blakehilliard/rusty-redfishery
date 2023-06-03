use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use http::{
    header::{self},
    HeaderMap, HeaderName, HeaderValue,
};
use http_auth_basic;
use redfish_data::{
    get_odata_metadata_document, get_odata_service_document, AllowedMethods, RedfishCollectionType,
    RedfishResourceType,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tower::layer::Layer;
use tower_http::normalize_path::{NormalizePath, NormalizePathLayer};
use uuid::Uuid;

mod json;
use json::JsonResponse;

// TODO: Is this a better fit for redfish-data?
// TODO: This is nice for straight-forward cases, but how will I allow any custom error response?
#[derive(Debug)]
pub enum RedfishErr {
    NotFound,
    Unauthorized,
    MethodNotAllowed(AllowedMethods),
}

pub trait RedfishNode {
    fn get_uri(&self) -> &str;
    fn get_body(&self) -> serde_json::Value;
    fn get_allowed_methods(&self) -> AllowedMethods;
    fn described_by(&self) -> Option<&str>; // TODO: Stricter URL type???
}

// TODO: Should all these methods be async?
pub trait RedfishTree {
    // Return Ok(RedfishNode) at the given URI, or a RedfishErr.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return RedfishErr::Unauthorized.
    fn get(&self, uri: &str, username: Option<&str>) -> Result<&dyn RedfishNode, RedfishErr>;

    // Create a resource, given the collction URI and JSON input.
    // Return Ok(RedfishNode) of the new resource, or Err.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return RedfishErr::Unauthorized.
    fn create(
        &mut self,
        uri: &str,
        req: serde_json::Value,
        username: Option<&str>,
    ) -> Result<&dyn RedfishNode, RedfishErr>;

    // Delete a resource, given its URI.
    // Return Ok after it has been deleted, or Error if it cannot be deleted.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return RedfishErr::Unauthorized.
    fn delete(&mut self, uri: &str, username: Option<&str>) -> Result<(), RedfishErr>;

    // Patch a resource.
    // Return the patched resource on success, or Error.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return RedfishErr::Unauthorized.
    fn patch(
        &mut self,
        uri: &str,
        req: serde_json::Value,
        username: Option<&str>,
    ) -> Result<&dyn RedfishNode, RedfishErr>;

    fn get_collection_types(&self) -> &[RedfishCollectionType];

    fn get_resource_types(&self) -> &[RedfishResourceType];
}

// TODO: Better way to declare tree type???
pub fn app<T: RedfishTree + Send + Sync + 'static>(tree: T) -> NormalizePath<Router> {
    let state = AppState {
        tree: Arc::new(Mutex::new(tree)),
        sessions: Arc::new(Mutex::new(Vec::new())),
    };

    let app = Router::new()
        .route("/redfish", get(get_redfish))
        .route("/redfish/v1/$metadata", get(get_odata_metadata_doc))
        .route("/redfish/v1/odata", get(get_odata_service_doc))
        .route(
            "/redfish/*path",
            get(getter).post(poster).delete(deleter).patch(patcher),
        )
        .with_state(state);

    NormalizePathLayer::trim_trailing_slash().layer(app)
}

struct Session {
    token: String,
    username: String,
    uri: String,
}

//FIXME: Figure out right kind of mutex: https://docs.rs/tokio/1.25.0/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
#[derive(Clone)]
struct AppState {
    tree: Arc<Mutex<dyn RedfishTree + Send>>,
    sessions: Arc<Mutex<Vec<Session>>>,
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
    let user = match get_request_username(&headers, &state) {
        Ok(user) => user,
        Err(e) => return get_error_response(e),
    };
    match tree.get(uri.as_str(), user.as_deref()) {
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
    let user = match get_request_username(&headers, &state) {
        Ok(user) => user,
        Err(e) => return get_error_response(e),
    };

    match tree.delete(uri.as_str(), user.as_deref()) {
        Ok(_) => {
            let mut sessions = state.sessions.lock().unwrap();
            for index in 0..sessions.len() {
                if sessions[index].uri == uri {
                    sessions.swap_remove(index);
                    break;
                }
            }
            (StatusCode::NO_CONTENT, [("Cache-Control", "no-cache")]).into_response()
        }
        Err(error) => get_error_response(error),
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
    let user = match get_request_username(&headers, &state) {
        Ok(user) => user,
        Err(e) => return get_error_response(e),
    };

    match tree.create(uri.as_str(), payload, user.as_deref()) {
        Ok(node) => {
            let mut additional_headers = HeaderMap::new();
            // TODO: Would it be better to inspect node to see if it's a Session?
            if uri == "/redfish/v1/SessionService/Sessions" {
                let token = Uuid::new_v4().as_simple().to_string();
                let username = node
                    .get_body()
                    .as_object()
                    .unwrap()
                    .get("UserName")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                let session = Session {
                    token: token.clone(),
                    username,
                    uri: node.get_uri().to_string(),
                };
                state.sessions.lock().unwrap().push(session);
                let header_val = HeaderValue::from_str(token.as_str()).unwrap();
                additional_headers.insert("x-auth-token", header_val);
            }
            get_node_created_response(node, additional_headers)
        }
        Err(error) => get_error_response(error),
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
    let user = match get_request_username(&headers, &state) {
        Ok(user) => user,
        Err(e) => return get_error_response(e),
    };

    match tree.patch(uri.as_str(), payload, user.as_deref()) {
        Ok(node) => get_node_get_response(node),
        Err(error) => get_error_response(error),
    }
}

async fn get_redfish(headers: HeaderMap) -> Response {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return bad_odata_version_response();
        }
    }
    get_non_node_json_response(StatusCode::OK, json!({ "v1": "/redfish/v1/" }), "GET,HEAD")
        .into_response()
}

async fn get_odata_metadata_doc(headers: HeaderMap, State(state): State<AppState>) -> Response {
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
    )
        .into_response()
}

async fn get_odata_service_doc(State(state): State<AppState>) -> Response {
    let tree = state.tree.lock().unwrap();
    let service_root = tree.get("/redfish/v1", None);
    get_non_node_json_response(
        StatusCode::OK,
        get_odata_service_document(service_root.unwrap().get_body().as_object().unwrap()),
        "GET,HEAD",
    )
    .into_response()
}

fn node_to_allow(node: &dyn RedfishNode) -> String {
    node.get_allowed_methods().to_string()
}

fn bad_odata_version_response() -> Response {
    (
        StatusCode::PRECONDITION_FAILED,
        [("OData-Version", "4.0")],
        [("Cache-Control", "no-cache")],
    )
        .into_response()
}

fn get_described_by_header_value(node: &dyn RedfishNode) -> Option<HeaderValue> {
    if let Some(described_by) = node.described_by() {
        let val = format!("<{}>; rel=describedby", described_by);
        if let Ok(val) = HeaderValue::from_str(val.as_str()) {
            return Some(val);
        }
    }
    None
}

fn get_node_etag_header_value(node: &dyn RedfishNode) -> Option<HeaderValue> {
    let body = node.get_body();
    if body.is_object() {
        if let Some(etag) = body.as_object().unwrap().get("@odata.etag") {
            if let Ok(val) = HeaderValue::from_str(etag.as_str()?) {
                return Some(val);
            }
        }
    }
    None
}

fn add_node_headers(headers: &mut HeaderMap, node: &dyn RedfishNode) -> () {
    if let Some(described_by) = get_described_by_header_value(node) {
        headers.insert(header::LINK, described_by);
    }
    if let Some(etag) = get_node_etag_header_value(node) {
        headers.insert(header::ETAG, etag);
    }
    if let Some(described_by) = node.described_by() {
        let val = format!("<{}>; rel=describedby", described_by);
        let val = HeaderValue::from_str(val.as_str()).unwrap();
        headers.insert(header::LINK, val);
    }
}

fn get_node_get_response(node: &dyn RedfishNode) -> Response {
    let mut headers = get_standard_headers(node_to_allow(node).as_str());
    add_node_headers(&mut headers, node);
    JsonResponse::new(StatusCode::OK, headers, node.get_body()).into_response()
}

fn get_node_created_response(node: &dyn RedfishNode, additional_headers: HeaderMap) -> Response {
    let mut headers = get_standard_headers(node_to_allow(node).as_str());
    headers.extend(additional_headers);
    add_node_headers(&mut headers, node);
    headers.insert(
        header::LOCATION,
        HeaderValue::from_str(node.get_uri()).unwrap(),
    );
    JsonResponse::new(StatusCode::CREATED, headers, node.get_body()).into_response()
}

fn get_non_node_json_response(
    status: StatusCode,
    data: serde_json::Value,
    allow: &str,
) -> Response {
    JsonResponse::new(status, get_standard_headers(allow), data).into_response()
}

fn get_standard_headers(allow: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ALLOW, HeaderValue::from_str(allow).unwrap());
    headers.insert(
        HeaderName::from_static("odata-version"),
        HeaderValue::from_static("4.0"),
    );
    headers.insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-cache"),
    );
    headers
}

fn get_error_response(error: RedfishErr) -> Response {
    match error {
        RedfishErr::NotFound => (
            StatusCode::NOT_FOUND,
            // FIXME: Avoid repeating this everywhere
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")],
        )
            .into_response(),
        RedfishErr::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")],
            [("www-authenticate", "Basic realm=\"simple\"")], // TODO: Customize?
        )
            .into_response(),
        RedfishErr::MethodNotAllowed(allowed) => (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, allowed.to_string())],
            [("OData-Version", "4.0")],
            [("Cache-Control", "no-cache")],
        )
            .into_response(),
    }
}

fn get_token_user(token: String, state: &AppState) -> Option<String> {
    for session in state.sessions.lock().unwrap().iter() {
        if session.token == token {
            return Some(session.username.clone());
        }
    }
    None
}

// Parse credentials from request. If bad credentials, return RedfishError.
// If no credentials, return Ok(None).
// If credentials check out, return Ok(Some(username)).
fn get_request_username(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<Option<String>, RedfishErr> {
    match headers.get("x-auth-token") {
        Some(token) => match get_token_user(token.to_str().unwrap().to_string(), &state) {
            None => Err(RedfishErr::Unauthorized),
            Some(user) => Ok(Some(user)),
        },
        None => match headers.get("authorization") {
            None => Ok(None),
            Some(header_val) => match http_auth_basic::Credentials::from_header(
                header_val.to_str().unwrap().to_string(),
            ) {
                Err(_) => Err(RedfishErr::Unauthorized),
                // TODO: Actually validate credentials!
                Ok(credentials) => Ok(Some(credentials.user_id)),
            },
        },
    }
}
