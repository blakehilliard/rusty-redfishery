use std::collections::HashMap;

use axum::{
    ServiceExt,
    Router,
};
use tower_http::normalize_path::{NormalizePath};
use serde_json::{Value, json};
use redfish_axum::{
    RedfishNode,
    RedfishTree,
    RedfishCollectionType,
    RedfishResourceType,
    RedfishCollectionSchemaVersion,
    RedfishResourceSchemaVersion,
};

const ODATA_SERVICE_DOC_URI: &str = "/redfish/v1/odata";

struct RedfishCollection {
    uri: String,
    resource_type: String, //FIXME: name conflicts with RedfishResourceType
    schema_version: RedfishCollectionSchemaVersion,
    name: String,
    members: Vec<String>,
    postable: bool,
}

impl RedfishNode for RedfishCollection {
    fn get_uri(&self) -> &str {
        self.uri.as_str()
    }

    fn get_body(&self) -> Value {
        let mut member_list = Vec::new();
        for member in self.members.iter() {
            let mut member_obj = HashMap::new();
            member_obj.insert(String::from("@odata.id"), member);
            member_list.push(member_obj);
        }
        json!({
            "@odata.id": self.uri,
            "@odata.type": format!("#{}.{}", self.resource_type, self.resource_type),
            "Name": self.name,
            "Members": member_list,
            "Members@odata.count": self.members.len(),
        })
    }

    fn can_delete(&self) -> bool { false }

    fn can_patch(&self) -> bool { false }

    fn can_post(&self) -> bool { self.postable }
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
    resource_type: String, // FIXME: Name conflicts with RedfishResourceType ?
    schema_version: RedfishResourceSchemaVersion,
    term_name: String, //TODO: Constructor where this is optional and derived from resource_type
    id: String, //TODO: Better name?
    body: Value, //TODO: Enforce map
    deletable: bool,
    patchable: bool,
    collection: Option<String>,
}

