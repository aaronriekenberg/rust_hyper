use futures::{Future, Stream};

use futures_cpupool::CpuPool;

use hyper::header;
use hyper::server::{Request, Response};
use hyper::StatusCode;

use net2::unix::UnixTcpBuilderExt;

use std::borrow::Cow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;

#[derive(Debug)]
pub struct RequestContext {
  req: Request,
  start_time: Instant,
  remote_addr: Option<SocketAddr>
}

impl RequestContext {

  fn new(req: Request, remote_addr: Option<SocketAddr>) -> Self {
    RequestContext { req: req, start_time: Instant::now(), remote_addr: remote_addr }
  }

  pub fn req(&self) -> &Request {
    &self.req
  }

  pub fn start_time(&self) -> &Instant {
    &self.start_time
  }

  pub fn remote_addr(&self) -> Option<SocketAddr> {
    self.remote_addr
  }

}

pub trait RequestHandler : Send + Sync {

  fn use_worker_threadpool(&self) -> bool;

  fn handle(&self, req_context: &RequestContext) -> Response;

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
      path_to_handler: path_to_handler,
      not_found_handler: not_found_handler
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
  req_context: &RequestContext,
  data_last_modified: &SystemTime,
  cache_max_age_seconds: u32) -> Option<Response> {

  if let Some(if_modified_since_header) =
     req_context.req().headers().get::<header::IfModifiedSince>() {
    let if_modified_since = SystemTime::from(if_modified_since_header.0);
    if ::utils::system_time_in_seconds_u64(&data_last_modified) <=
       ::utils::system_time_in_seconds_u64(&if_modified_since) {
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

  let req = req_context.req();

  let remote_addr = match req_context.remote_addr() {
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

  let duration = ::utils::duration_in_seconds_f64(&req_context.start_time().elapsed());

  info!("{} \"{} {} {}\" {} {} {:.9}s", 
        remote_addr,
        method, uri, version,
        response_status, content_length,
        duration);
}

struct InnerThreadedServer {
  worker_pool: CpuPool,
  route_configuration: RouteConfiguration
}

type ThreadedServerFuture = Box<Future<Item = ::hyper::Response, Error = ::hyper::Error>>;

#[derive(Clone)]
struct ThreadedServer {
  inner: Arc<InnerThreadedServer>,
  remote_addr: Option<SocketAddr>
}

impl ThreadedServer {

  fn new(
    worker_threads: usize,
    route_configuration: RouteConfiguration) -> Self {

    let worker_pool = ::futures_cpupool::Builder::new()
      .pool_size(worker_threads)
      .name_prefix("worker-")
      .create();

    let inner = Arc::new(InnerThreadedServer {
      worker_pool: worker_pool,
      route_configuration: route_configuration
    });

    ThreadedServer { inner: inner, remote_addr: None }
  }

  fn clone_with_remote_addr(&self, remote_addr: SocketAddr) -> Self {

    ThreadedServer {
      inner: Arc::clone(&self.inner),
      remote_addr: Some(remote_addr)
    }
  }

  fn invoke_handler(
    handler: &RouteConfigurationHandler,
    req_context: &RequestContext) -> ::hyper::Response {

    let response = handler.handle(&req_context);

    log_request_and_response(&req_context, &response);

    response
  }

}

impl ::hyper::server::Service for ThreadedServer {

  type Request = ::hyper::Request;
  type Response = ::hyper::Response;
  type Error = ::hyper::Error;
  type Future = ThreadedServerFuture;

  fn call(&self, req: Request) -> Self::Future {

    let req_context = RequestContext::new(req, self.remote_addr);

    let route_configuration = &self.inner.route_configuration;

    let handler = route_configuration.path_to_handler()
       .get(req_context.req().path())
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

fn run_handler_thread(
  listen_addr: SocketAddr,
  threaded_server: ThreadedServer) -> Result<(), Box<::std::error::Error + Send + Sync>> {

  let mut core = Core::new()?;

  let handle = core.handle();

  let net2_listener = ::net2::TcpBuilder::new_v4()?
    .reuse_port(true)?
    .bind(listen_addr)?
    .listen(128)?;

  let tcp_listener = TcpListener::from_listener(net2_listener, &listen_addr, &handle)?;

  let http = ::hyper::server::Http::<::hyper::Chunk>::new();

  info!("started handler thread");

  let listener_future = tcp_listener.incoming()
    .for_each(move |(socket, remote_addr)| {
      if let Ok(_) = socket.set_nodelay(true) {
        let connection_future = http.serve_connection(
          socket,
          threaded_server.clone_with_remote_addr(remote_addr))
          .map(|_| ())
          .map_err(move |err| error!("server connection error: ({}) {}", remote_addr, err));
        handle.spawn(connection_future);
      }
      Ok(())
  });

  core.run(listener_future)?;

  error!("core.run returned in handler thread");

  Err(From::from("core.run returned in handler thread"))
}

pub fn run_forever(
  listen_addr: SocketAddr,
  handler_threads: usize,
  worker_threads: usize,
  route_configuration: RouteConfiguration) -> Result<(), Box<::std::error::Error>> {

  let threaded_server = ThreadedServer::new(
    worker_threads,
    route_configuration);

  let mut join_handles = Vec::with_capacity(handler_threads);

  for i in 0..handler_threads {
    let name = format!("handler-{}", i);
    let threaded_server_clone = threaded_server.clone();
    let join_handle = ::std::thread::Builder::new().name(name).spawn(move || {
      run_handler_thread(listen_addr, threaded_server_clone)
    })?;
    join_handles.push(join_handle);
  }

  info!("Listening on http://{} handler_threads={} worker_threads={}",
        listen_addr,
        handler_threads,
        worker_threads);

  for join_handle in join_handles {
    let result = join_handle.join();
    return Err(From::from(format!("join_handle.join returned unexpectedly result = {:?}", result)));
  }

  Err(From::from("run_forever returning"))
}
