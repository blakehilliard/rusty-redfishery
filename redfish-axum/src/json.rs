use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use http::header::HeaderMap;
use serde_json::Value;

// JSON response that allows customizing status code and headers
pub struct JsonResponse {
    status: StatusCode,
    headers: HeaderMap,
    data: Value,
}

impl JsonResponse {
    pub fn new(status: StatusCode, headers: HeaderMap, data: Value) -> Self {
        Self {
            status,
            headers,
            data,
        }
    }
}

impl IntoResponse for JsonResponse {
    fn into_response(self) -> Response {
        let mut response = Json(self.data).into_response();
        *response.status_mut() = self.status;
        response.headers_mut().extend(self.headers);
        response
    }
}
