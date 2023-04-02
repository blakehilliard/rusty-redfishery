use axum::{
    extract::Path,
    routing::get,
    response::Json,
    Router,
    ServiceExt,
};
use tower_http::normalize_path::NormalizePathLayer;
use tower::layer::Layer;
use serde_json::{Value, json};

async fn get_redfish() -> Json<Value> {
    Json(json!({ "v1": "/redfish/v1/" }))
}

async fn handle_redfish_path(Path(path): Path<String>) -> Json<Value> {
    Json(json!({"TODO": "/redfish/".to_owned() + &path}))
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
