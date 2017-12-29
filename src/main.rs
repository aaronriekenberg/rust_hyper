mod server;
mod utils;

extern crate chrono;
extern crate crypto;
extern crate hyper;
#[macro_use] extern crate horrorshow;
extern crate fern;
extern crate futures_cpupool;
#[macro_use] extern crate log;
extern crate mime;
#[macro_use] extern crate serde_derive;
extern crate serde_yaml;

use chrono::prelude::Local;

use crypto::digest::Digest;
use crypto::sha2::Sha256;

use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::header;
use hyper::server::{Http, Response};
use hyper::StatusCode;

use mime::Mime;

use std::borrow::Cow;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::process::Command;
use std::thread;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandInfo {
  http_path: String,
  description: String,
  command: String,
  args: Vec<String>
}

#[derive(Debug, Serialize, Deserialize)]
struct StaticPathInfo {
  http_path: String,
  fs_path: String,
  content_type: String,
  cache_max_age_seconds: u32,
  include_in_main_page: bool
}

#[derive(Debug, Serialize, Deserialize)]
struct MainPageInfo {
  title: String,
  cache_max_age_seconds: u32
}

#[derive(Debug, Serialize, Deserialize)]
struct Configuration {
  listen_address: String,
  threads: usize,
  max_pending_tasks: usize,
  main_page_info: MainPageInfo,
  commands: Vec<CommandInfo>,
  static_paths: Vec<StaticPathInfo>
}

struct NotFoundHandler;

impl server::RequestHandler for NotFoundHandler {

  fn handle(&self, _: &server::RequestContext) -> Response {
    server::build_response_string(
      StatusCode::NotFound,
      Cow::from("Route not found"),
      header::ContentType::plaintext())
      .with_header(header::CacheControl(
                     vec![header::CacheDirective::MaxAge(0)]))
  }

}

struct IndexHandler {
  index_string: String,
  creation_time: SystemTime,
  cache_max_age_seconds: u32
}

impl IndexHandler {

  pub fn new(config: &Configuration) -> Result<Self, Box<Error>> {

    let static_paths_to_include: Vec<_> = 
      config.static_paths.iter().filter(|s| s.include_in_main_page).collect();

    let now = SystemTime::now();

    let mut last_modified_string = String::new();
    last_modified_string.push_str("Last Modified: ");
    last_modified_string.push_str(&utils::local_time_to_string(utils::system_time_to_local(&now)));

    let s = html! {
      : doctype::HTML;
      html {
        head {
          title: &config.main_page_info.title;
          meta(name = "viewport", content = "width=device, initial-scale=1");
          link(rel = "stylesheet", type = "text/css", href = "style.css");
        }
        body {
          h2 {
            : &config.main_page_info.title;
          }
          @ if config.commands.len() > 0 {
            h3 {
              : "Comamnds:"
            }
            ul {
              @ for command_info in &config.commands {
                li {
                  a(href = &command_info.http_path) {
                    : &command_info.description
                  }
                }
              }
            }
          }
          @ if static_paths_to_include.len() > 0 {
            h3 {
              : "Static Paths:"
            }
            ul {
              @ for static_path in &static_paths_to_include {
                li {
                  a(href = &static_path.http_path) {
                    : &static_path.fs_path
                  }
                }
              }
            }
          }
          hr;
          small {
            : &last_modified_string
          }
        }
      }
    }.into_string()?;

    Ok(IndexHandler { 
      index_string: s,
      creation_time: now,
      cache_max_age_seconds: config.main_page_info.cache_max_age_seconds
    })
  }

}

impl server::RequestHandler for IndexHandler {

  fn handle(&self, req_context: &server::RequestContext) -> Response {
    if let Some(response) = server::handle_not_modified(
      req_context.req(),
      &self.creation_time,
      self.cache_max_age_seconds) {
      return response;
    }

    server::build_response_string(
      StatusCode::Ok,
      Cow::from(self.index_string.clone()),
      header::ContentType::html())
      .with_header(header::LastModified(self.creation_time.into()))
      .with_header(header::CacheControl(
         vec![header::CacheDirective::Public,
              header::CacheDirective::MaxAge(self.cache_max_age_seconds)]))
  }

}

