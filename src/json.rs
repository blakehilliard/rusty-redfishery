use bytes::{BufMut, BytesMut};
use serde::Serialize;
use http::{
    header::{self, HeaderValue},
};
use axum::{
    http::StatusCode,
    response::{Response, IntoResponse},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonGetResponse<T>(pub T);

impl<T> From<T> for JsonGetResponse<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> IntoResponse for JsonGetResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        let mut buf = BytesMut::with_capacity(128).writer();
        match serde_json::to_writer(&mut buf, &self.0) {
            Ok(()) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                )],
                [(
                    header::ALLOW,
                    HeaderValue::from_static("GET,HEAD"),
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