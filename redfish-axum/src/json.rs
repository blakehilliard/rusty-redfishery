use bytes::{BufMut, BytesMut};
use http::{
    header::{self, HeaderValue},
};
use axum::{
    http::StatusCode,
    response::{Response, IntoResponse},
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
    pub fn new(data: Value, allow: String, described_by: Option<&str>) -> Self {
        let described_by = match described_by {
            None => None,
            Some(x) => Some(String::from(x))
        };
        Self { data, allow, described_by }
    }
}

impl IntoResponse for JsonResponse
{
    fn into_response(self) -> Response {
        let mut buf = BytesMut::with_capacity(128).writer();

        match serde_json::to_writer(&mut buf, &self.data) {
            Ok(()) => {
                let mut response = (
                    [(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                    )],
                    [(
                        header::ALLOW,
                        self.allow.as_str(),
                    )],
                    [("OData-Version", "4.0")],
                    [("Cache-Control", "no-cache")],
                    buf.into_inner().freeze(),
                ).into_response();
                if self.described_by.is_some() {
                    let headers = response.headers_mut();
                    let link = format!("<{}>; rel=describedby", self.described_by.unwrap());
                    headers.append(header::LINK, HeaderValue::from_str(link.as_str()).expect("FIXME"));
                }
                response
            },
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                )],
                [("Cache-Control", "no-cache")],
                err.to_string(),
            ).into_response(),
        }
    }
}