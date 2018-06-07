use futures::Future;

use hyper::{Body, Response, Request, Server, StatusCode};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::service::service_fn;

use std::borrow::Cow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug)]
pub struct RequestContext {
  req: Request<Body>,
  start_time: Instant
}

impl RequestContext {

  fn new(req: Request<Body>) -> Self {
    RequestContext {
      req,
      start_time: Instant::now()
    }
  }

}

struct RequestLogInfo {
  start_time: Instant,
  method: String,
  uri: String,
  version: String
}

impl RequestLogInfo {

  fn new(req_context: &RequestContext) -> Self {
    let req = &req_context.req;

    RequestLogInfo {
      start_time: req_context.start_time,
      method: req.method().to_string(),
      uri: req.uri().to_string(),
      version: format!("{:?}", req.version())
    }
  }

}

fn log_request_and_response(
  req_log_info: &RequestLogInfo,
  resp: &Response<Body>) {

  let response_status = resp.status().as_u16().to_string();

  let duration = ::utils::duration_in_seconds_f64(&req_log_info.start_time.elapsed());

  info!("\"{} {} {}\" {} {:.9}s",
        req_log_info.method, req_log_info.uri, req_log_info.version,
        response_status,
        duration);
}

pub type ResponseFuture = Box<Future<Item=::hyper::Response<::hyper::Body>, Error=::std::io::Error> + Send>;

pub trait RequestHandler : Send + Sync {

  fn handle(&self, req_context: &RequestContext) -> ResponseFuture;

}

pub type RouteConfigurationHandler = Arc<RequestHandler>;
pub type RouteConfigurationHandlerMap = HashMap<String, RouteConfigurationHandler>;

pub struct RouteConfiguration {
  path_to_handler: RouteConfigurationHandlerMap,
  not_found_handler: RouteConfigurationHandler
}

impl RouteConfiguration {

  pub fn new(
    path_to_handler: RouteConfigurationHandlerMap,
    not_found_handler: RouteConfigurationHandler) -> Self {
    RouteConfiguration {
      path_to_handler,
      not_found_handler
    }
  }

  pub fn path_to_handler(&self) -> &RouteConfigurationHandlerMap {
    &self.path_to_handler
  }

  pub fn not_found_handler(&self) -> &RouteConfigurationHandler {
    &self.not_found_handler
  }

}

pub fn text_plain_content_type_header_value() -> HeaderValue {
  HeaderValue::from_static("text/plain")
}

pub fn text_html_content_type_header_value() -> HeaderValue {
  HeaderValue::from_static("text/html")
}

pub fn build_response_status(
  status_code: StatusCode) -> Response<Body> {

  Response::builder()
    .status(status_code)
    .body(Body::empty())
    .unwrap()

}

pub fn build_response_string(
  status_code: StatusCode,
  body: Cow<'static, str>,
  content_type: HeaderValue) -> Response<Body> {

  Response::builder()
    .status(status_code)
    .header(CONTENT_TYPE, content_type)
    .body(From::from(body))
    .unwrap()
}

pub fn build_response_vec(
  status_code: StatusCode,
  body: Vec<u8>,
  content_type: HeaderValue) -> Response<Body> {

  Response::builder()
    .status(status_code)
    .header(CONTENT_TYPE, content_type)
    .body(From::from(body))
    .unwrap()
}


struct InnerThreadedServer {
  route_configuration: RouteConfiguration
}

#[derive(Clone)]
struct ThreadedServer {
  inner: Arc<InnerThreadedServer>
}

impl ThreadedServer {

  fn new(
    route_configuration: RouteConfiguration) -> Self {

    ThreadedServer {
      inner: Arc::new(
        InnerThreadedServer {
          route_configuration
        }
      )
    }
  }

}

impl ThreadedServer {

  fn call(&self, req: Request<Body>) -> ResponseFuture {

    let req_context = RequestContext::new(req);

    let req_log_info = RequestLogInfo::new(&req_context);

    let route_configuration = &self.inner.route_configuration;

    let handler = route_configuration.path_to_handler()
      .get(req_context.req.uri().path())
      .unwrap_or(route_configuration.not_found_handler());

    Box::new(
      handler.handle(&req_context)
        .then(move |result| {
          match result {
            Ok(resp) => {
              log_request_and_response(&req_log_info, &resp);
              Ok(resp)
            },
            Err(e) => {
              warn!("handler error: {}", e);
              let resp = build_response_status(StatusCode::INTERNAL_SERVER_ERROR);
              log_request_and_response(&req_log_info, &resp);
              Ok(resp)
            }
          }
        }))
  }

}

fn run_server(
  listen_addr: SocketAddr,
  threaded_server: ThreadedServer) -> Result<(), Box<::std::error::Error>> {

  let server = Server::bind(&listen_addr)
    .serve(move || {
      let threaded_server_clone = threaded_server.clone();

      service_fn(move |req: Request<Body>| {
        threaded_server_clone.call(req)
      })

    })
    .map_err(|e| warn!("server error: {}", e));

  info!("Listening on http://{}", listen_addr);

  ::hyper::rt::run(server);

  Err(From::from("run_server exiting"))
}

pub fn run_forever(
  listen_addr: SocketAddr,
  route_configuration: RouteConfiguration) -> Result<(), Box<::std::error::Error>> {

  let threaded_server = ThreadedServer::new(
    route_configuration);

  run_server(listen_addr, threaded_server)
}
