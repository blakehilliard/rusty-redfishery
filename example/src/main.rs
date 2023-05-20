use axum::{
    ServiceExt,
    Router,
};
use tower_http::normalize_path::{NormalizePath};
use serde_json::json;
use redfish_axum::RedfishNode;
use redfish_data::{
    RedfishResourceSchemaVersion,
    get_uri_id,
};

mod tree;
use tree::{MockTree, RedfishCollection, RedfishResource};

fn create_session(collection: &RedfishCollection, req: serde_json::Value) -> Result<RedfishResource, ()> {
    // Look at existing members to see next Id to pick
    let mut highest = 0;
    for member in collection.members.iter() {
        let id = get_uri_id(member.as_str());
        let id = id.parse().unwrap(); // TODO: Not so catastrophic?
        if id > highest {
            highest = id;
        }
    }
    let id = (highest + 1).to_string();
    let member_uri = format!("{}/{}", collection.get_uri(), id);

    // Return new resource
    Ok(RedfishResource::new(
        member_uri.as_str(),
        String::from("Session"),
        RedfishResourceSchemaVersion::new(1, 6, 0),
        String::from("Session"),
        String::from(format!("Session {}", id)),
        true,
        false,
        Some(String::from(collection.get_uri())),
        json!({
            "UserName": req.as_object().unwrap().get("UserName").unwrap().as_str(),
            "Password": serde_json::Value::Null,
        }),
    ))
}

