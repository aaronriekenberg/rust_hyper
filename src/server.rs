use futures_cpupool;
use futures_cpupool::{CpuFuture, CpuPool};

use hyper;
use hyper::header;
use hyper::server::{Request, Response};
use hyper::StatusCode;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use utils;

#[derive(Debug)]
pub struct RequestContext {
  req: Request,
  start_time: Instant
}

impl RequestContext {

  pub fn new(req: Request) -> Self {
    RequestContext { req: req, start_time: Instant::now() }
  }

  pub fn req(&self) -> &Request {
    &self.req
  }

  pub fn start_time(&self) -> &Instant {
    &self.start_time
  }

}

pub trait RequestHandler : Send + Sync {
  fn handle(&self, req_context: &RequestContext) -> Response;
}

pub struct RouteConfiguration {
  path_to_handler: HashMap<String, Box<RequestHandler>>,
  not_found_handler: Box<RequestHandler>
}

impl RouteConfiguration {

  pub fn new(
    path_to_handler: HashMap<String, Box<RequestHandler>>,
    not_found_handler: Box<RequestHandler>) -> Self {
    RouteConfiguration {
      path_to_handler: path_to_handler,
      not_found_handler: not_found_handler
    }
  }

  pub fn path_to_handler(&self) -> &HashMap<String, Box<RequestHandler>> {
    &self.path_to_handler
  }

  pub fn not_found_handler(&self) -> &Box<RequestHandler> {
    &self.not_found_handler
  }

}

pub fn build_response_status(
  status_code: StatusCode) -> Response {
  Response::new()
    .with_status(status_code)
}

pub fn build_response_string(
  status_code: StatusCode,
  body: Cow<'static, str>,
  content_type: header::ContentType) -> Response
{
  build_response_status(status_code)
    .with_header(content_type)
    .with_header(header::ContentLength(body.len() as u64))
    .with_body(body)
}

pub fn build_response_vec(
  status_code: StatusCode,
  body: Vec<u8>,
  content_type: header::ContentType) -> Response
{
  build_response_status(status_code)
    .with_header(content_type)
    .with_header(header::ContentLength(body.len() as u64))
    .with_body(body)
}

pub fn handle_not_modified(
  req: &Request,
  data_last_modified: &SystemTime,
  cache_max_age_seconds: u32) -> Option<Response> {

  if let Some(if_modified_since_header) =
     req.headers().get::<header::IfModifiedSince>() {
    let if_modified_since = SystemTime::from(if_modified_since_header.0);
    if utils::system_time_in_seconds_u64(&data_last_modified) <=
       utils::system_time_in_seconds_u64(&if_modified_since) {
      return Some(
        build_response_status(StatusCode::NotModified)
          .with_header(header::LastModified((*data_last_modified).into()))
          .with_header(header::CacheControl(
                         vec![header::CacheDirective::Public,
                              header::CacheDirective::MaxAge(cache_max_age_seconds)])));
    }
  }

  None
}

pub fn log_request_and_response(
  req_context: &RequestContext,
  resp: &Response) {

  let req = &req_context.req();

  let remote_addr = match req.remote_addr() {
    Some(remote_addr) => Cow::from(remote_addr.to_string()),
    None => Cow::from("")
  };

  let method = req.method().to_string();

  let uri = req.uri().to_string();

  let version = req.version().to_string();

  let response_status = resp.status().as_u16().to_string();

  let content_length = match resp.headers().get::<header::ContentLength>() {
    Some(content_length_header) => Cow::from(content_length_header.0.to_string()),
    None => Cow::from("0")
  };

  let duration = utils::duration_in_seconds_f64(&req_context.start_time().elapsed());

  info!("{} \"{} {} {}\" {} {} {:.9}s", 
        remote_addr,
        method, uri, version,
        response_status, content_length,
        duration);
}

struct InnerThreadedServer {
  cpu_pool: CpuPool,
  max_pending_tasks: usize,
  pending_tasks: AtomicUsize,
  route_configuration: RouteConfiguration
}

#[derive(Clone)]
pub struct ThreadedServer {
  inner: Arc<InnerThreadedServer>
}

impl ThreadedServer {

  pub fn new(
    pool_threads: usize,
    max_pending_tasks: usize,
    route_configuration: RouteConfiguration) -> Self {

    let cpu_pool = futures_cpupool::Builder::new()
      .pool_size(pool_threads)
      .name_prefix("server-")
      .create();

    let inner = Arc::new(InnerThreadedServer {
      cpu_pool: cpu_pool,
      max_pending_tasks: max_pending_tasks,
      pending_tasks: AtomicUsize::new(0),
      route_configuration: route_configuration
    });

    ThreadedServer { inner: inner }
  }

  fn wait_for_pending_tasks(&self) {
    let inner = &self.inner;
    loop {
      let pending_tasks = inner.pending_tasks.load(Ordering::SeqCst);
      if pending_tasks < inner.max_pending_tasks {
        break;
      } else {
        warn!("pending tasks is big: {}", pending_tasks);
        thread::sleep(Duration::from_millis(100));
      }
    }
  }

}

impl hyper::server::Service for ThreadedServer {

  type Request = Request;
  type Response = Response;
  type Error = hyper::Error;
  type Future = CpuFuture<Response, hyper::Error>;

  fn call(&self, req: Request) -> Self::Future {
    let req_context = RequestContext::new(req);

    self.wait_for_pending_tasks();

    let inner = Arc::clone(&self.inner);

    self.inner.pending_tasks.fetch_add(1, Ordering::SeqCst);

    self.inner.cpu_pool.spawn_fn(move || {

      debug!("do_in_thread thread {:?} req_context {:?}", thread::current().name(), req_context);

      let path = req_context.req().path();

      let handler = inner.route_configuration.path_to_handler()
        .get(path)
        .unwrap_or(inner.route_configuration.not_found_handler());

      let response = handler.handle(&req_context);

      log_request_and_response(&req_context, &response);

      inner.pending_tasks.fetch_sub(1, Ordering::SeqCst);

      Ok(response)

    })
  }

}