impl RedfishResource {
    fn new(uri: &str, resource_type: String, schema_version: RedfishResourceSchemaVersion, term_name: String, name: String, deletable: bool, patchable: bool, collection: Option<String>, rest: Value) -> Self {
        let mut body = rest;
        body["@odata.id"] = json!(uri);
        body["@odata.type"] = json!(format!("#{}.{}.{}", resource_type, schema_version.to_str(), term_name));
        let id = get_uri_id(uri);
        body["Id"] = json!(id);
        body["Name"] = json!(name);
        Self {
            uri: String::from(uri), resource_type, schema_version, term_name, id, body, deletable, patchable, collection,
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

    fn can_delete(&self) -> bool { self.deletable }

    fn can_patch(&self) -> bool { self.patchable }

    fn can_post(&self) -> bool { false }
}

struct ODataServiceValue {
    kind: String,
    name: String,
    url: String,
}

struct ODataServiceDoc {
    values: Vec<ODataServiceValue>,
}

impl ODataServiceValue {
    fn new_singleton(url: &str) -> Self {
        Self {
            kind: String::from("Singleton"),
            name: String::from(std::path::Path::new(url).file_name().unwrap().to_str().unwrap()),
            url: String::from(url),
        }
    }
}

impl ODataServiceDoc {
    fn new() -> Self {
        let mut values = Vec::new();
        values.push(ODataServiceValue::new_singleton("/redfish/v1"));
        Self { values, }
    }

    fn add_value(&mut self, value: ODataServiceValue) {
        self.values.push(value);
    }
}

impl RedfishNode for ODataServiceDoc {
    fn get_uri(&self) -> &str {
        ODATA_SERVICE_DOC_URI
    }

    fn get_body(&self) -> Value {
        let mut value_list = Vec::new();
        for value in self.values.iter() {
            let mut value_obj = HashMap::new();
            value_obj.insert(String::from("kind"), value.kind.clone());
            value_obj.insert(String::from("name"), value.name.clone());
            value_obj.insert(String::from("url"), value.url.clone());
            value_list.push(value_obj);
        }
        json!({
            "@odata.id": self.get_uri(),
            "@odata.context": "/redfish/v1/$metadata",
            "value": value_list,
        })
    }

    fn can_delete(&self) -> bool { false }

    fn can_patch(&self) -> bool { false }

    fn can_post(&self) -> bool { false }
}

struct MockTree {
    //FIXME: Would be better as a Map
    resources: Vec<RedfishResource>,
    collections: Vec<RedfishCollection>,
    odata_service_doc: ODataServiceDoc,
    collection_types: Vec<RedfishCollectionType>,
    resource_types: Vec<RedfishResourceType>,
}

impl MockTree {
    fn new() -> Self {
        Self {
            resources: Vec::new(),
            collections: Vec::new(),
            odata_service_doc: ODataServiceDoc::new(),
            collection_types: Vec::new(),
            resource_types: Vec::new(),
         }
    }

    fn add_resource(&mut self, resource: RedfishResource) {
        let resource_type_name = resource.resource_type.clone();
        let schema_version = resource.schema_version.clone();
        self.resources.push(resource);
        for resource_type in self.resource_types.iter() {
            if resource_type.name == resource_type_name && resource_type.version == schema_version {
                return;
            }
        }
        self.resource_types.push(RedfishResourceType::new_dmtf(resource_type_name, schema_version));
    }

    fn add_collection(&mut self, collection: RedfishCollection) {
        let collection_type_name = collection.resource_type.clone();
        let schema_version = collection.schema_version.clone();
        self.collections.push(collection);
        for collection_type in self.collection_types.iter() {
            if collection_type.name == collection_type_name && collection_type.version == schema_version {
                return;
            }
        }
        self.collection_types.push(RedfishCollectionType::new_dmtf(collection_type_name, schema_version));
    }

    fn add_odata_service_value(&mut self, value: ODataServiceValue) {
        self.odata_service_doc.add_value(value);
    }
}

impl RedfishTree for MockTree {
    fn get(&self, uri: &str) -> Option<&dyn RedfishNode> {
        if uri == ODATA_SERVICE_DOC_URI {
            return Some(&self.odata_service_doc);
        }
        for node in &self.resources {
            if uri == node.get_uri() {
                return Some(node);
            }
        }
        for node in &self.collections {
            if uri == node.get_uri() {
                return Some(node);
            }
        }
        None
    }

    fn create(&mut self, uri: &str, req: serde_json::Value) -> Option<&dyn RedfishNode> {
        for collection in self.collections.iter_mut() {
            if uri == collection.get_uri() {
                // TODO: Don't hardcode this!
                if uri != "/redfish/v1/SessionService/Sessions" {
                    return None;
                }

                // Look at existing members to see next Id to pick
                // TODO: Less catastrophic error handling
                let mut highest = 0;
                for member in collection.members.iter() {
                    let id = get_uri_id(member.as_str());
                    let id = id.parse().unwrap();
                    if id > highest {
                        highest = id;
                    }
                }
                let id = (highest + 1).to_string();
                let member_uri = format!("{}/{}", collection.get_uri(), id);

                // Create new resource and add it to the tree.
                // TODO: Move lots of this stuff into SessionCollection struct???
                let new_member = RedfishResource::new(
                    member_uri.as_str(),
                    String::from("Session"),
                    RedfishResourceSchemaVersion::new(1, 6, 0),
                    String::from("Session"),
                    String::from(format!("Session {}", id)),
                    true,
                    false,
                    Some(String::from(uri)),
                    json!({
                        "UserName": req.as_object().unwrap().get("UserName").unwrap().as_str(),
                        "Password": serde_json::Value::Null,
                    }),
                );
                self.resources.push(new_member);

                // Update members of collection.
                collection.members.push(member_uri.clone());

                // Return new resource.
                return self.get(member_uri.as_str());
            }
        }
        None
    }

    fn delete(&mut self, uri: &str) -> Result<(), ()> {
        for index in 0..self.resources.len() {
            if self.resources[index].get_uri() == uri {
                if self.resources[index].can_delete() {
                    if let Some(collection_uri) = &self.resources[index].collection {
                        for collection in self.collections.iter_mut() {
                            if collection_uri == collection.get_uri() {
                                if let Some(member_index) = collection.members.iter().position(|x| x == uri) {
                                    collection.members.remove(member_index);
                                }
                                break;
                            }
                        }
                    }
                    self.resources.remove(index);
                    return Ok(());
                }
                return Err(());
            }
        }
        Err(())
    }

    fn patch(&mut self, uri: &str, req: serde_json::Value) -> Result<&dyn RedfishNode, ()> {
        for resource in self.resources.iter_mut() {
            if resource.get_uri() == uri {
                if ! resource.can_patch() {
                    return Err(());
                }
                if uri != "/redfish/v1/SessionService" {
                    return Err(());
                }
                // TODO: Move to per-resource functions
                // FIXME: Allow patch that doesn't set this! And do correct error handling!
                let new_timeout = req.as_object().unwrap().get("SessionTimeout").unwrap().as_u64().unwrap();
                let cur_timeout = resource.body.as_object_mut().unwrap().get_mut("SessionTimeout").unwrap();
                *cur_timeout = serde_json::Value::from(new_timeout);
                return Ok(resource);
            }
        }
        Err(())
    }

    fn get_collection_types(&self) -> &Vec<RedfishCollectionType> {
        &self.collection_types
    }

    fn get_resource_types(&self) -> &Vec<RedfishResourceType> {
        &self.resource_types
    }
}

fn get_mock_tree() -> MockTree {
    let mut tree = MockTree::new();
    //TODO: Make a way for this to not be hardcoded
    tree.add_odata_service_value(ODataServiceValue::new_singleton("/redfish/v1/AccountService"));
    tree.add_odata_service_value(ODataServiceValue::new_singleton("/redfish/v1/SessionService"));

    tree.add_resource(RedfishResource::new(
        "/redfish/v1",
        String::from("ServiceRoot"),
        RedfishResourceSchemaVersion::new(1, 15, 0),
        String::from("ServiceRoot"),
        String::from("Root Service"),
        false,
        false,
        None,
        json!({
            "AccountService": {
                "@odata.id": "/redfish/v1/AccountService",
            },
            "Links": {
                "Sessions": {
                    "@odata.id": "/redfish/v1/SessionService/Sessions"
                },
            },
        })
    ));
    tree.add_resource(RedfishResource::new(
        "/redfish/v1/SessionService",
        String::from("SessionService"),
        RedfishResourceSchemaVersion::new(1, 1, 9),
        String::from("SessionService"),
        String::from("Session Service"),
        false,
        true,
        None,
        json!({
            "@Redfish.WriteableProperties": ["SessionTimeout"],
            "SessionTimeout": 600,
            "Sessions": {
                "@odata.id": "/redfish/v1/SessionService/Sessions"
            },
        })
    ));
    tree.add_collection(RedfishCollection {
        uri: String::from("/redfish/v1/SessionService/Sessions"),
        resource_type: String::from("SessionCollection"),
        schema_version: RedfishCollectionSchemaVersion::new(1),
        name: String::from("Session Collection"),
        members: vec![],
        postable: true,
    });
    tree.add_resource(RedfishResource::new(
        "/redfish/v1/AccountService",
        String::from("AccountService"),
        RedfishResourceSchemaVersion::new(1, 12, 0),
        String::from("AccountService"),
        String::from("Account Service"),
        false,
        false,
        None,
        json!({
            "Accounts": {
                "@odata.id": "/redfish/v1/AccountService/Accounts"
            },
            "Roles": {
                "@odata.id": "/redfish/v1/AccountService/Roles"
            }
        })
    ));
    tree.add_collection(RedfishCollection {
        uri: String::from("/redfish/v1/AccountService/Accounts"),
        resource_type: String::from("ManagerAccountCollection"),
        schema_version: RedfishCollectionSchemaVersion::new(1),
        name: String::from("Account Collection"),
        members: vec![String::from("/redfish/v1/AccountService/Accounts/admin")],
        postable: true,
    });
    tree.add_resource(RedfishResource::new(
        "/redfish/v1/AccountService/Accounts/admin",
        String::from("ManagerAccount"),
        RedfishResourceSchemaVersion::new(1, 10, 0),
        String::from("ManagerAccount"),
        String::from("Admin Account"),
        false,
        false,
        Some(String::from("/redfish/v1/AccountService/Accounts")),
        json!({
            "@Redfish.WriteableProperties": ["Password"],
            "Links": {
                "Role": {
                    "@odata.id": "/redfish/v1/AccountService/Roles/Administrator"
                }
            },
            "Password": null,
            "RoleId": "Administrator",
            "UserName": "admin",
        })
    ));
    tree.add_collection(RedfishCollection {
        uri: String::from("/redfish/v1/AccountService/Roles"),
        resource_type: String::from("RoleCollection"),
        schema_version: RedfishCollectionSchemaVersion::new(1),
        name: String::from("Role Collection"),
        members: vec![
            String::from("/redfish/v1/AccountService/Roles/Administrator"),
            String::from("/redfish/v1/AccountService/Roles/Operator"),
            String::from("/redfish/v1/AccountService/Roles/ReadOnly"),
        ],
        postable: true,
    });
    tree.add_resource(RedfishResource::new(
        "/redfish/v1/AccountService/Roles/Administrator",
        String::from("Role"),
        RedfishResourceSchemaVersion::new(1, 3, 1),
        String::from("Role"),
        String::from("Administrator Role"),
        false,
        false,
        Some(String::from("/redfish/v1/AccountService/Roles")),
        json!({
            "AssignedPrivileges": [
                "Login",
                "ConfigureManager",
                "ConfigureUsers",
                "ConfigureSelf",
                "ConfigureComponents",
            ],
            "IsPredefined": true,
            "RoleId": "Administrator",
        })
    ));
    tree.add_resource(RedfishResource::new(
        "/redfish/v1/AccountService/Roles/Operator",
        String::from("Role"),
        RedfishResourceSchemaVersion::new(1, 3, 1),
        String::from("Role"),
        String::from("Operator Role"),
        false,
        false,
        Some(String::from("/redfish/v1/AccountService/Roles")),
        json!({
            "AssignedPrivileges": [
                "Login",
                "ConfigureSelf",
                "ConfigureComponents",
            ],
            "IsPredefined": true,
            "RoleId": "Operator",
        })
    ));
    tree.add_resource(RedfishResource::new(
        "/redfish/v1/AccountService/Roles/ReadOnly",
        String::from("Role"),
        RedfishResourceSchemaVersion::new(1, 3, 1),
        String::from("Role"),
        String::from("ReadOnly Role"),
        false,
        false,
        Some(String::from("/redfish/v1/AccountService/Roles")),
        json!({
            "AssignedPrivileges": [
                "ConfigureSelf",
                "Login",
            ],
            "IsPredefined": true,
            "RoleId": "ReadOnly",
        })
    ));
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

    async fn get_response_json(response: Response) -> Value {
        assert_eq!(response.headers().get("content-type").unwrap().to_str().unwrap(), "application/json");
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn jget(app: &mut NormalizePath<Router>, uri: &str, status_code: StatusCode, allow: &str) -> Value {
        let response = get(app, uri).await;
        assert_eq!(response.status(), status_code);
        assert_eq!(response.headers().get("OData-Version").unwrap().to_str().unwrap(), "4.0");
        assert_eq!(response.headers().get("allow").unwrap().to_str().unwrap(), allow);
        get_response_json(response).await
    }

    async fn delete(app: &mut NormalizePath<Router>, uri: &str) -> Response {
        let req = Request::delete(uri).body(Body::empty()).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    async fn post(app: &mut NormalizePath<Router>, uri: &str, req: serde_json::Value) -> Response {
        let body = Body::from(serde_json::to_vec(&req).unwrap());
        let req = Request::post(uri).header("Content-Type", "application/json").body(body).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    async fn patch(app: &mut NormalizePath<Router>, uri: &str, req: serde_json::Value) -> Response {
        let body = Body::from(serde_json::to_vec(&req).unwrap());
        let req = Request::patch(uri).header("Content-Type", "application/json").body(body).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    #[tokio::test]
    async fn base_redfish_path() {
        let mut app = app();
        let body = jget(&mut app, "/redfish", StatusCode::OK, "GET,HEAD").await;
        assert_eq!(body, json!({ "v1": "/redfish/v1/" }));
    }

    #[tokio::test]
    async fn head_redfish_v1() {
        let mut app = app();
        let req = Request::head("/redfish/v1").body(Body::empty()).unwrap();
        let response = app.ready().await.unwrap().call(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap().to_str().unwrap(), "application/json");
        assert_eq!(response.headers().get("OData-Version").unwrap().to_str().unwrap(), "4.0");
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn get_redfish_v1() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1", StatusCode::OK, "GET,HEAD").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1",
            "@odata.type": "#ServiceRoot.v1_15_0.ServiceRoot",
            "Id": "RootService",
            "Name": "Root Service",
            "AccountService": {
                "@odata.id": "/redfish/v1/AccountService",
            },
            "Links": {
                "Sessions": {
                    "@odata.id": "/redfish/v1/SessionService/Sessions"
                }
            }
        }));
    }

    #[tokio::test]
    async fn get_odata_service_doc() {
        let mut app = app();
        let body = jget(&mut app, ODATA_SERVICE_DOC_URI, StatusCode::OK, "GET,HEAD").await;
        assert_eq!(body, json!({
            "@odata.id": ODATA_SERVICE_DOC_URI,
            "@odata.context": "/redfish/v1/$metadata",
            "value": [
                {
                    "kind": "Singleton",
                    "name": "v1",
                    "url": "/redfish/v1",
                },
                {
                    "kind": "Singleton",
                    "name": "AccountService",
                    "url": "/redfish/v1/AccountService",
                },
                {
                    "kind": "Singleton",
                    "name": "SessionService",
                    "url": "/redfish/v1/SessionService",
                },
            ],
        }));
    }

    #[tokio::test]
    async fn get_odata_metadata_doc() {
        let mut app = app();
        let response = get(&mut app, "/redfish/v1/$metadata").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("OData-Version").unwrap().to_str().unwrap(), "4.0");
        assert_eq!(response.headers().get("allow").unwrap().to_str().unwrap(), "GET,HEAD");
        assert_eq!(response.headers().get("content-type").unwrap().to_str().unwrap(), "application/xml");

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        println!("{}", body);
        assert_eq!(body, r#"<?xml version="1.0" encoding="UTF-8"?>
<edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/SessionCollection_v1.xml">
    <edmx:Include Namespace="SessionCollection" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ManagerAccountCollection_v1.xml">
    <edmx:Include Namespace="ManagerAccountCollection" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/RoleCollection_v1.xml">
    <edmx:Include Namespace="RoleCollection" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ServiceRoot_v1.xml">
    <edmx:Include Namespace="ServiceRoot" />
    <edmx:Include Namespace="ServiceRoot.v1_15_0" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/SessionService_v1.xml">
    <edmx:Include Namespace="SessionService" />
    <edmx:Include Namespace="SessionService.v1_1_9" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/AccountService_v1.xml">
    <edmx:Include Namespace="AccountService" />
    <edmx:Include Namespace="AccountService.v1_12_0" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/ManagerAccount_v1.xml">
    <edmx:Include Namespace="ManagerAccount" />
    <edmx:Include Namespace="ManagerAccount.v1_10_0" />
  </edmx:Reference>
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/Role_v1.xml">
    <edmx:Include Namespace="Role" />
    <edmx:Include Namespace="Role.v1_3_1" />
  </edmx:Reference>
  <edmx:DataServices>
    <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Service">
      <EntityContainer Name="Service" Extends="ServiceRoot.v1_15_0.ServiceContainer" />
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>
"#);
    }

    #[tokio::test]
    async fn session_service() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/SessionService/", StatusCode::OK, "GET,HEAD,PATCH").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService",
            "@odata.type": "#SessionService.v1_1_9.SessionService",
            "@Redfish.WriteableProperties": ["SessionTimeout"],
            "Id": "SessionService",
            "Name": "Session Service",
            "SessionTimeout": 600,
            "Sessions" : {"@odata.id": "/redfish/v1/SessionService/Sessions"},
        }));
    }

    #[tokio::test]
    async fn empty_session_collection() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions", StatusCode::OK, "GET,HEAD,POST").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [],
            "Members@odata.count": 0,
        }));
    }

    #[tokio::test]
    async fn default_administrator_role() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/AccountService/Roles/Administrator", StatusCode::OK, "GET,HEAD").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/AccountService/Roles/Administrator",
            "@odata.type": "#Role.v1_3_1.Role",
            "Id": "Administrator",
            "Name": "Administrator Role",
            "AssignedPrivileges": [
                "Login",
                "ConfigureManager",
                "ConfigureUsers",
                "ConfigureSelf",
                "ConfigureComponents",
            ],
            "IsPredefined": true,
            "RoleId": "Administrator",
        }));
    }

    #[tokio::test]
    async fn default_operator_role() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/AccountService/Roles/Operator", StatusCode::OK, "GET,HEAD").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/AccountService/Roles/Operator",
            "@odata.type": "#Role.v1_3_1.Role",
            "Id": "Operator",
            "Name": "Operator Role",
            "AssignedPrivileges": [
                "Login",
                "ConfigureSelf",
                "ConfigureComponents",
            ],
            "IsPredefined": true,
            "RoleId": "Operator",
        }));
    }

    #[tokio::test]
    async fn default_readonly_role() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/AccountService/Roles/ReadOnly", StatusCode::OK, "GET,HEAD").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/AccountService/Roles/ReadOnly",
            "@odata.type": "#Role.v1_3_1.Role",
            "Id": "ReadOnly",
            "Name": "ReadOnly Role",
            "AssignedPrivileges": [
                "ConfigureSelf",
                "Login",
            ],
            "IsPredefined": true,
            "RoleId": "ReadOnly",
        }));
    }

    #[tokio::test]
    async fn delete_not_allowed() {
        let mut app = app();
        let response = delete(&mut app, "/redfish/v1").await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get("allow").unwrap().to_str().unwrap(), "GET,HEAD");
    }

    #[tokio::test]
    async fn post_not_allowed() {
        let mut app = app();
        let data = json!({});
        let response = post(&mut app, "/redfish/v1", data).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get("allow").unwrap().to_str().unwrap(), "GET,HEAD");
    }

    #[tokio::test]
    async fn patch_not_allowed() {
        let mut app = app();
        let data = json!({});
        let response = patch(&mut app, "/redfish/v1", data).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get("allow").unwrap().to_str().unwrap(), "GET,HEAD");
    }

    #[tokio::test]
    async fn happy_patch() {
        let mut app = app();
        let data = json!({"SessionTimeout": 300});
        let response = patch(&mut app, "/redfish/v1/SessionService", data).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("allow").unwrap().to_str().unwrap(), "GET,HEAD,PATCH");

        let body = get_response_json(response).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService",
            "@odata.type": "#SessionService.v1_1_9.SessionService",
            "@Redfish.WriteableProperties": ["SessionTimeout"],
            "Id": "SessionService",
            "Name": "Session Service",
            "SessionTimeout": 300,
            "Sessions" : {"@odata.id": "/redfish/v1/SessionService/Sessions"},
        }));

        let body = jget(&mut app, "/redfish/v1/SessionService/", StatusCode::OK, "GET,HEAD,PATCH").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService",
            "@odata.type": "#SessionService.v1_1_9.SessionService",
            "@Redfish.WriteableProperties": ["SessionTimeout"],
            "Id": "SessionService",
            "Name": "Session Service",
            "SessionTimeout": 300,
            "Sessions" : {"@odata.id": "/redfish/v1/SessionService/Sessions"},
        }));
    }

    #[tokio::test]
    async fn post_and_delete_session() {
        let mut app = app();
        let data = json!({"UserName": "Obiwan", "Password": "n/a"});
        let response = post(&mut app, "/redfish/v1/SessionService/Sessions", data).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers().get("OData-Version").unwrap().to_str().unwrap(), "4.0");
        assert_eq!(response.headers().get("Location").unwrap().to_str().unwrap(), "/redfish/v1/SessionService/Sessions/1");

        let body = get_response_json(response).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/1",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "1",
            "Name": "Session 1",
            "UserName": "Obiwan",
            "Password": serde_json::Value::Null,
        }));

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions/1", StatusCode::OK, "GET,HEAD,DELETE").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/1",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "1",
            "Name": "Session 1",
            "UserName": "Obiwan",
            "Password": serde_json::Value::Null,
        }));

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions", StatusCode::OK, "GET,HEAD,POST").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [
                {"@odata.id": "/redfish/v1/SessionService/Sessions/1"}
            ],
            "Members@odata.count": 1,
        }));

        let response = delete(&mut app, "/redfish/v1/SessionService/Sessions/1").await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions", StatusCode::OK, "GET,HEAD,POST").await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [],
            "Members@odata.count": 0,
        }));
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
    async fn delete_not_found() {
        let mut app = app();
        let response = delete(&mut app, "/redfish/v1/notfound").await;
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

    #[tokio::test]
    async fn head_not_found() {
        let mut app = app();
        let req = Request::head("/redfish/v1/notfound").body(Body::empty()).unwrap();
        let response = app.ready().await.unwrap().call(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn patch_not_found() {
        let mut app = app();
        let response = patch(&mut app, "/redfish/v1/notfound", json!({})).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }
}