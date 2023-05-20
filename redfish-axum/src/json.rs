use http::{
    header::{self, HeaderValue}, HeaderName,
};
use axum::{
    http::StatusCode,
    response::{Response, IntoResponse, Json},
};
use serde_json::Value;

// JSON response that set additional headers required by redfish
pub struct JsonResponse {
    data: Value,
    // TODO: Maybe just store HeaderMap instead?
    allow: String,
    described_by: Option<String>,
}

impl JsonResponse {
    pub fn new(data: Value, allow: String, described_by: Option<String>) -> Self {
        Self { data, allow, described_by }
    }
}

impl IntoResponse for JsonResponse
{
    fn into_response(self) -> Response {
        let mut response = Json(self.data).into_response();
        let headers = response.headers_mut();
        headers.insert(header::ALLOW, HeaderValue::from_str(self.allow.as_str()).expect("FIXME"));
        headers.insert(HeaderName::from_static("odata-version"), HeaderValue::from_static("4.0"));
        headers.insert(HeaderName::from_static("cache-control"), HeaderValue::from_static("no-cache"));
        if self.described_by.is_some() {
            headers.insert(header::LINK, HeaderValue::from_str(self.described_by.unwrap().as_str()).expect("FIXME"));
        }
        response
    }
}