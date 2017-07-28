extern crate hyper;
extern crate futures;
extern crate futures_cpupool;
#[macro_use] extern crate log;
extern crate simple_logger;

use log::LogLevel;

use futures::Future;

use futures_cpupool::CpuPool;

use hyper::header::{ContentLength, ContentType};
use hyper::server::{Http, Service, Request, Response};

use std::thread;

static PHRASE: &'static [u8] = b"Hello World!";

struct Server {
  cpu_pool: CpuPool
}

impl Service for Server {

  type Request = Request;
  type Response = Response;
  type Error = hyper::Error;
  type Future = futures::BoxFuture<Response, hyper::Error>;

  fn call(&self, req: Request) -> Self::Future {
    info!("begin call thread {:?}", thread::current().name());

    let result = self.cpu_pool.spawn_fn(move || {

      info!("do_in_thread thread {:?} req {:?}", thread::current().name(), req);
      info!("req.uri.path = {}", req.uri().path());

      Ok(Response::new()
        .with_header(ContentLength(PHRASE.len() as u64))
        .with_header(ContentType::plaintext())
        .with_body(PHRASE))

    }).boxed();

    info!("end call thread {:?}", thread::current().name());

    result
  }

}

fn main() {
  simple_logger::init_with_level(LogLevel::Info).expect("init_with_level failed");

  let addr = "0.0.0.0:1337".parse().unwrap();

  let cpu_pool = futures_cpupool::Builder::new().name_prefix("server-").create();

  let http_server = Http::new()
    .bind(&addr, move || Ok(Server { cpu_pool: cpu_pool.clone() } ))
    .expect("bind failed");

  info!("Listening on http://{} with cpu pool", http_server.local_addr().unwrap());

  http_server.run().expect("http_server.run failed");
}
