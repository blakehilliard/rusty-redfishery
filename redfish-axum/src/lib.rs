use async_trait::async_trait;
use axum::{
    debug_handler,
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
    get_odata_metadata_document, get_odata_service_document, AllowedMethods, CollectionType,
    ResourceType,
};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio;
use tower::layer::Layer;
use tower_http::normalize_path::{NormalizePath, NormalizePathLayer};
use uuid::Uuid;

mod json;
use json::JsonResponse;

// TODO: Is this a better fit for redfish-data?
// TODO: This is nice for straight-forward cases, but how will I allow any custom error response?
#[derive(Debug)]
pub enum Error {
    NotFound,
    Unauthorized,
    MethodNotAllowed(AllowedMethods),
    BadODataVersion,
}

pub trait Node {
    fn get_uri(&self) -> &str; // TODO: Stricter type? Ensure abspath? Don't allow trailing / ???
    fn get_body(&self) -> Value;
    fn get_allowed_methods(&self) -> AllowedMethods;
    fn described_by(&self) -> Option<&str>; // TODO: Stricter URL type???
}

#[async_trait]
pub trait Tree {
    // Return Ok(Node) at the given URI, or a Error.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return Error::Unauthorized.
    async fn get(&self, uri: &str, username: Option<&str>) -> Result<&dyn Node, Error>;

    // Create a resource, given the collction URI and JSON input.
    // Return Ok(Node) of the new resource, or Err.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return Error::Unauthorized.
    async fn create(
        &mut self,
        uri: &str,
        req: Map<String, Value>,
        username: Option<&str>,
    ) -> Result<&dyn Node, Error>;

    // Delete a resource, given its URI.
    // Return Ok after it has been deleted, or Error if it cannot be deleted.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return Error::Unauthorized.
    async fn delete(&mut self, uri: &str, username: Option<&str>) -> Result<(), Error>;

    // Patch a resource.
    // Return the patched resource on success, or Error.
    // If the request successfully provided credentials as a user, the username is given.
    // If the request did not attempt to authenticate, the username is None.
    // If the requested URI requires authentication, and the username is None, you must return Error::Unauthorized.
    async fn patch(
        &mut self,
        uri: &str,
        req: Map<String, Value>,
        username: Option<&str>,
    ) -> Result<&dyn Node, Error>;

    fn get_collection_types(&self) -> &[CollectionType];

    fn get_resource_types(&self) -> &[ResourceType];
}

