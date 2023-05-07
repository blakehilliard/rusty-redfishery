use bytes::{BufMut, BytesMut};
use serde::Serialize;
use http::{
    header::{self, HeaderValue},
};
use axum::{
    http::StatusCode,
    response::{Response, IntoResponse},
};

pub struct JsonGetResponse<T> {
    pub data: T,
    pub allow: String,
}

impl<T> IntoResponse for JsonGetResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        let mut buf = BytesMut::with_capacity(128).writer();
        match serde_json::to_writer(&mut buf, &self.data) {
            Ok(()) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                )],
                [(
                    header::ALLOW,
                    self.allow.as_str(),
                )],
                [(
                    "OData-Version",
                    "4.0",
                )],
                buf.into_inner().freeze(),
            )
                .into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                )],
                err.to_string(),
            )
                .into_response(),
        }
    }
}