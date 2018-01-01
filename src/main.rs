extern crate chrono;
extern crate crypto;
extern crate hyper;
#[macro_use] extern crate horrorshow;
extern crate fern;
extern crate futures;
extern crate futures_cpupool;
#[macro_use] extern crate log;
extern crate mime;
#[macro_use] extern crate serde_derive;
extern crate serde_yaml;

mod config;
mod handlers;
mod server;
mod utils;

use chrono::prelude::Local;

use std::sync::Arc;

fn initialize_logging() -> Result<(), fern::InitError>{
  fern::Dispatch::new()
    .level(log::LevelFilter::Info)
    .format(|out, message, record| {
      out.finish(
        format_args!("{} [{}] {} {} - {}",
          Local::now().format("%Y-%m-%d %H:%M:%S%.3f %z"),
          std::thread::current().name().unwrap_or("UNKNOWN"),
          record.level(),
          record.target(),
          message
        )
      )
    })
    .chain(std::io::stdout())
    .apply()?;

  Ok(())
}

fn log_executable_info() {
  let executable_path = std::env::args().nth(0).expect("missing argument 0");
  info!("executable_path = {}", executable_path);

  let executable_checksum = utils::file_sha256(executable_path).expect("error getting executable sha256");
  info!("sha256 = {}", executable_checksum);
}

fn build_route_configuration(config: &config::Configuration) -> server::RouteConfiguration {
  let mut path_to_handler = server::RouteConfigurationHandlerMap::new();

  let index_handler =
    handlers::index::IndexHandler::new(config).expect("error creating IndexHandler");
  path_to_handler.insert("/".to_string(), Arc::new(Box::new(index_handler)));

  for command_info in config.commands() {
    let handler =
      handlers::command::CommandHandler::new(command_info.clone());
    path_to_handler.insert(command_info.http_path().clone(), Arc::new(Box::new(handler)));
  }

  for static_path_info in config.static_paths() {
    let mime_type = static_path_info.content_type().parse().expect("invalid mime type");
    let handler =
      handlers::static_file::StaticFileHandler::new(
        static_path_info.fs_path().clone(),
        mime_type,
        static_path_info.cache_max_age_seconds());
    path_to_handler.insert(static_path_info.http_path().clone(), Arc::new(Box::new(handler)));
  }

  let not_found_handler = handlers::not_found::NotFoundHandler;

  server::RouteConfiguration::new(
    path_to_handler,
    Arc::new(Box::new(not_found_handler)))
}

fn create_threaded_server(config: &config::Configuration) -> server::ThreadedServer {

  let route_configuration = build_route_configuration(&config);

  server::ThreadedServer::new(
    config.threads(),
    route_configuration)
}

fn main() {
  initialize_logging().expect("failed to initialize logging");

  log_executable_info();

  let config_file = std::env::args().nth(1).expect("config file required as command line argument");

  let config = config::read_config(config_file).expect("error reading configuration file");
  info!("config = {:#?}", config);

  let listen_addr = config.listen_address().parse().expect("invalid listen_address");

  let threaded_server = create_threaded_server(&config);

  let http_server = hyper::server::Http::new()
    .bind(&listen_addr, move || Ok(threaded_server.clone()))
    .expect("bind failed");

  info!("Listening on http://{} with cpu pool size {}",
        http_server.local_addr().unwrap(),
        config.threads());

  http_server.run().expect("http_server.run failed");
}
