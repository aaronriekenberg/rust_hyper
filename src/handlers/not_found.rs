use hyper::{Body, Response, StatusCode};

use std::borrow::Cow;

pub struct NotFoundHandler;

impl ::server::RequestHandler for NotFoundHandler {

  fn handle(&self, _: &::server::RequestContext) -> Response<Body> {

    ::server::build_response_string(
      StatusCode::NOT_FOUND,
      Cow::from("Route not found"),
      "text/plain")
      //.with_header(header::CacheControl(
      //               vec![header::CacheDirective::MaxAge(0)]))

  }

}
