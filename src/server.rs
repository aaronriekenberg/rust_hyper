use futures::Future;

use futures_cpupool::CpuPool;

use hyper::{Body, Response, Request, Server, StatusCode};
use hyper::header::HeaderValue;
use hyper::service::service_fn;

use std::borrow::Cow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

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

  fn use_worker_threadpool(&self) -> bool { false }

  fn handle(&self, req_context: &RequestContext) -> Response<Body>;

}

#[derive(Debug)]
pub struct ThreadConfiguration {
  worker_threads: usize
}

impl ThreadConfiguration {

  pub fn new(
    worker_threads: usize) -> Self {

    ThreadConfiguration {
      worker_threads
    }
  }

  pub fn worker_threads(&self) -> usize {
    self.worker_threads
  }

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
    .header("Content-Type", content_type)
    .body(From::from(body))
    .unwrap()
}

pub fn build_response_vec(
  status_code: StatusCode,
  body: Vec<u8>,
  content_type: Cow<'static, str>) -> Response<Body> {
  Response::builder()
    .status(status_code)
    .header("Content-Type", HeaderValue::from_str(&content_type).unwrap())
    .body(From::from(body))
    .unwrap()
}

pub fn handle_not_modified(
  req_context: &RequestContext,
  data_last_modified: &SystemTime,
  cache_max_age_seconds: u32) -> Option<Response<Body>> {

/*
  if let Some(ref if_modified_since_header) =
     req_context.req().headers().get::<header::IfModifiedSince>() {
    let if_modified_since = SystemTime::from(if_modified_since_header.0);
    if ::utils::system_time_in_seconds_u64(&data_last_modified) <=
       ::utils::system_time_in_seconds_u64(&if_modified_since) {
      return Some(
        build_response_status(StatusCode::NotModified)
          .with_header(header::LastModified(From::from(*data_last_modified)))
          .with_header(header::CacheControl(
                         vec![header::CacheDirective::Public,
                              header::CacheDirective::MaxAge(cache_max_age_seconds)])));
    }
  }
*/

  None
}

fn log_request_and_response(
  req_context: &RequestContext,
  resp: &Response<Body>) {

  let req = req_context.req();

  let method = req.method().to_string();

  let uri = req.uri().to_string();

  let version = format!("{:?}", req.version());

  let response_status = resp.status().as_u16().to_string();

  let content_length = match resp.headers().get("Content-Length") {
    Some(ref content_length_header) => Cow::from(format!("{:?}", content_length_header)),
    None => Cow::from("0")
  };

  let duration = ::utils::duration_in_seconds_f64(&req_context.start_time().elapsed());

  info!("\"{} {} {}\" {} {} {:.9}s",
        method, uri, version,
        response_status, content_length,
        duration);
}

struct InnerThreadedServer {
  worker_pool: CpuPool,
  route_configuration: RouteConfiguration
}

type ThreadedServerFuture = Box<Future<Item=::hyper::Response<::hyper::Body>, Error=::hyper::Error> + Send>;

#[derive(Clone)]
struct ThreadedServer {
  inner: Arc<InnerThreadedServer>
}

impl ThreadedServer {

  fn new(
    worker_threads: usize,
    route_configuration: RouteConfiguration) -> Self {

    let worker_pool = ::futures_cpupool::Builder::new()
      .pool_size(worker_threads)
      .name_prefix("worker-")
      .create();

    ThreadedServer {
      inner: Arc::new(
        InnerThreadedServer {
          worker_pool,
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

    if handler.use_worker_threadpool() {

      let handler_clone = Arc::clone(handler);

      Box::new(self.inner.worker_pool.spawn_fn(move || {

        Ok(ThreadedServer::invoke_handler(&handler_clone, &req_context))

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
  thread_configuration: ThreadConfiguration,
  route_configuration: RouteConfiguration) -> Result<(), Box<::std::error::Error>> {

  let threaded_server = ThreadedServer::new(
    thread_configuration.worker_threads(),
    route_configuration);

  info!("thread_configuration = {:#?}", thread_configuration);

  run_server(listen_addr, threaded_server)
}
