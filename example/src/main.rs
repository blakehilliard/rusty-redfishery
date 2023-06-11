use axum::{Router, ServiceExt};
use redfish_axum::{Error, Node};
use redfish_data::{get_uri_id, ResourceSchemaVersion};
use serde_json::{json, Value};
use tower_http::normalize_path::NormalizePath;

mod tree;
use tree::{Collection, MockTree, Resource};

fn create_session(collection: &Collection, req: Value) -> Result<Resource, Error> {
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
    Ok(Resource::new(
        member_uri.as_str(),
        String::from("Session"),
        ResourceSchemaVersion::new(1, 6, 0),
        String::from("Session"),
        String::from(format!("Session {}", id)),
        Some(|_| Ok(())),
        None,
        Some(String::from(collection.get_uri())),
        json!({
            "UserName": req.as_object().unwrap().get("UserName").unwrap().as_str(),
            "Password": serde_json::Value::Null,
        }),
    ))
}

fn patch_session_service(resource: &mut Resource, req: Value) -> Result<(), Error> {
    // TODO: Allow patch that doesn't set this! And do correct error handling!
    let new_timeout = req
        .as_object()
        .unwrap()
        .get("SessionTimeout")
        .unwrap()
        .as_u64()
        .unwrap();
    resource.body["SessionTimeout"] = Value::from(new_timeout);
    Ok(())
}