struct CommandHandler {
  command_info: CommandInfo,
  command_line_string: String
}

impl CommandHandler {

  pub fn new(command_info: CommandInfo) -> Self {

    let mut command_line_string = String::new();

    command_line_string.push_str("$ ");
    command_line_string.push_str(&command_info.command);

    for arg in &command_info.args {
      command_line_string.push(' ');
      command_line_string.push_str(arg);
    }

    CommandHandler { command_info: command_info, command_line_string: command_line_string }
  }

  fn run_command(&self) -> String {

    let mut command = Command::new(&self.command_info.command);

    command.args(&self.command_info.args);

    let command_output =
      match command.output() {
        Ok(output) => {
          let mut combined_output =
            String::with_capacity(output.stderr.len() + output.stdout.len());
          combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
          combined_output.push_str(&String::from_utf8_lossy(&output.stdout));
          combined_output
        },
        Err(err) => format!("command error: {}", err),
      };

    command_output
  }

  fn build_pre_string(&self, command_output: String) -> String {

    let mut pre_string = String::with_capacity(command_output.len() + 100);

    pre_string.push_str("Now: ");
    pre_string.push_str(&utils::local_time_to_string(Local::now()));
    pre_string.push_str("\n\n");
    pre_string.push_str(&self.command_line_string);
    pre_string.push_str("\n\n");
    pre_string.push_str(&command_output);

    pre_string
  }

  fn build_html_string(&self, pre_string: String) -> String {

    let html_string = html! {
      : doctype::HTML;
      html {
        head {
          title: &self.command_info.description;
          meta(name = "viewport", content = "width=device, initial-scale=1");
          link(rel = "stylesheet", type = "text/css", href = "style.css");
        }
        body {
          a(href = "..") {
            : ".."
          }
          pre {
            : pre_string
          }
        }
      }
    }.into_string()
     .unwrap_or_else(|err| format!("error executing template: {}", err));

    html_string
  }

}

impl server::RequestHandler for CommandHandler {

  fn handle(&self, _: &server::RequestContext) -> Response {
    let command_output = self.run_command();

    let pre_string = self.build_pre_string(command_output);

    let html_string = self.build_html_string(pre_string);

    server::build_response_string(
      StatusCode::Ok,
      Cow::from(html_string),
      header::ContentType::html())
      .with_header(header::CacheControl(
         vec![header::CacheDirective::MaxAge(0)]))
  }

}

struct StaticFileHandler {
  file_path: String,
  mime_type: Mime,
  cache_max_age_seconds: u32
}

impl StaticFileHandler {

  pub fn new(file_path: String, mime_type: Mime, cache_max_age_seconds: u32) -> Self {
    StaticFileHandler { 
      file_path: file_path,
      mime_type: mime_type,
      cache_max_age_seconds: cache_max_age_seconds
    }
  }

  fn read_file(&self) -> Result<Vec<u8>, io::Error> {
    let mut file = File::open(&self.file_path)?;

    let mut file_contents = Vec::new();

    file.read_to_end(&mut file_contents)?;

    Ok(file_contents)
  }

}

impl server::RequestHandler for StaticFileHandler {

  fn handle(&self, req_context: &server::RequestContext) -> Response {
    debug!("StaticFileHandler.handle req_context = {:?}", req_context);

    let file_metadata =
      match fs::metadata(&self.file_path) {
        Ok(metadata) => metadata,
        Err(_) => return server::build_response_status(StatusCode::NotFound)
      };

    let file_modified =
      match file_metadata.modified() {
        Ok(file_modified) => file_modified,
        Err(_) => return server::build_response_status(StatusCode::NotFound)
      };

    if let Some(response) = server::handle_not_modified(
      req_context.req(),
      &file_modified,
      self.cache_max_age_seconds) {
      return response;
    }

    match self.read_file() {
      Ok(file_contents) => {
        server::build_response_vec(
          StatusCode::Ok,
          file_contents,
          header::ContentType(self.mime_type.clone()))
          .with_header(header::LastModified(file_modified.into()))
          .with_header(header::CacheControl(
             vec![header::CacheDirective::Public,
                  header::CacheDirective::MaxAge(self.cache_max_age_seconds)]))
      },
      Err(_) => {
        server::build_response_status(StatusCode::NotFound)
      }
    }
  }

}

