extern crate chrono;
extern crate crypto;
extern crate hyper;
#[macro_use] extern crate horrorshow;
extern crate fern;
extern crate futures;
extern crate futures_cpupool;
#[macro_use] extern crate log;
extern crate mime;
extern crate net2;
#[macro_use] extern crate serde_derive;
extern crate serde_yaml;
extern crate tokio_core;

mod config;
mod handlers;
mod logging;
mod server;
mod utils;

use std::sync::Arc;

fn install_panic_hook() {

  let original_panic_hook = std::panic::take_hook();

  std::panic::set_hook(Box::new(move |panic_info| {
    original_panic_hook(panic_info);
    std::process::exit(1);
  }));
}

fn log_executable_info(executable_path: String) -> Result<(), std::io::Error> {

  info!("executable_path = '{}'", executable_path);

  let checksum = utils::file_sha256(&executable_path)?;

  info!("sha256 = {}", checksum);

  Ok(())
}

fn build_thread_configuration(config: &config::Configuration) -> server::ThreadConfiguration {

  server::ThreadConfiguration::new(
    config.worker_threads()
  )

}

fn build_route_configuration(config: &config::Configuration) -> Result<server::RouteConfiguration, Box<std::error::Error>> {

  let mut path_to_handler = server::RouteConfigurationHandlerMap::new();

  let index_handler = handlers::index::IndexHandler::new(config)?;
  path_to_handler.insert("/".to_string(), Arc::new(index_handler));

  for command_info in config.commands() {
    let handler =
      handlers::command::CommandHandler::new(command_info.clone());
    path_to_handler.insert(command_info.http_path().clone(), Arc::new(handler));
  }

  for static_path_info in config.static_paths() {
    let mime_type = static_path_info.content_type().parse()?;
    let handler =
      handlers::static_file::StaticFileHandler::new(
        static_path_info.fs_path().clone(),
        mime_type,
        static_path_info.cache_max_age_seconds());
    path_to_handler.insert(static_path_info.http_path().clone(), Arc::new(handler));
  }

  let not_found_handler = handlers::not_found::NotFoundHandler;

  Ok(server::RouteConfiguration::new(
       path_to_handler,
       Arc::new(not_found_handler)))
}

fn main() {

  install_panic_hook();

  logging::initialize_logging().expect("failed to initialize logging");

  let executable_path = std::env::args().nth(0).expect("missing executable command line argument");

  log_executable_info(executable_path).expect("failed to log executable info");

  let config_file = std::env::args().nth(1).expect("config file required as command line argument");

  let config = config::read_config(config_file).expect("error reading configuration file");
  info!("config = {:#?}", config);

  let listen_addr = config.listen_address().parse().expect("invalid listen_address");

  let thread_configuration = build_thread_configuration(&config);

  let route_configuration = build_route_configuration(&config).expect("failed to build route_configuration");

  server::run_forever(
    listen_addr,
    thread_configuration,
    route_configuration).expect("server::run_forever failed");
}