fn get_mock_tree() -> MockTree {
    let mut tree = MockTree::new();
    tree.add_resource(Resource::new(
        "/redfish/v1",
        String::from("ServiceRoot"),
        ResourceSchemaVersion::new(1, 15, 0),
        String::from("ServiceRoot"),
        String::from("Root Service"),
        None,
        None,
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
        }),
    ));
    tree.add_resource(Resource::new(
        "/redfish/v1/SessionService",
        String::from("SessionService"),
        ResourceSchemaVersion::new(1, 1, 8),
        String::from("SessionService"),
        String::from("Session Service"),
        None,
        Some(patch_session_service),
        None,
        json!({
            "@Redfish.WriteableProperties": ["SessionTimeout"],
            "SessionTimeout": 600,
            "Sessions": {
                "@odata.id": "/redfish/v1/SessionService/Sessions"
            },
        }),
    ));
    tree.add_collection(Collection::new(
        "/redfish/v1/SessionService/Sessions",
        String::from("SessionCollection"),
        String::from("Session Collection"),
        Vec::new(),
        Some(create_session),
    ));
    tree.add_resource(Resource::new(
        "/redfish/v1/AccountService",
        String::from("AccountService"),
        ResourceSchemaVersion::new(1, 12, 0),
        String::from("AccountService"),
        String::from("Account Service"),
        None,
        None,
        None,
        json!({
            "Accounts": {
                "@odata.id": "/redfish/v1/AccountService/Accounts"
            },
            "Roles": {
                "@odata.id": "/redfish/v1/AccountService/Roles"
            }
        }),
    ));
    tree.add_collection(Collection::new(
        "/redfish/v1/AccountService/Accounts",
        String::from("ManagerAccountCollection"),
        String::from("Account Collection"),
        vec![String::from("/redfish/v1/AccountService/Accounts/admin")],
        None,
    ));
    tree.add_resource(Resource::new(
        "/redfish/v1/AccountService/Accounts/admin",
        String::from("ManagerAccount"),
        ResourceSchemaVersion::new(1, 10, 0),
        String::from("ManagerAccount"),
        String::from("Admin Account"),
        None,
        None,
        Some(String::from("/redfish/v1/AccountService/Accounts")),
        json!({
            "@Redfish.WriteableProperties": ["Password"],
            "AccountTypes": ["Redfish"],
            "Links": {
                "Role": {
                    "@odata.id": "/redfish/v1/AccountService/Roles/Administrator"
                }
            },
            "Password": null,
            "RoleId": "Administrator",
            "UserName": "admin",
        }),
    ));
    tree.add_collection(Collection::new(
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
    tree.add_resource(Resource::new(
        "/redfish/v1/AccountService/Roles/Administrator",
        String::from("Role"),
        ResourceSchemaVersion::new(1, 3, 1),
        String::from("Role"),
        String::from("Administrator Role"),
        None,
        None,
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
        }),
    ));
    tree.add_resource(Resource::new(
        "/redfish/v1/AccountService/Roles/Operator",
        String::from("Role"),
        ResourceSchemaVersion::new(1, 3, 1),
        String::from("Role"),
        String::from("Operator Role"),
        None,
        None,
        Some(String::from("/redfish/v1/AccountService/Roles")),
        json!({
            "AssignedPrivileges": [
                "Login",
                "ConfigureSelf",
                "ConfigureComponents",
            ],
            "IsPredefined": true,
            "RoleId": "Operator",
        }),
    ));
    tree.add_resource(Resource::new(
        "/redfish/v1/AccountService/Roles/ReadOnly",
        String::from("Role"),
        ResourceSchemaVersion::new(1, 3, 1),
        String::from("Role"),
        String::from("ReadOnly Role"),
        None,
        None,
        Some(String::from("/redfish/v1/AccountService/Roles")),
        json!({
            "AssignedPrivileges": [
                "ConfigureSelf",
                "Login",
            ],
            "IsPredefined": true,
            "RoleId": "ReadOnly",
        }),
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
        body::Body,
        http::{Request, StatusCode},
        response::Response,
    };
    use http::{request::Builder, HeaderValue};
    use serde_json::{json, Value};
    use tower::{Service, ServiceExt};

    enum Auth {
        Token(String),
        Basic(String),
        None,
    }

    fn add_auth_headers(req: &mut Builder, auth: &Auth) {
        match auth {
            Auth::Token(token) => {
                let headers = req.headers_mut().unwrap();
                headers.insert("x-auth-token", HeaderValue::from_str(&token).unwrap());
            }
            Auth::Basic(header_val) => {
                let headers = req.headers_mut().unwrap();
                headers.insert("authorization", HeaderValue::from_str(&header_val).unwrap());
            }
            _ => (),
        }
    }

    // Return Auth::Basic for admin/admin credentials
    fn admin_admin_basic_auth() -> Auth {
        Auth::Basic(String::from("Basic YWRtaW46YWRtaW4="))
    }

    async fn get(app: &mut NormalizePath<Router>, uri: &str, auth: &Auth) -> Response {
        let mut req = Request::get(uri);
        add_auth_headers(&mut req, auth);
        let req = req.body(Body::empty()).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    async fn get_response_json(response: Response) -> Value {
        assert_eq!(get_header(&response, "content-type"), "application/json");
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn jget(
        app: &mut NormalizePath<Router>,
        uri: &str,
        status_code: StatusCode,
        auth: &Auth,
        headers: &[(&str, &str)],
    ) -> Value {
        let response = get(app, uri, auth).await;
        assert_eq!(response.status(), status_code);
        assert_eq!(get_header(&response, "OData-Version"), "4.0");
        assert_eq!(get_header(&response, "cache-control"), "no-cache");
        for (key, val) in headers {
            assert_eq!(get_header(&response, *key), *val);
        }
        get_response_json(response).await
    }

    fn get_header<'a>(response: &'a Response, key: &str) -> &'a str {
        response.headers().get(key).unwrap().to_str().unwrap()
    }

    async fn login(app: &mut NormalizePath<Router>) -> (Auth, String) {
        let data = json!({"UserName": "Obiwan", "Password": "n/a"});
        let response = post(
            app,
            "/redfish/v1/SessionService/Sessions",
            data,
            &Auth::None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(get_header(&response, "OData-Version"), "4.0");
        assert_eq!(
            get_header(&response, "Location"),
            "/redfish/v1/SessionService/Sessions/1"
        );
        assert_eq!(get_header(&response, "cache-control"), "no-cache");
        assert_eq!(
            get_header(&response, "Link"),
            "<https://redfish.dmtf.org/schemas/v1/Session.v1_6_0.json>; rel=describedby"
        );
        (
            Auth::Token(get_header(&response, "X-Auth-Token").to_string()),
            get_header(&response, "Location").to_string(),
        )
    }

    async fn delete(app: &mut NormalizePath<Router>, uri: &str, auth: &Auth) -> Response {
        let mut req = Request::delete(uri);
        add_auth_headers(&mut req, auth);
        let req = req.body(Body::empty()).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    async fn post(
        app: &mut NormalizePath<Router>,
        uri: &str,
        req: serde_json::Value,
        auth: &Auth,
    ) -> Response {
        let body = Body::from(serde_json::to_vec(&req).unwrap());
        let mut req = Request::post(uri).header("Content-Type", "application/json");
        add_auth_headers(&mut req, auth);
        let req = req.body(body).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    async fn patch(
        app: &mut NormalizePath<Router>,
        uri: &str,
        req: serde_json::Value,
        auth: &Auth,
    ) -> Response {
        let body = Body::from(serde_json::to_vec(&req).unwrap());
        let mut req = Request::patch(uri).header("Content-Type", "application/json");
        add_auth_headers(&mut req, auth);
        let req = req.body(body).unwrap();
        app.ready().await.unwrap().call(req).await.unwrap()
    }

    fn validate_unauthorized(response: &Response) {
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            get_header(&response, "www-authenticate"),
            "Basic realm=\"simple\""
        );
    }

    #[tokio::test]
    async fn base_redfish_path() {
        let mut app = app();
        let body = jget(
            &mut app,
            "/redfish",
            StatusCode::OK,
            &Auth::None,
            &[("allow", "GET,HEAD")],
        )
        .await;
        assert_eq!(body, json!({ "v1": "/redfish/v1/" }));
    }

    #[tokio::test]
    async fn head_redfish_v1() {
        let mut app = app();
        let req = Request::head("/redfish/v1").body(Body::empty()).unwrap();
        let response = app.ready().await.unwrap().call(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(get_header(&response, "content-type"), "application/json");
        assert_eq!(get_header(&response, "OData-Version"), "4.0");
        assert_eq!(get_header(&response, "cache-control"), "no-cache");
        assert_eq!(
            get_header(&response, "Link"),
            "<https://redfish.dmtf.org/schemas/v1/ServiceRoot.v1_15_0.json>; rel=describedby"
        );
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn get_redfish_v1() {
        let mut app = app();
        let body = jget(
            &mut app, "/redfish/v1", StatusCode::OK, &Auth::None,
            &[
                ("allow", "GET,HEAD"),
                ("link", "<https://redfish.dmtf.org/schemas/v1/ServiceRoot.v1_15_0.json>; rel=describedby"),
            ]).await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
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
            })
        );
    }

    #[tokio::test]
    async fn get_odata_service_doc() {
        let mut app = app();
        let body = jget(
            &mut app,
            "/redfish/v1/odata",
            StatusCode::OK,
            &Auth::None,
            &[("allow", "GET,HEAD")],
        )
        .await;
        assert_eq!(
            body,
            json!({
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
            })
        );
    }

    #[tokio::test]
    async fn get_odata_metadata_doc() {
        let mut app = app();
        let response = get(&mut app, "/redfish/v1/$metadata", &Auth::None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(get_header(&response, "OData-Version"), "4.0");
        assert_eq!(get_header(&response, "allow"), "GET,HEAD");
        assert_eq!(get_header(&response, "content-type"), "application/xml");

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        assert_eq!(
            body,
            r#"<?xml version="1.0" encoding="UTF-8"?>
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
    <edmx:Include Namespace="SessionService.v1_1_8" />
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
  <edmx:Reference Uri="http://redfish.dmtf.org/schemas/v1/RedfishExtensions_v1.xml">
    <edmx:Include Namespace="RedfishExtensions.v1_0_0" Alias="Redfish"/>
  </edmx:Reference>
  <edmx:DataServices>
    <Schema xmlns="http://docs.oasis-open.org/odata/ns/edm" Namespace="Service">
      <EntityContainer Name="Service" Extends="ServiceRoot.v1_15_0.ServiceContainer" />
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>
"#
        );
    }

    #[tokio::test]
    async fn get_missing_auth() {
        let mut app = app();
        let response = get(&mut app, "/redfish/v1/SessionService", &Auth::None).await;
        validate_unauthorized(&response);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn post_missing_auth() {
        let mut app = app();
        let response = post(&mut app, "/redfish/v1", json!({}), &Auth::None).await;
        validate_unauthorized(&response);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn delete_missing_auth() {
        let mut app = app();
        let response = delete(&mut app, "/redfish/v1", &Auth::None).await;
        validate_unauthorized(&response);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn patch_missing_auth() {
        let mut app = app();
        let response = patch(&mut app, "/redfish/v1", json!({}), &Auth::None).await;
        validate_unauthorized(&response);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn get_session_service() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let body = jget(
            &mut app, "/redfish/v1/SessionService/", StatusCode::OK, &token,
            &[
                ("allow", "GET,HEAD,PATCH"),
                ("link", "<https://redfish.dmtf.org/schemas/v1/SessionService.v1_1_8.json>; rel=describedby"),
                ("etag", "\"HARDCODED_ETAG\""),
            ],
        ).await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService",
                "@odata.type": "#SessionService.v1_1_8.SessionService",
                "@Redfish.WriteableProperties": ["SessionTimeout"],
                "Id": "SessionService",
                "Name": "Session Service",
                "SessionTimeout": 600,
                "Sessions" : {"@odata.id": "/redfish/v1/SessionService/Sessions"},
            })
        );
    }

    #[tokio::test]
    async fn get_session_collection() {
        let mut app = app();
        let body = jget(
            &mut app,
            "/redfish/v1/SessionService/Sessions",
            StatusCode::OK,
            &admin_admin_basic_auth(),
            &[
                ("allow", "GET,HEAD,POST"),
                (
                    "link",
                    "<https://redfish.dmtf.org/schemas/v1/SessionCollection.json>; rel=describedby",
                ),
                ("etag", "\"HARDCODED_ETAG\""),
            ],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService/Sessions",
                "@odata.type": "#SessionCollection.SessionCollection",
                "Name": "Session Collection",
                "Members" : [],
                "Members@odata.count": 0,
            })
        );
    }

    #[tokio::test]
    async fn default_administrator_role() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let body = jget(
            &mut app,
            "/redfish/v1/AccountService/Roles/Administrator",
            StatusCode::OK,
            &token,
            &[],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
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
            })
        );
    }

    #[tokio::test]
    async fn default_operator_role() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let body = jget(
            &mut app,
            "/redfish/v1/AccountService/Roles/Operator",
            StatusCode::OK,
            &token,
            &[],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
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
            })
        );
    }

    #[tokio::test]
    async fn default_readonly_role() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let body = jget(
            &mut app,
            "/redfish/v1/AccountService/Roles/ReadOnly",
            StatusCode::OK,
            &token,
            &[],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
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
            })
        );
    }

    #[tokio::test]
    async fn delete_not_allowed() {
        let mut app = app();
        let (token, _) = login(&mut app).await;

        let response = delete(&mut app, "/redfish/v1", &token).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD");

        let response = delete(&mut app, "/redfish/v1/SessionService/Sessions", &token).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD,POST");
    }

    #[tokio::test]
    async fn post_not_allowed() {
        let mut app = app();
        let (token, _) = login(&mut app).await;

        let response = post(&mut app, "/redfish/v1", json!({}), &token).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD");

        let response = post(&mut app, "/redfish/v1/SessionService", json!({}), &token).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD,PATCH");
    }

    #[tokio::test]
    async fn patch_not_allowed() {
        let mut app = app();
        let (token, _) = login(&mut app).await;

        let response = patch(&mut app, "/redfish/v1", json!({}), &token).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD");

        let response = patch(
            &mut app,
            "/redfish/v1/SessionService/Sessions",
            json!({}),
            &token,
        )
        .await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD,POST");
    }

    #[tokio::test]
    async fn happy_patch() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let data = json!({"SessionTimeout": 300});

        let response = patch(&mut app, "/redfish/v1/SessionService", data, &token).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD,PATCH");
        assert_eq!(get_header(&response, "cache-control"), "no-cache");
        assert_eq!(
            get_header(&response, "Link"),
            "<https://redfish.dmtf.org/schemas/v1/SessionService.v1_1_8.json>; rel=describedby"
        );
        assert_eq!(get_header(&response, "etag"), "\"HARDCODED_ETAG\"");

        let body = get_response_json(response).await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService",
                "@odata.type": "#SessionService.v1_1_8.SessionService",
                "@Redfish.WriteableProperties": ["SessionTimeout"],
                "Id": "SessionService",
                "Name": "Session Service",
                "SessionTimeout": 300,
                "Sessions" : {"@odata.id": "/redfish/v1/SessionService/Sessions"},
            })
        );

        let body = jget(
            &mut app,
            "/redfish/v1/SessionService/",
            StatusCode::OK,
            &token,
            &[("allow", "GET,HEAD,PATCH")],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService",
                "@odata.type": "#SessionService.v1_1_8.SessionService",
                "@Redfish.WriteableProperties": ["SessionTimeout"],
                "Id": "SessionService",
                "Name": "Session Service",
                "SessionTimeout": 300,
                "Sessions" : {"@odata.id": "/redfish/v1/SessionService/Sessions"},
            })
        );

        // Ensure basic auth works too
        let data = json!({"SessionTimeout": 600});
        let response = patch(
            &mut app,
            "/redfish/v1/SessionService",
            data,
            &admin_admin_basic_auth(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(get_header(&response, "allow"), "GET,HEAD,PATCH");
        assert_eq!(get_header(&response, "cache-control"), "no-cache");
        assert_eq!(
            get_header(&response, "Link"),
            "<https://redfish.dmtf.org/schemas/v1/SessionService.v1_1_8.json>; rel=describedby"
        );
    }

    #[tokio::test]
    async fn post_and_delete_session() {
        let mut app = app();

        // Create session 1
        let data = json!({"UserName": "admin", "Password": "admin"});
        let response = post(
            &mut app,
            "/redfish/v1/SessionService/Sessions",
            data,
            &Auth::None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(get_header(&response, "OData-Version"), "4.0");
        assert_eq!(
            get_header(&response, "Location"),
            "/redfish/v1/SessionService/Sessions/1"
        );
        assert_eq!(get_header(&response, "cache-control"), "no-cache");
        assert_eq!(
            get_header(&response, "Link"),
            "<https://redfish.dmtf.org/schemas/v1/Session.v1_6_0.json>; rel=describedby"
        );
        assert_eq!(get_header(&response, "etag"), "\"HARDCODED_ETAG\"");
        let token1 = Auth::Token(get_header(&response, "X-Auth-Token").to_string());

        // Create session 2
        let data = json!({"UserName": "Obiwan", "Password": "n/a"});
        let response = post(
            &mut app,
            "/redfish/v1/SessionService/Sessions",
            data,
            &Auth::None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(get_header(&response, "OData-Version"), "4.0");
        assert_eq!(
            get_header(&response, "Location"),
            "/redfish/v1/SessionService/Sessions/2"
        );
        assert_eq!(get_header(&response, "cache-control"), "no-cache");
        assert_eq!(
            get_header(&response, "Link"),
            "<https://redfish.dmtf.org/schemas/v1/Session.v1_6_0.json>; rel=describedby"
        );
        assert_eq!(get_header(&response, "etag"), "\"HARDCODED_ETAG\"");
        let token2 = Auth::Token(get_header(&response, "X-Auth-Token").to_string());

        let body = get_response_json(response).await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService/Sessions/2",
                "@odata.type": "#Session.v1_6_0.Session",
                "Id": "2",
                "Name": "Session 2",
                "UserName": "Obiwan",
                "Password": serde_json::Value::Null,
            })
        );

        // GET the sessions and collection, ensure both tokens work
        let body = jget(
            &mut app,
            "/redfish/v1/SessionService/Sessions/1",
            StatusCode::OK,
            &token2,
            &[("allow", "GET,HEAD,DELETE")],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService/Sessions/1",
                "@odata.type": "#Session.v1_6_0.Session",
                "Id": "1",
                "Name": "Session 1",
                "UserName": "admin",
                "Password": serde_json::Value::Null,
            })
        );

        let body = jget(
            &mut app,
            "/redfish/v1/SessionService/Sessions/2",
            StatusCode::OK,
            &token1,
            &[("allow", "GET,HEAD,DELETE")],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService/Sessions/2",
                "@odata.type": "#Session.v1_6_0.Session",
                "Id": "2",
                "Name": "Session 2",
                "UserName": "Obiwan",
                "Password": serde_json::Value::Null,
            })
        );

        let body = jget(
            &mut app,
            "/redfish/v1/SessionService/Sessions",
            StatusCode::OK,
            &token2,
            &[("allow", "GET,HEAD,POST")],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService/Sessions",
                "@odata.type": "#SessionCollection.SessionCollection",
                "Name": "Session Collection",
                "Members" : [
                    {"@odata.id": "/redfish/v1/SessionService/Sessions/1"},
                    {"@odata.id": "/redfish/v1/SessionService/Sessions/2"},
                ],
                "Members@odata.count": 2,
            })
        );

        // DELETE a session
        let response = delete(&mut app, "/redfish/v1/SessionService/Sessions/1", &token2).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .unwrap()
                .to_str()
                .unwrap(),
            "no-cache"
        );

        // Ensure it is gone and that remaining token still works
        let body = jget(
            &mut app,
            "/redfish/v1/SessionService/Sessions",
            StatusCode::OK,
            &token2,
            &[("allow", "GET,HEAD,POST")],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService/Sessions",
                "@odata.type": "#SessionCollection.SessionCollection",
                "Name": "Session Collection",
                "Members" : [
                    {"@odata.id": "/redfish/v1/SessionService/Sessions/2"},
                ],
                "Members@odata.count": 1,
            })
        );

        let response = get(&mut app, "/redfish/v1/SessionService/Sessions/1", &token2).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = jget(
            &mut app,
            "/redfish/v1/SessionService/Sessions/2",
            StatusCode::OK,
            &token2,
            &[("allow", "GET,HEAD,DELETE")],
        )
        .await;
        assert_eq!(
            body,
            json!({
                "@odata.etag": "\"HARDCODED_ETAG\"",
                "@odata.id": "/redfish/v1/SessionService/Sessions/2",
                "@odata.type": "#Session.v1_6_0.Session",
                "Id": "2",
                "Name": "Session 2",
                "UserName": "Obiwan",
                "Password": serde_json::Value::Null,
            })
        );

        // Ensure token of deleted session does not work
        let response = get(&mut app, "/redfish/v1/SessionService/Sessions", &token1).await;
        validate_unauthorized(&response);

        // DELETE other session using basic auth
        let response = delete(
            &mut app,
            "/redfish/v1/SessionService/Sessions/2",
            &admin_admin_basic_auth(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .unwrap()
                .to_str()
                .unwrap(),
            "no-cache"
        );

        // Ensure its token doesn't work anymore either
        let response = get(&mut app, "/redfish/v1/SessionService/Sessions", &token2).await;
        validate_unauthorized(&response);
    }

    #[tokio::test]
    async fn post_to_members() {
        let mut app = app();
        let data = json!({"UserName": "Obiwan", "Password": "n/a"});
        let response = post(
            &mut app,
            "/redfish/v1/SessionService/Sessions/Members",
            data,
            &Auth::None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(get_header(&response, "OData-Version"), "4.0");
        assert_eq!(
            get_header(&response, "Location"),
            "/redfish/v1/SessionService/Sessions/1"
        );
    }

    #[tokio::test]
    async fn post_not_found() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let response = post(&mut app, "/redfish/v1/notfound", json!({}), &token).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn delete_not_found() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let response = delete(&mut app, "/redfish/v1/notfound", &token).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn get_not_found() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let response = get(&mut app, "/redfish/v1/notfound", &token).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn head_not_found() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let mut req = Request::head("/redfish/v1/notfound");
        add_auth_headers(&mut req, &token);
        let req = req.body(Body::empty()).unwrap();
        let response = app.ready().await.unwrap().call(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn patch_not_found() {
        let mut app = app();
        let (token, _) = login(&mut app).await;
        let response = patch(&mut app, "/redfish/v1/notfound", json!({}), &token).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn get_bad_odata_version() {
        let mut app = app();
        let request = Request::get("/redfish/v1")
            .header("OData-Version", "4.1")
            .body(Body::empty())
            .unwrap();
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
        let request = Request::delete("/redfish/v1/SessionService/Sessions/1")
            .header("OData-Version", "4.1")
            .body(Body::empty())
            .unwrap();
        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    }
}
