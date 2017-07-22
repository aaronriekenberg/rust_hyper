#[macro_use] extern crate log;
extern crate futures;
extern crate hyper;
extern crate net2;
extern crate simple_logger;
extern crate tokio_core;

use futures::future::FutureResult;
use futures::Stream;

use log::LogLevel;

use hyper::{Get, StatusCode};
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

use net2::unix::UnixTcpBuilderExt;

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;

use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;

struct Echo;

impl Service for Echo {

  type Request = Request;
  type Response = Response;
  type Error = hyper::Error;
  type Future = FutureResult<Response, hyper::Error>;

  fn call(&self, req: Request) -> Self::Future {

    futures::future::ok(match req.method() {

      &Get => {
        let mut response = String::new();
        response.push_str("Got ");
        response.push_str(&req.path());
        Response::new()
          .with_header(ContentLength(response.len() as u64))
          .with_body(response)
      },

      _ => {
        Response::new().with_status(StatusCode::NotFound)
      }

    })
  }

}

fn serve(listen_addr: &SocketAddr, protocol: &Http) {

  info!("starting {:?}", thread::current().name());

  let mut core = Core::new().expect("error creating core");
  let handle = core.handle();

  let listener = net2::TcpBuilder::new_v4().expect("error creating listener builder")
    .reuse_port(true).expect("error setting reuse_port")
    .bind(listen_addr).expect("error calling bind")
    .listen(128).expect("error calling listen");

  let listener = TcpListener::from_listener(listener, &listen_addr, &handle)
    .expect("error calling from_listener");

  let server =
    listener.incoming().for_each(|(socket, addr)| {
      protocol.bind_connection(&handle, socket, addr, Echo);
      Ok(())
    });

  core.run(server).expect("error calling core.run");

  info!("exiting {:?}", thread::current().name());

}

fn start_server(num_threads: usize, addr: &str) {

  info!("start_server num_threads = {} addr = {}", num_threads, addr); 

  let listen_addr: SocketAddr = addr.parse().unwrap();

  let protocol = Arc::new(Http::new());

  let mut thread_handles = Vec::with_capacity(num_threads);

  for i in 0..num_threads {
    let protocol = Arc::clone(&protocol);
    thread_handles.push(
      thread::Builder::new()
        .name(format!("serve-{}", i))
        .spawn(move || serve(&listen_addr, &protocol))
        .expect("spawn error"));
  }

  while let Some(handle) = thread_handles.pop() {
    handle.join().expect("join failed");
  }
}

fn main() {
  simple_logger::init_with_level(LogLevel::Info).expect("init_with_level failed");
  start_server(8, "0.0.0.0:1337");
}
