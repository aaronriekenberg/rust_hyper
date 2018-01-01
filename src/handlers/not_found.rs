use hyper::header;
use hyper::server::Response;
use hyper::StatusCode;

use server;

use std::borrow::Cow;

pub struct NotFoundHandler;

impl server::RequestHandler for NotFoundHandler {

  fn use_threadpool(&self) -> bool { false }

  fn handle(&self, _: &server::RequestContext) -> Response {
    server::build_response_string(
      StatusCode::NotFound,
      Cow::from("Route not found"),
      header::ContentType::plaintext())
      .with_header(header::CacheControl(
                     vec![header::CacheDirective::MaxAge(0)]))
  }

}
