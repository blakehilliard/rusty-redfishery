use axum::{
    extract::{
        Path,
        State,
    },
    http::StatusCode,
    routing::get,
    response::{Json, IntoResponse},
    ServiceExt,
    Router,
    debug_handler,
};
use std::sync::{Arc, Mutex};
use tower_http::normalize_path::{NormalizePath};
use serde_json::{Value, json};

trait RedfishNode {
    fn get_uri(&self) -> &str;
    fn get_body(&self) -> Value;
}

struct RedfishCollection {
    uri: String,
    resource_type: String,
    name: String,
    members: Vec<String>,
}

impl RedfishNode for RedfishCollection {
    fn get_uri(&self) -> &str {
        self.uri.as_str()
    }

    fn get_body(&self) -> Value {
        //FIXME: Support more than 0 members
        json!({
            "@odata.id": self.uri,
            "@odata.type": format!("#{}.{}", self.resource_type, self.resource_type),
            "Name": self.name,
            "Members": [

            ],
            "Members@odata.count": self.members.len(),
        })
    }
}

#[allow(dead_code)]
struct RedfishResource {
    uri: String, //TODO: Enforce things here? Does DMTF recommend trailing slash or no?
    resource_type: String,
    schema_version: String, //TODO: Enforce conformity
    term_name: String, //TODO: Constructor where this is optional and derived from resource_type
    id: String, //TODO: Better name?
    body: Value, //TODO: Enforce map
}

impl RedfishResource {
    fn new(uri: String, resource_type: String, schema_version: String, term_name: String, name: String, rest: Value) -> Self {
        let mut body = rest;
        body["@odata.id"] = json!(uri);
        body["@odata.type"] = json!(format!("#{}.{}.{}", resource_type, schema_version, term_name));
        let id = match resource_type.as_str() {
            "ServiceRoot" => String::from("RootService"),
            _ => String::from(std::path::Path::new(uri.as_str()).file_name().unwrap().to_str().unwrap())
        };
        body["Id"] = json!(id);
        body["Name"] = json!(name);
        Self {
            uri, resource_type, schema_version, term_name, id, body
        }
    }
}

impl RedfishNode for RedfishResource {
    fn get_uri(&self) -> &str {
        self.uri.as_str()
    }

    fn get_body(&self) -> Value {
        self.body.clone()
    }
}

#[debug_handler]
async fn handle_redfish_path(
    Path(path): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    let uri = "/redfish/".to_owned() + &path;
    let state = state.lock().unwrap();
    for node in &state.redfish_tree {
        if uri == node.get_uri() {
            return (StatusCode::OK, Json(node.get_body()));
        }
    }
    (StatusCode::NOT_FOUND, Json(json!({"TODO": "FIXME"}))) //FIXME
}

fn app() -> NormalizePath<Router> {
    // Create mock redfish tree
    let mut tree: Vec<Box<dyn RedfishNode + Send>> = Vec::new();
    tree.push(Box::new(RedfishResource::new(
        String::from("/redfish/v1"),
        String::from("ServiceRoot"),
        String::from("v1_15_0"),
        String::from("ServiceRoot"),
        String::from("Root Service"),
        json!({
            "Links": {
                "Sessions": {
                    "@odata.id": "/redfish/v1/SessionService/Sessions"
                },
            },
        })
    )));
    tree.push(Box::new(RedfishResource::new(
        String::from("/redfish/v1/SessionService"),
        String::from("SessionService"),
        String::from("v1_1_9"),
        String::from("SessionService"),
        String::from("Session Service"),
        json!({
            "Sessions": {
                "@odata.id": "/redfish/v1/SessionService/Sessions"
            },
        })
    )));
    tree.push(Box::new(RedfishCollection {
        uri: String::from("/redfish/v1/SessionService/Sessions"),
        resource_type: String::from("SessionCollection"),
        name: String::from("Session Collection"),
        members: vec![],
    }));

    // Set as state which can be passed to all handlers
    let state = AppState {
        redfish_tree: tree,
    };
    let state = Arc::new(Mutex::new(state));

    let app = Router::new()
        .route("/redfish", get(redfish_axum::get_redfish))
        .route("/redfish/*path", get(handle_redfish_path))
        .with_state(state);
    //FIXME: Figure out how to do below rather than above
    //let app = redfish_axum::router(get(handle_redfish_path))
    //    .with_state(state);
    redfish_axum::layer(app)
}

struct AppState {
    redfish_tree: Vec<Box<dyn RedfishNode + Send>>,
}

#[tokio::main]
async fn main() {
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app().into_make_service())
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    async fn jget(uri: &str, status_code: StatusCode) -> Value {
        let response = app()
            .oneshot(
                Request::get(uri)
                    .body(Body::from(
                        serde_json::to_vec(&json!({})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), status_code);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn base_redfish_path() {
        let body = jget("/redfish", StatusCode::OK).await;
        assert_eq!(body, json!({ "v1": "/redfish/v1/" }));
    }

    #[tokio::test]
    async fn redfish_v1() {
        let body = jget("/redfish/v1", StatusCode::OK).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1",
            "@odata.type": "#ServiceRoot.v1_15_0.ServiceRoot",
            "Id": "RootService",
            "Name": "Root Service",
            "Links": {
                "Sessions": {
                    "@odata.id": "/redfish/v1/SessionService/Sessions"
                }
            }
        }));
    }

    #[tokio::test]
    async fn session_service() {
        let body = jget("/redfish/v1/SessionService/", StatusCode::OK).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService",
            "@odata.type": "#SessionService.v1_1_9.SessionService",
            "Id": "SessionService",
            "Name": "Session Service",
            "Sessions" : {"@odata.id": "/redfish/v1/SessionService/Sessions"},
        }));
    }

    #[tokio::test]
    async fn empty_session_collection() {
        let body = jget("/redfish/v1/SessionService/Sessions", StatusCode::OK).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [],
            "Members@odata.count": 0,
        }));
    }

    #[tokio::test]
    async fn not_found() {
        let body = jget("/redfish/v1/notfound", StatusCode::NOT_FOUND).await;
        assert_eq!(body, json!({ "TODO": "FIXME" }));
    }
}