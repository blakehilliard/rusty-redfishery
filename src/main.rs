use axum::{
    extract::Path,
    http::StatusCode,
    routing::get,
    response::Json,
    Router,
    ServiceExt,
};
use tower_http::normalize_path::NormalizePathLayer;
use tower::layer::Layer;
use serde_json::{Value, json};

struct RedfishResource {
    uri: String, //TODO: Enforce things here? Does DMTF recommend trailing slash or no?
    resource_type: String,
    schema_version: String, //TODO: Enforce conformity
    term_name: String, //TODO: Constructor where this is optional and derived from resource_type
    id: String, //TODO: Better name?
    name: String, //TODO: Better name?
}

impl RedfishResource {
    fn json(&self) -> Value {
        json!({
            "@odata.id": self.uri,
            "@odata.type": self.odata_type(),
            "Id": self.id,
            "Name": self.name,
        })
    }

    fn odata_type(&self) -> String {
        format!("#{}.{}.{}", self.resource_type, self.schema_version, self.term_name)
    }
}

async fn get_redfish() -> Json<Value> {
    Json(json!({ "v1": "/redfish/v1/" }))
}

async fn handle_redfish_path(Path(path): Path<String>) -> (StatusCode, Json<Value>) {
    let root = RedfishResource {
        uri: String::from("/redfish/v1"),
        resource_type: String::from("ServiceRoot"),
        schema_version: String::from("v1_15_0"),
        term_name: String::from("ServiceRoot"),
        id: String::from("RootService"),
        name: String::from("Root Service"),
    };
    let uri = "/redfish/".to_owned() + &path;
    if uri == "/redfish/v1" {
        return (StatusCode::OK, Json(root.json()));
    }
    (StatusCode::NOT_FOUND, Json(json!({"TODO": "FIXME"})))
}

#[tokio::main]
async fn main() {
    let layer = NormalizePathLayer::trim_trailing_slash();
    let app = Router::new()
        .route("/redfish", get(get_redfish))
        .route("/redfish/*path", get(handle_redfish_path));

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(layer.layer(app).into_make_service())
        .await
        .unwrap();
}
