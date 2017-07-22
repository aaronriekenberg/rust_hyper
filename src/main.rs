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

use std::sync::Arc;
use std::thread;

static PHRASE: &'static [u8] = b"Hello World!";

fn do_in_thread(req: &Request) -> Result<Response, hyper::Error> {
  info!("do_in_thread req {:?} thread {:?}", req, thread::current().name());
  Ok(Response::new()
    .with_header(ContentLength(PHRASE.len() as u64))
    .with_header(ContentType::plaintext())
    .with_body(PHRASE))
}

struct Server {
    cpu_pool: Arc<CpuPool>
}

impl Service for Server {

    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = futures::BoxFuture<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
      self.cpu_pool.spawn_fn(move || do_in_thread(&req)).boxed()
    }

}

fn main() {
    simple_logger::init_with_level(LogLevel::Info).expect("init_with_level failed");

    let addr = "127.0.0.1:1337".parse().unwrap();

    let cpu_pool = Arc::new(futures_cpupool::Builder::new().pool_size(4).name_prefix("server-").create());

    let http_server = Http::new().bind(&addr, move || Ok(Server { cpu_pool: Arc::clone(&cpu_pool) } )).unwrap();

    info!("Listening on http://{} with 1 thread.", http_server.local_addr().unwrap());

    http_server.run().unwrap();
}
