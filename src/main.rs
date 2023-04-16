use axum::{
    ServiceExt,
    Router,
};
use std::sync::{Arc};
use tower_http::normalize_path::{NormalizePath};
use serde_json::{Value, json};
use redfish_axum::{RedfishNode, RedfishTree};

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

struct MockTree {
    nodes: Vec<Box<dyn RedfishNode + Send + Sync>>,
}

impl MockTree {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn add_node(&mut self, node: Box<dyn RedfishNode + Send + Sync>) {
        self.nodes.push(node);
    }
}

impl RedfishTree for MockTree {
    fn get(&self, uri: &str) -> Option<&Box<dyn RedfishNode + Send + Sync>> {
        for node in &self.nodes {
            if uri == node.get_uri() {
                return Some(node);
            }
        }
        None
    }
}

fn get_mock_tree() -> MockTree {
    let mut tree = MockTree::new();
    tree.add_node(Box::new(RedfishResource::new(
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
    tree.add_node(Box::new(RedfishResource::new(
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
    tree.add_node(Box::new(RedfishCollection {
        uri: String::from("/redfish/v1/SessionService/Sessions"),
        resource_type: String::from("SessionCollection"),
        name: String::from("Session Collection"),
        members: vec![],
    }));
    tree
}

fn app() -> NormalizePath<Router> {
    let tree = get_mock_tree();
    redfish_axum::app(Arc::new(tree))
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
        response::Response,
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    async fn get(uri: &str) -> Response {
        let req = Request::get(uri).body(Body::empty()).unwrap();
        app().oneshot(req).await.unwrap()
    }

    async fn jget(uri: &str, status_code: StatusCode) -> Value {
        let response = get(uri).await;

        assert_eq!(response.status(), status_code);
        assert_eq!(response.headers().get("content-type").unwrap().to_str().unwrap(), "application/json");

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
        let response = get("/redfish/v1/notfound").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }
}