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

fn serve(thread_id: usize, listen_addr: &SocketAddr, protocol: &Http) {

    info!("starting serve thread_id {}", thread_id);

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

    info!("exiting serve thread_id {}", thread_id);

}

fn start_server(num_instances: usize, addr: &str) {

    let listen_addr: SocketAddr = addr.parse().unwrap();

    let protocol = Arc::new(Http::new());

    for thread_id in 0..(num_instances-1) {
        let protocol = Arc::clone(&protocol);
        thread::spawn(move || serve(thread_id, &listen_addr, &protocol));
    }

    serve(num_instances-1, &listen_addr, &protocol);

}

fn main() {
    simple_logger::init_with_level(LogLevel::Info).expect("init_with_level failed");
    start_server(8, "0.0.0.0:1337");
}
