use futures::future;

use hyper::StatusCode;

use std::borrow::Cow;

pub struct NotFoundHandler;

impl crate::server::RequestHandler for NotFoundHandler {
    fn handle(&self, _: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        Box::new(future::ok(crate::server::build_response_string(
            StatusCode::NOT_FOUND,
            Cow::from("Route not found"),
            crate::server::text_plain_content_type_header_value(),
        )))
    }
}