// TODO: Better way to declare tree type???
pub fn app<T: Tree + Send + Sync + 'static>(tree: T) -> NormalizePath<Router> {
    let state = AppState {
        tree: Arc::new(tokio::sync::RwLock::new(tree)),
        sessions: Arc::new(std::sync::RwLock::new(Vec::new())),
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

#[derive(Clone)]
struct AppState {
    tree: Arc<tokio::sync::RwLock<dyn Tree + Send + Sync>>,
    sessions: Arc<std::sync::RwLock<Vec<Session>>>,
}

fn validate_odata_version(headers: &HeaderMap) -> Result<(), Error> {
    if let Some(odata_version) = headers.get("odata-version") {
        if odata_version != "4.0" {
            return Err(Error::BadODataVersion);
        }
    }
    Ok(())
}

#[debug_handler]
async fn getter(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Error> {
    validate_odata_version(&headers)?;
    let uri = "/redfish/".to_owned() + &path;
    let tree = state.tree.read().await;
    let user = get_request_username(&headers, &state)?;
    let node = tree.get(uri.as_str(), user.as_deref()).await?;
    Ok(get_node_get_response(node))
}

#[debug_handler]
async fn deleter(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Error> {
    validate_odata_version(&headers)?;
    let uri = "/redfish/".to_owned() + &path;
    let mut tree = state.tree.write().await;
    let user = get_request_username(&headers, &state)?;

    tree.delete(uri.as_str(), user.as_deref()).await?;
    let mut sessions = state.sessions.write().unwrap();
    for index in 0..sessions.len() {
        if sessions[index].uri == uri {
            sessions.swap_remove(index);
            break;
        }
    }
    Ok((StatusCode::NO_CONTENT, [("Cache-Control", "no-cache")]))
}

#[debug_handler]
async fn poster(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<Map<String, Value>>,
) -> Result<impl IntoResponse, Error> {
    validate_odata_version(&headers)?;

    let mut uri = "/redfish/".to_owned() + &path;
    if let Some(stripped) = uri.strip_suffix("/Members") {
        uri = stripped.to_string();
    }

    let mut tree = state.tree.write().await;
    let user = get_request_username(&headers, &state)?;

    let node = tree.create(uri.as_str(), payload, user.as_deref()).await?;
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
        state.sessions.write().unwrap().push(session);
        let header_val = HeaderValue::from_str(token.as_str()).unwrap();
        additional_headers.insert("x-auth-token", header_val);
    }
    Ok(get_node_created_response(node, additional_headers))
}

#[debug_handler]
async fn patcher(
    headers: HeaderMap,
    Path(path): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<Map<String, Value>>,
) -> Result<impl IntoResponse, Error> {
    validate_odata_version(&headers)?;
    let uri = "/redfish/".to_owned() + &path;
    let mut tree = state.tree.write().await;
    let user = get_request_username(&headers, &state)?;

    let node = tree.patch(uri.as_str(), payload, user.as_deref()).await?;
    Ok(get_node_get_response(node))
}

async fn get_redfish(headers: HeaderMap) -> Result<impl IntoResponse, Error> {
    validate_odata_version(&headers)?;
    Ok(get_non_node_json_response(
        StatusCode::OK,
        json!({ "v1": "/redfish/v1/" }),
        "GET,HEAD",
    ))
}

async fn get_odata_metadata_doc(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Error> {
    validate_odata_version(&headers)?;
    let tree = state.tree.read().await;
    let body = get_odata_metadata_document(tree.get_collection_types(), tree.get_resource_types());
    Ok((
        [(header::CONTENT_TYPE, "application/xml")],
        [(header::ALLOW, "GET,HEAD")],
        COMMON_RESPONSE_HEADERS,
        body,
    ))
}

async fn get_odata_service_doc(State(state): State<AppState>) -> impl IntoResponse {
    let tree = state.tree.read().await;
    let service_root = tree.get("/redfish/v1", None).await;
    get_non_node_json_response(
        StatusCode::OK,
        //TODO: Handle better than unwrap()
        get_odata_service_document(service_root.unwrap().get_body().as_object().unwrap()),
        "GET,HEAD",
    )
}

fn node_to_allow(node: &dyn Node) -> String {
    node.get_allowed_methods().to_string()
}

fn get_described_by_header_value(node: &dyn Node) -> Option<HeaderValue> {
    if let Some(described_by) = node.described_by() {
        let val = format!("<{}>; rel=describedby", described_by);
        if let Ok(val) = HeaderValue::from_str(val.as_str()) {
            return Some(val);
        }
    }
    None
}

fn get_node_etag_header_value(node: &dyn Node) -> Option<HeaderValue> {
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

fn add_node_headers(headers: &mut HeaderMap, node: &dyn Node) -> () {
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

fn get_node_get_response(node: &dyn Node) -> impl IntoResponse {
    let mut headers = get_standard_headers(node_to_allow(node).as_str());
    add_node_headers(&mut headers, node);
    JsonResponse::new(StatusCode::OK, headers, node.get_body())
}

fn get_node_created_response(node: &dyn Node, additional_headers: HeaderMap) -> impl IntoResponse {
    let mut headers = get_standard_headers(node_to_allow(node).as_str());
    headers.extend(additional_headers);
    add_node_headers(&mut headers, node);
    headers.insert(
        header::LOCATION,
        HeaderValue::from_str(node.get_uri()).unwrap(),
    );
    JsonResponse::new(StatusCode::CREATED, headers, node.get_body())
}

fn get_non_node_json_response(status: StatusCode, data: Value, allow: &str) -> impl IntoResponse {
    JsonResponse::new(status, get_standard_headers(allow), data)
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

const COMMON_RESPONSE_HEADERS: ([(&str, &str); 1], [(&str, &str); 1]) =
    ([("OData-Version", "4.0")], [("Cache-Control", "no-cache")]);

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound => (StatusCode::NOT_FOUND, COMMON_RESPONSE_HEADERS).into_response(),
            Error::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                COMMON_RESPONSE_HEADERS,
                [("www-authenticate", "Basic realm=\"simple\"")],
            )
                .into_response(),
            Error::MethodNotAllowed(allowed) => (
                StatusCode::METHOD_NOT_ALLOWED,
                [(header::ALLOW, allowed.to_string())],
                COMMON_RESPONSE_HEADERS,
            )
                .into_response(),
            Error::BadODataVersion => {
                (StatusCode::PRECONDITION_FAILED, COMMON_RESPONSE_HEADERS).into_response()
            }
        }
    }
}

fn get_token_user(token: String, state: &AppState) -> Option<String> {
    for session in state.sessions.read().unwrap().iter() {
        if session.token == token {
            return Some(session.username.clone());
        }
    }
    None
}

// Parse credentials from request. If bad credentials, return Erroror.
// If no credentials, return Ok(None).
// If credentials check out, return Ok(Some(username)).
fn get_request_username(headers: &HeaderMap, state: &AppState) -> Result<Option<String>, Error> {
    match headers.get("x-auth-token") {
        Some(token) => match get_token_user(token.to_str().unwrap().to_string(), &state) {
            None => Err(Error::Unauthorized),
            Some(user) => Ok(Some(user)),
        },
        None => match headers.get("authorization") {
            None => Ok(None),
            Some(header_val) => match http_auth_basic::Credentials::from_header(
                header_val.to_str().unwrap().to_string(),
            ) {
                Err(_) => Err(Error::Unauthorized),
                // TODO: Actually validate credentials!
                Ok(credentials) => Ok(Some(credentials.user_id)),
            },
        },
    }
}
