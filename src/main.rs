use axum::{
    ServiceExt,
    Router,
};
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

fn get_uri_id(uri: &str) -> String {
    match uri {
        "/redfish/v1" => String::from("RootService"),
        _ => String::from(std::path::Path::new(uri).file_name().unwrap().to_str().unwrap())
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
    fn new(uri: &str, resource_type: String, schema_version: String, term_name: String, name: String, rest: Value) -> Self {
        let mut body = rest;
        body["@odata.id"] = json!(uri);
        body["@odata.type"] = json!(format!("#{}.{}.{}", resource_type, schema_version, term_name));
        let id = get_uri_id(uri);
        body["Id"] = json!(id);
        body["Name"] = json!(name);
        Self {
            uri: String::from(uri), resource_type, schema_version, term_name, id, body
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
    //FIXME: Would be better as a Map
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

    fn create(&mut self, uri: &str, req: serde_json::Value) -> Option<&Box<dyn RedfishNode + Send + Sync>> {
        for node in &self.nodes {
            if uri == node.get_uri() {
                // TODO: Don't hardcode this!
                if uri != "/redfish/v1/SessionService/Sessions" {
                    return None;
                }

                // Look at existing members to see next Id to pick
                // TODO: Does this need to protect against parallel runs?
                // TODO: Less catastrophic error handling
                // TODO: Use members prop directly?
                let collection = self.get(uri).unwrap();
                let body = collection.get_body();
                let members = body
                    .as_object().unwrap()
                    .get("Members").unwrap()
                    .as_array().unwrap();
                let mut highest = 0;
                for member in members {
                    let id = get_uri_id(member.as_object().unwrap().get("@odata.id").unwrap().as_str().unwrap());
                    let id = id.parse().unwrap();
                    if id > highest {
                        highest = id;
                    }
                }
                let id = (highest + 1).to_string();
                let member_uri = format!("{}/{}", collection.get_uri(), id);

                // Create new resource and add it to the tree.
                let new_member = Box::new(RedfishResource::new(
                    member_uri.as_str(),
                    String::from("Session"),
                    String::from("v1_6_0"),
                    String::from("Session"),
                    String::from(format!("Session {}", id)),
                    json!({
                        "UserName": req.as_object().unwrap().get("UserName").unwrap().as_str(),
                        "Password": serde_json::Value::Null,
                    }),
                ));
                self.add_node(new_member);

                // FIXME: Update members of collection.
                //collection.members.push(uri);

                // Return new resource.
                return self.get(member_uri.as_str());
            }
        }
        None
    }
}

fn get_mock_tree() -> MockTree {
    let mut tree = MockTree::new();
    tree.add_node(Box::new(RedfishResource::new(
        "/redfish/v1",
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
        "/redfish/v1/SessionService",
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
    redfish_axum::app(tree)
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
    use tower::{ServiceExt, Service};

    async fn get(app: &mut NormalizePath<Router>, uri: &str) -> Response {
        let req = Request::get(uri).body(Body::empty()).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    async fn jget(app: &mut NormalizePath<Router>, uri: &str, status_code: StatusCode) -> Value {
        let response = get(app, uri).await;

        assert_eq!(response.status(), status_code);
        assert_eq!(response.headers().get("content-type").unwrap().to_str().unwrap(), "application/json");

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn post(app: &mut NormalizePath<Router>, uri: &str, req: serde_json::Value) -> Response {
        let body = Body::from(serde_json::to_vec(&req).unwrap());
        let req = Request::post(uri).header("Content-Type", "application/json").body(body).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    async fn jpost(app: &mut NormalizePath<Router>, uri: &str, req: serde_json::Value, status_code: StatusCode) -> Value {
        let response = post(app, uri, req).await;
        assert_eq!(response.status(), status_code);
        assert_eq!(response.headers().get("content-type").unwrap().to_str().unwrap(), "application/json");

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn base_redfish_path() {
        let mut app = app();
        let body = jget(&mut app, "/redfish", StatusCode::OK).await;
        assert_eq!(body, json!({ "v1": "/redfish/v1/" }));
    }

    #[tokio::test]
    async fn redfish_v1() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1", StatusCode::OK).await;
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
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/SessionService/", StatusCode::OK).await;
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
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions", StatusCode::OK).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [],
            "Members@odata.count": 0,
        }));
    }

    /* FIXME
    #[tokio::test]
    async fn post_not_allowed() {
        let body = jpost("/redfish/v1", json!({}), StatusCode::METHOD_NOT_ALLOWED).await;
        assert_eq!(body, json!({
            "TODO": "FIXME",
        }));
    }
    */

    #[tokio::test]
    async fn post_session() {
        let mut app = app();
        let data = json!({"UserName": "Obiwan", "Password": "n/a"});
        let body = jpost(&mut app, "/redfish/v1/SessionService/Sessions", data, StatusCode::CREATED).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/1",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "1",
            "Name": "Session 1",
            "UserName": "Obiwan",
            "Password": serde_json::Value::Null,
        }));

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions/1", StatusCode::OK).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/1",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "1",
            "Name": "Session 1",
            "UserName": "Obiwan",
            "Password": serde_json::Value::Null,
        }));

        /* FIXME
        let body = jget("/redfish/v1/SessionService/Sessions", StatusCode::OK).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [
                {"@odata.id": "/redfish/v1/SessionService/Sessions/1"}
            ],
            "Members@odata.count": 1,
        }));
        */
    }

    #[tokio::test]
    async fn post_not_found() {
        let mut app = app();
        let response = post(&mut app, "/redfish/v1/notfound", json!({})).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn get_not_found() {
        let mut app = app();
        let response = get(&mut app, "/redfish/v1/notfound").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }
}