use axum::{
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

#[tokio::main]
async fn main() {
    let app = NormalizePathLayer::trim_trailing_slash().layer(Router::new().route("/redfish", get(get_redfish)));

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