fn get_mock_tree() -> MockTree {
    let mut tree = MockTree::new();
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
            "SessionService": {
                "@odata.id": "/redfish/v1/SessionService",
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
    tree.add_collection(RedfishCollection::new(
        "/redfish/v1/SessionService/Sessions",
        String::from("SessionCollection"),
        String::from("Session Collection"),
        vec![String::from("/redfish/v1/SessionService/Sessions/1")],
        Some(create_session),
    ));
    tree.add_resource(RedfishResource::new(
        "/redfish/v1/SessionService/Sessions/1",
        String::from("Session"),
        RedfishResourceSchemaVersion::new(1, 6, 0),
        String::from("Session"),
        String::from("Session 1"),
        true,
        false,
        Some(String::from("/redfish/v1/SessionService/Sessions")),
        json!({
            "UserName": "admin",
            "Password": serde_json::Value::Null,
        }),
    ));
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
    tree.add_collection(RedfishCollection::new(
        "/redfish/v1/AccountService/Accounts",
        String::from("ManagerAccountCollection"),
        String::from("Account Collection"),
        vec![String::from("/redfish/v1/AccountService/Accounts/admin")],
        None,
    ));
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
    tree.add_collection(RedfishCollection::new(
        "/redfish/v1/AccountService/Roles",
        String::from("RoleCollection"),
        String::from("Role Collection"),
        vec![
            String::from("/redfish/v1/AccountService/Roles/Administrator"),
            String::from("/redfish/v1/AccountService/Roles/Operator"),
            String::from("/redfish/v1/AccountService/Roles/ReadOnly"),
        ],
        None,
    ));
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

    async fn jget(app: &mut NormalizePath<Router>, uri: &str, status_code: StatusCode, allow: &str, schema_url: Option<&str>) -> Value {
        let response = get(app, uri).await;
        assert_eq!(response.status(), status_code);
        assert_eq!(response.headers().get("OData-Version").unwrap().to_str().unwrap(), "4.0");
        assert_eq!(response.headers().get("allow").unwrap().to_str().unwrap(), allow);
        assert_eq!(response.headers().get("cache-control").unwrap().to_str().unwrap(), "no-cache");
        if schema_url.is_some() {
            assert_eq!(response.headers().get("Link").unwrap().to_str().unwrap(), format!("<{}>; rel=describedby", schema_url.unwrap()));
        }
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
        let body = jget(&mut app, "/redfish", StatusCode::OK, "GET,HEAD", None).await;
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
        assert_eq!(response.headers().get("cache-control").unwrap().to_str().unwrap(), "no-cache");
        assert_eq!(response.headers().get("Link").unwrap().to_str().unwrap(), "<https://redfish.dmtf.org/schemas/v1/ServiceRoot.v1_15_0.json>; rel=describedby");
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn get_redfish_v1() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1", StatusCode::OK, "GET,HEAD", Some("https://redfish.dmtf.org/schemas/v1/ServiceRoot.v1_15_0.json")).await;
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
            },
            "SessionService": {
                "@odata.id": "/redfish/v1/SessionService",
            }
        }));
    }

    #[tokio::test]
    async fn get_odata_service_doc() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/odata", StatusCode::OK, "GET,HEAD", None).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/odata",
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
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/Session_v1.xml">
    <edmx:Include Namespace="Session" />
    <edmx:Include Namespace="Session.v1_6_0" />
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
        let body = jget(&mut app, "/redfish/v1/SessionService/", StatusCode::OK, "GET,HEAD,PATCH", Some("https://redfish.dmtf.org/schemas/v1/SessionService.v1_1_9.json")).await;
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
    async fn get_session_collection() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions", StatusCode::OK, "GET,HEAD,POST", Some("https://redfish.dmtf.org/schemas/v1/SessionCollection.json")).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [
                {"@odata.id": "/redfish/v1/SessionService/Sessions/1"},
            ],
            "Members@odata.count": 1,
        }));
    }

    #[tokio::test]
    async fn default_administrator_role() {
        let mut app = app();
        let body = jget(&mut app, "/redfish/v1/AccountService/Roles/Administrator", StatusCode::OK, "GET,HEAD", Some("https://redfish.dmtf.org/schemas/v1/Role.v1_3_1.json")).await;
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
        let body = jget(&mut app, "/redfish/v1/AccountService/Roles/Operator", StatusCode::OK, "GET,HEAD", Some("https://redfish.dmtf.org/schemas/v1/Role.v1_3_1.json")).await;
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
        let body = jget(&mut app, "/redfish/v1/AccountService/Roles/ReadOnly", StatusCode::OK, "GET,HEAD", Some("https://redfish.dmtf.org/schemas/v1/Role.v1_3_1.json")).await;
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
        assert_eq!(response.headers().get("cache-control").unwrap().to_str().unwrap(), "no-cache");
        assert_eq!(response.headers().get("Link").unwrap().to_str().unwrap(), "<https://redfish.dmtf.org/schemas/v1/SessionService.v1_1_9.json>; rel=describedby");

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

        let body = jget(&mut app, "/redfish/v1/SessionService/", StatusCode::OK, "GET,HEAD,PATCH", Some("https://redfish.dmtf.org/schemas/v1/SessionService.v1_1_9.json")).await;
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
        assert_eq!(response.headers().get("Location").unwrap().to_str().unwrap(), "/redfish/v1/SessionService/Sessions/2");
        assert_eq!(response.headers().get("cache-control").unwrap().to_str().unwrap(), "no-cache");
        assert_eq!(response.headers().get("Link").unwrap().to_str().unwrap(), "<https://redfish.dmtf.org/schemas/v1/Session.v1_6_0.json>; rel=describedby");

        let body = get_response_json(response).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/2",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "2",
            "Name": "Session 2",
            "UserName": "Obiwan",
            "Password": serde_json::Value::Null,
        }));

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions/1", StatusCode::OK, "GET,HEAD,DELETE", Some("https://redfish.dmtf.org/schemas/v1/Session.v1_6_0.json")).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/1",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "1",
            "Name": "Session 1",
            "UserName": "admin",
            "Password": serde_json::Value::Null,
        }));

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions/2", StatusCode::OK, "GET,HEAD,DELETE", Some("https://redfish.dmtf.org/schemas/v1/Session.v1_6_0.json")).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/2",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "2",
            "Name": "Session 2",
            "UserName": "Obiwan",
            "Password": serde_json::Value::Null,
        }));

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions", StatusCode::OK, "GET,HEAD,POST", Some("https://redfish.dmtf.org/schemas/v1/SessionCollection.json")).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [
                {"@odata.id": "/redfish/v1/SessionService/Sessions/1"},
                {"@odata.id": "/redfish/v1/SessionService/Sessions/2"},
            ],
            "Members@odata.count": 2,
        }));

        let response = delete(&mut app, "/redfish/v1/SessionService/Sessions/1").await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers().get("cache-control").unwrap().to_str().unwrap(), "no-cache");

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions", StatusCode::OK, "GET,HEAD,POST", Some("https://redfish.dmtf.org/schemas/v1/SessionCollection.json")).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions",
            "@odata.type": "#SessionCollection.SessionCollection",
            "Name": "Session Collection",
            "Members" : [
                {"@odata.id": "/redfish/v1/SessionService/Sessions/2"},
            ],
            "Members@odata.count": 1,
        }));

        let response = get(&mut app, "/redfish/v1/SessionService/Sessions/1").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = jget(&mut app, "/redfish/v1/SessionService/Sessions/2", StatusCode::OK, "GET,HEAD,DELETE", Some("https://redfish.dmtf.org/schemas/v1/Session.v1_6_0.json")).await;
        assert_eq!(body, json!({
            "@odata.id": "/redfish/v1/SessionService/Sessions/2",
            "@odata.type": "#Session.v1_6_0.Session",
            "Id": "2",
            "Name": "Session 2",
            "UserName": "Obiwan",
            "Password": serde_json::Value::Null,
        }));
    }

    #[tokio::test]
    async fn post_to_members() {
        let mut app = app();
        let data = json!({"UserName": "Obiwan", "Password": "n/a"});
        let response = post(&mut app, "/redfish/v1/SessionService/Sessions/Members", data).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers().get("OData-Version").unwrap().to_str().unwrap(), "4.0");
        assert_eq!(response.headers().get("Location").unwrap().to_str().unwrap(), "/redfish/v1/SessionService/Sessions/2");
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

    #[tokio::test]
    async fn get_bad_odata_version() {
        let mut app = app();
        let request = Request::get("/redfish/v1").header("OData-Version", "4.1").body(Body::empty()).unwrap();
        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    }

    #[tokio::test]
    async fn patch_bad_odata_version() {
        let mut app = app();
        let data = json!({"SessionTimeout": 300});
        let body = Body::from(serde_json::to_vec(&data).unwrap());
        let request = Request::patch("/redfish/v1/SessionService")
            .header("Content-Type", "application/json")
            .header("OData-Version", "4.1")
            .body(body)
            .unwrap();
        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    }

    #[tokio::test]
    async fn post_bad_odata_version() {
        let mut app = app();
        let data = json!({});
        let body = Body::from(serde_json::to_vec(&data).unwrap());
        let request = Request::post("/redfish/v1/SessionService/Sessions")
            .header("Content-Type", "application/json")
            .header("OData-Version", "4.1")
            .body(body)
            .unwrap();
        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    }

    #[tokio::test]
    async fn delete_bad_odata_version() {
        let mut app = app();
        let request = Request::delete("/redfish/v1/SessionService/Sessions/1").header("OData-Version", "4.1").body(Body::empty()).unwrap();
        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    }
}