fn initialize_logging() -> Result<(), fern::InitError>{
  fern::Dispatch::new()
    .level(log::LevelFilter::Info)
    .format(|out, message, record| {
      out.finish(
        format_args!("{} [{}] {} {} - {}",
          Local::now().format("%Y-%m-%d %H:%M:%S%.3f %z"),
          thread::current().name().unwrap_or("UNKNOWN"),
          record.level(),
          record.target(),
          message
        )
      )
    })
    .chain(io::stdout())
    .apply()?;

  Ok(())
}

fn file_sha256(path: String) -> Result<String, io::Error> {
  let mut file = File::open(path)?;

  let mut hasher = Sha256::new();

  let mut buffer = vec![0; 1024 * 1024];

  loop {
    let bytes_read = file.read(&mut buffer[..])?;
    match bytes_read {
      0 => break,
      _ => hasher.input(&buffer[0..bytes_read])
    }
  }

  Ok(hasher.result_str())
}

fn read_config(config_file: String) -> Result<Configuration, Box<Error>> {
  info!("reading {}", config_file);

  let mut file = File::open(config_file)?;

  let mut file_contents = String::new();

  file.read_to_string(&mut file_contents)?;

  let configuration: Configuration = serde_yaml::from_str(&file_contents)?;

  Ok(configuration)
}

fn build_route_configuration(config: &Configuration) -> server::RouteConfiguration {
  let mut path_to_handler = server::RouteConfigurationHandlerMap::new();

  let index_handler = IndexHandler::new(config).expect("error creating IndexHandler");
  path_to_handler.insert("/".to_string(), Box::new(index_handler));

  for command_info in &config.commands {
    let handler = CommandHandler::new(command_info.clone());
    path_to_handler.insert(command_info.http_path.clone(), Box::new(handler));
  }

  for static_path_info in &config.static_paths {
    let mime_type = static_path_info.content_type.parse().expect("invalid mime type");
    let handler = StaticFileHandler::new(
      static_path_info.fs_path.clone(),
      mime_type,
      static_path_info.cache_max_age_seconds);
    path_to_handler.insert(static_path_info.http_path.clone(), Box::new(handler));
  }

  let not_found_handler = Box::new(NotFoundHandler);

  server::RouteConfiguration::new(
    path_to_handler,
    not_found_handler)
}

fn create_threaded_server(config: &Configuration) -> server::ThreadedServer {

  let route_configuration = build_route_configuration(&config);

  server::ThreadedServer::new(
    config.threads,
    config.max_pending_tasks,
    route_configuration)
}

fn main() {
  initialize_logging().expect("failed to initialize logging");

  let executable_path = env::args().nth(0).expect("missing argument 0");
  info!("executable_path = {}", executable_path);

  let executable_checksum = file_sha256(executable_path).expect("error getting executable sha256");
  info!("sha256 = {}", executable_checksum);

  let config_file = env::args().nth(1).expect("config file required as command line argument");

  let config = read_config(config_file).expect("error reading configuration file");
  info!("config = {:#?}", config);

  let listen_addr = config.listen_address.parse().expect("invalid listen_address");

  let threaded_server = create_threaded_server(&config);

  let http_server = Http::new()
    .bind(&listen_addr, move || Ok(threaded_server.clone()))
    .expect("bind failed");

  info!("Listening on http://{} with cpu pool size {}",
        http_server.local_addr().unwrap(),
        config.threads);

  http_server.run().expect("http_server.run failed");
}
