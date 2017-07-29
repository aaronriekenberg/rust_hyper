extern crate hyper;
extern crate futures;
extern crate futures_cpupool;
#[macro_use] extern crate log;
extern crate simple_logger;

use log::LogLevel;

use futures::Future;

use futures_cpupool::CpuPool;

use hyper::StatusCode;
use hyper::header::{ContentLength, ContentType};
use hyper::server::{Http, Service, Request, Response};

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

static NOT_FOUND_BODY: &'static str = "Route Not Found";

trait RequestHandler : Send + Sync {
  fn call(&self, req: &Request) -> Response;
}

struct RouteConfiguration {
  routes: HashMap<String, Box<RequestHandler>>
}

fn build_response(
  status_code: StatusCode,
  body: String,
  content_type: ContentType) -> Response
{
  Response::new()
    .with_status(status_code)
    .with_header(ContentLength(body.len() as u64))
    .with_header(content_type)
    .with_body(body)
}

struct ThreadedServer {
  cpu_pool: CpuPool,
  route_configuration: Arc<RouteConfiguration>
}

impl Service for ThreadedServer {

  type Request = Request;
  type Response = Response;
  type Error = hyper::Error;
  type Future = futures::BoxFuture<Response, hyper::Error>;

  fn call(&self, req: Request) -> Self::Future {
    info!("begin call thread {:?}", thread::current().name());

    let route_configuration = Arc::clone(&self.route_configuration);

    let result = self.cpu_pool.spawn_fn(move || {

      info!("do_in_thread thread {:?} req {:?}", thread::current().name(), req);

      let path = req.uri().path();
      info!("path = '{}'", path);

      let mut response_option = None;

      if let Some(request_handler) = route_configuration.routes.get(path) {
        response_option = Some(request_handler.call(&req));
      }

      match response_option {
        Some(response) => Ok(response),
        None => {
          Ok(build_response(
               StatusCode::NotFound,
               NOT_FOUND_BODY.to_string(),
               ContentType::plaintext()))
        }
      }

    }).boxed();

    info!("end call thread {:?}", thread::current().name());

    result
  }

}

struct IndexHandler;

impl RequestHandler for IndexHandler {

  fn call(&self, _: &Request) -> Response {
    let body_string = String::from("<html><body><h1>Index Page</h1></body></html>");
    build_response(
      StatusCode::Ok,
      body_string,
      ContentType::html())
  }

}

fn build_route_configuration() -> Arc<RouteConfiguration> {
  let mut routes : HashMap<String, Box<RequestHandler>> = HashMap::new();

  routes.insert("/".to_string(), Box::new(IndexHandler));

  Arc::new(RouteConfiguration { routes: routes })
}

fn main() {
  simple_logger::init_with_level(LogLevel::Info).expect("init_with_level failed");

  let addr = "0.0.0.0:1337".parse().unwrap();

  let route_configuration = build_route_configuration();

  let cpu_pool = futures_cpupool::Builder::new().name_prefix("server-").create();

  let http_server = Http::new()
    .bind(&addr, move || Ok(
      ThreadedServer { 
        cpu_pool: cpu_pool.clone(),
        route_configuration: Arc::clone(&route_configuration)
      }
    ))
    .expect("bind failed");

  info!("Listening on http://{} with cpu pool", http_server.local_addr().unwrap());

  http_server.run().expect("http_server.run failed");
}
