use futures::Future;
use futures::future::poll_fn;

use hyper::{Body, Response, Request, Server, StatusCode};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::service::service_fn;

use std::borrow::Cow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio_threadpool::blocking;

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

  pub fn req(&self) -> &Request<Body> {
    &self.req
  }

  pub fn start_time(&self) -> &Instant {
    &self.start_time
  }

}

pub trait RequestHandler : Send + Sync {

  fn blocking(&self) -> bool { false }

  fn handle(&self, req_context: &RequestContext) -> Response<Body>;

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

pub static TEXT_PLAIN_CONTENT_TYPE: &'static str = "text/plain";

pub static TEXT_HTML_CONTENT_TYPE: &'static str = "text/html";

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
  content_type: &'static str) -> Response<Body> {
  Response::builder()
    .status(status_code)
    .header(CONTENT_TYPE, content_type)
    .body(From::from(body))
    .unwrap()
}

pub fn build_response_vec(
  status_code: StatusCode,
  body: Vec<u8>,
  content_type: Cow<'static, str>) -> Response<Body> {
  Response::builder()
    .status(status_code)
    .header(CONTENT_TYPE, HeaderValue::from_str(&content_type).unwrap())
    .body(From::from(body))
    .unwrap()
}

fn log_request_and_response(
  req_context: &RequestContext,
  resp: &Response<Body>) {

  let req = req_context.req();

  let method = req.method().to_string();

  let uri = req.uri().to_string();

  let version = format!("{:?}", req.version());

  let response_status = resp.status().as_u16().to_string();

  let duration = ::utils::duration_in_seconds_f64(&req_context.start_time().elapsed());

  info!("\"{} {} {}\" {} {:.9}s",
        method, uri, version,
        response_status,
        duration);
}

struct InnerThreadedServer {
  route_configuration: RouteConfiguration
}

type ThreadedServerFuture = Box<Future<Item=::hyper::Response<::hyper::Body>, Error=::std::io::Error> + Send>;

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

  fn invoke_handler(
    handler: &RouteConfigurationHandler,
    req_context: &RequestContext) -> Response<Body> {

    let response = handler.handle(&req_context);

    log_request_and_response(&req_context, &response);

    response
  }

}

impl ThreadedServer {

  fn call(&self, req: Request<Body>) -> ThreadedServerFuture {

    let req_context = RequestContext::new(req);

    let route_configuration = &self.inner.route_configuration;

    let handler = route_configuration.path_to_handler()
       .get(req_context.req().uri().path())
       .unwrap_or(route_configuration.not_found_handler());

    if handler.blocking() {

      let handler_clone = Arc::clone(handler);

      Box::new(poll_fn(move || {

        blocking(|| {

          ThreadedServer::invoke_handler(&handler_clone, &req_context)

        })
        .map_err(|e| { 
          warn!("blocking error {}", e);
          ::std::io::Error::new(::std::io::ErrorKind::Other, e)
        })

      }))

    } else {

      Box::new(::futures::future::ok(ThreadedServer::invoke_handler(&handler, &req_context)))

    }

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
