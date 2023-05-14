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
use http::header;
use redfish_data::{
    RedfishCollectionType,
    RedfishResourceType,
};

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

    // Delete a resource, given its URI.
    // Return Ok after it has been deleted, or Error if it cannot be deleted.
    fn delete(&mut self, uri: &str) -> Result<(), ()>;

    // Patch a resource.
    // Return the patched resource on success, or Error.
    fn patch(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, ()>;

    fn get_collection_types(&self) -> &Vec<RedfishCollectionType>;

    fn get_resource_types(&self) -> &Vec<RedfishResourceType>;
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
        .route("/redfish/*path",
               get(getter).post(poster).delete(deleter).patch(patcher))
        .with_state(state);

    NormalizePathLayer::trim_trailing_slash()
        .layer(app)
}

//FIXME: Figure out right kind of mutex: https://docs.rs/tokio/1.25.0/tokio/sync/struct.Mutex.html#which-kind-of-mutex-should-you-use
//TODO: Is it necessary to wrap the tree in this struct at all?
// TODO: Do I still need Send and Mutex?
#[derive(Clone)]
struct AppState {
    tree: Arc<Mutex<dyn RedfishTree + Send>>,
}

async fn getter(
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let uri = "/redfish/".to_owned() + &path;
    let tree = state.tree.lock().unwrap();
    match tree.get(uri.as_str()) {
        Some(node) => JsonGetResponse {
            data: node.get_body(),
            allow: node_to_allow(node),
        }.into_response(),
        _ => StatusCode::NOT_FOUND.into_response()
    }
}

async fn deleter(
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let uri = "/redfish/".to_owned() + &path;
    let mut tree = state.tree.lock().unwrap();
    match tree.delete(uri.as_str()) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => {
            match tree.get(uri.as_str()) {
                Some(node) => (
                    StatusCode::METHOD_NOT_ALLOWED,
                    [(
                        header::ALLOW,
                        node_to_allow(node),
                    )],
                ).into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}

async fn poster(
    Path(path): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let uri = "/redfish/".to_owned() + &path;
    let mut tree = state.tree.lock().unwrap();
    if let Some(node) = tree.create(uri.as_str(), payload) {
        return (
            StatusCode::CREATED,
            [(header::LOCATION, node.get_uri())],
            [("OData-Version", "4.0")],
            Json(node.get_body()),
        ).into_response();
    }
    match tree.get(uri.as_str()) {
        Some(node) => (
            StatusCode::METHOD_NOT_ALLOWED,
            [(
                header::ALLOW,
                node_to_allow(node),
            )],
        ).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn patcher(
    Path(path): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let uri = "/redfish/".to_owned() + &path;
    let mut tree = state.tree.lock().unwrap();
    match tree.patch(uri.as_str(), payload) {
        Ok(node) => JsonGetResponse {
            data: node.get_body(),
            allow: node_to_allow(node),
        }.into_response(),
        Err(_) => {
            match tree.get(uri.as_str()) {
                Some(node) => (
                    StatusCode::METHOD_NOT_ALLOWED,
                    [(
                        header::ALLOW,
                        node_to_allow(node),
                    )],
                ).into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}

async fn get_redfish() -> JsonGetResponse<Value> {
    JsonGetResponse {
        data: json!({ "v1": "/redfish/v1/" }),
        allow: String::from("GET,HEAD"),
    }
}

async fn get_odata_metadata_doc(
    State(state): State<AppState>,
) -> Response {
    let tree = state.tree.lock().unwrap();

    let mut body = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<edmx:Edmx xmlns:edmx=\"http://docs.oasis-open.org/odata/ns/edmx\" Version=\"4.0\">\n");
    let mut service_root_type: Option<&RedfishResourceType> = None;
    for collection_type in tree.get_collection_types() {
        body.push_str(collection_type.to_xml().as_str());
    }
    for resource_type in tree.get_resource_types() {
        body.push_str(resource_type.to_xml().as_str());
        if resource_type.name == "ServiceRoot" {
            service_root_type = Some(resource_type);
        }
    }
    if service_root_type.is_some() {
        body.push_str("  <edmx:DataServices>\n");
        body.push_str("    <Schema xmlns=\"http://docs.oasis-open.org/odata/ns/edm\" Namespace=\"Service\">\n");
        body.push_str(format!("      <EntityContainer Name=\"Service\" Extends=\"{}.ServiceContainer\" />\n", service_root_type.unwrap().get_versioned_name()).as_str());
        body.push_str("    </Schema>\n  </edmx:DataServices>\n");
    }
    body.push_str("</edmx:Edmx>\n");

    (
        [(header::CONTENT_TYPE, "application/xml")],
        [(header::ALLOW, "GET,HEAD")],
        [("OData-Version", "4.0")],
        body,
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