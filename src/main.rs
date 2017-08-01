extern crate chrono;
extern crate hyper;
#[macro_use] extern crate horrorshow;
extern crate fern;
extern crate futures;
extern crate futures_cpupool;
#[macro_use] extern crate log;
extern crate mime;
#[macro_use] extern crate serde_derive;
extern crate serde_yaml;

use chrono::prelude::Local;

use futures::Future;

use futures_cpupool::CpuPool;

use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::StatusCode;
use hyper::header::{CacheControl, CacheDirective, ContentLength, ContentType, IfModifiedSince, LastModified, Referer, UserAgent};
use hyper::server::{Http, Service, Request, Response};

use mime::Mime;

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;

static NOT_FOUND_BODY: &'static str = "Route Not Found";

trait RequestHandler : Send + Sync {
  fn handle(&self, req: &Request) -> Response;
}

struct RouteConfiguration {
  routes: HashMap<String, Box<RequestHandler>>
}

fn build_response_status(
  status_code: StatusCode) -> Response {
  Response::new()
    .with_status(status_code)
}

fn build_response_string(
  status_code: StatusCode,
  body: String,
  content_type: ContentType) -> Response
{
  build_response_status(status_code)
    .with_header(content_type)
    .with_header(ContentLength(body.len() as u64))
    .with_body(body)
}

fn build_response_vec(
  status_code: StatusCode,
  body: Vec<u8>,
  content_type: ContentType) -> Response
{
  build_response_status(status_code)
    .with_header(content_type)
    .with_header(ContentLength(body.len() as u64))
    .with_body(body)
}

fn systemtime_in_seconds(st: &std::time::SystemTime) -> i64 {
  match st.duration_since(UNIX_EPOCH) {
    Ok(dur) => {
      dur.as_secs() as i64
    },
    Err(err) => {
      let neg = err.duration();
      -(neg.as_secs() as i64)
    }
  }
}

fn log_request_and_response(
  req: &Request,
  resp: &Response) {

  let mut log_string = String::with_capacity(300);

  match req.remote_addr() {
    Some(remote_addr) => {
      log_string.push_str(&remote_addr.to_string());
      log_string.push(' ');
    },
    None => {}
  }

  log_string.push('"');
  log_string.push_str(&req.method().to_string());
  log_string.push(' ');
  log_string.push_str(&req.uri().to_string());
  log_string.push(' ');
  log_string.push_str(&req.version().to_string());
  log_string.push('"');

  log_string.push(' ');
  log_string.push_str(&resp.status().as_u16().to_string());

  log_string.push(' ');
  match resp.headers().get::<ContentLength>() {
    Some(content_length_header) => log_string.push_str(&content_length_header.0.to_string()),
    None => log_string.push('0')
  }

  log_string.push(' ');
  log_string.push('"');
  match req.headers().get::<Referer>() {
    Some(referrer_header) => log_string.push_str(&referrer_header),
    None => {}
  }
  log_string.push('"');

  log_string.push(' ');
  log_string.push('"');
  match req.headers().get::<UserAgent>() {
    Some(user_agent_header) => log_string.push_str(&user_agent_header),
    None => {}
  }
  log_string.push('"');

  info!("{}", log_string);
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
    debug!("begin call thread {:?}", thread::current().name());

    let route_configuration = Arc::clone(&self.route_configuration);

    let result = self.cpu_pool.spawn_fn(move || {

      debug!("do_in_thread thread {:?} req {:?}", thread::current().name(), req);

      let path = req.uri().path();
      debug!("path = '{}'", path);

      let response = match route_configuration.routes.get(path) {
        Some(request_handler) => request_handler.handle(&req),
        None =>
          build_response_string(
            StatusCode::NotFound,
            NOT_FOUND_BODY.to_string(),
            ContentType::plaintext())
      };

      log_request_and_response(&req, &response);

      Ok(response)

    }).boxed();

    debug!("end call thread {:?}", thread::current().name());

    result
  }

}

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
struct Configuration {
  listen_address: String,
  main_page_title: String,
  threads: usize,
  commands: Vec<CommandInfo>,
  static_paths: Vec<StaticPathInfo>
}

struct IndexHandler {
  index_string: String
}

impl IndexHandler {

  pub fn new(config: &Configuration) -> Result<Self, Box<Error>> {

    let static_paths_to_include: Vec<_> = 
      config.static_paths.iter().filter(|s| s.include_in_main_page).collect();

    let s = html! {
      : doctype::HTML;
      html {
        head {
          title: &config.main_page_title;
          meta(name = "viewport", content = "width=device, initial-scale=1");
          link(rel = "stylesheet", type = "text/css", href = "style.css");
        }
        body {
          h2 {
            : &config.main_page_title;
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
        }
      }
    }.into_string()?;

    Ok(IndexHandler { index_string: s })
  }

}

impl RequestHandler for IndexHandler {

  fn handle(&self, _: &Request) -> Response {
    build_response_string(
      StatusCode::Ok,
      self.index_string.clone(),
      ContentType::html())
  }

}

fn current_time_string() -> String {
  let now = Local::now();  
  now.format("%Y-%m-%d %H:%M:%S%.9f %z").to_string()
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
    pre_string.push_str(&current_time_string());
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

impl RequestHandler for CommandHandler {

  fn handle(&self, _: &Request) -> Response {
    let command_output = self.run_command();

    let pre_string = self.build_pre_string(command_output);

    let html_string = self.build_html_string(pre_string);

    build_response_string(
      StatusCode::Ok,
      html_string,
      ContentType::html())
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

  fn get_metadata(&self) -> Result<fs::Metadata, Box<Error>> {
    let metadata = fs::metadata(&self.file_path)?;
    Ok(metadata)
  }

  fn read_file(&self) -> Result<Vec<u8>, Box<Error>> {
    let mut file = File::open(&self.file_path)?;

    let mut file_contents = Vec::new();

    file.read_to_end(&mut file_contents)?;

    Ok(file_contents)
  }

}

impl RequestHandler for StaticFileHandler {

  fn handle(&self, req: &Request) -> Response {
    debug!("StaticFileHandler.handle req = {:?}", req);

    let file_metadata =
      match self.get_metadata() {
        Ok(metadata) => metadata,
        Err(_) => return build_response_status(StatusCode::InternalServerError)
      };

    let file_modified =
      match file_metadata.modified() {
        Ok(file_modified) => file_modified,
        Err(_) => return build_response_status(StatusCode::InternalServerError)
      };

    let if_modified_since_header_option: Option<&IfModifiedSince> =
      req.headers().get();

    if let Some(if_modified_since_header) = if_modified_since_header_option {
      let if_modified_since: SystemTime = if_modified_since_header.0.into();
      if systemtime_in_seconds(&file_modified) <=
         systemtime_in_seconds(&if_modified_since) {
        return build_response_status(StatusCode::NotModified)
          .with_header(LastModified(file_modified.into()));
      }
    }

    match self.read_file() {
      Ok(file_contents) => {
        build_response_vec(
          StatusCode::Ok,
          file_contents,
          ContentType(self.mime_type.clone()))
          .with_header(LastModified(file_modified.into()))
          .with_header(CacheControl(
             vec![CacheDirective::MaxAge(self.cache_max_age_seconds),
                  CacheDirective::Public]))
      },
      Err(_) => {
        build_response_status(StatusCode::InternalServerError)
      }
    }
  }

}

fn initialize_logging() -> Result<(), fern::InitError>{
  fern::Dispatch::new()
    .level(log::LogLevelFilter::Info)
    .level_for("hyper::http::request", log::LogLevelFilter::Warn)
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

fn read_config(config_file: &str) -> Result<Configuration, Box<Error>> {
  info!("reading {}", config_file);

  let mut file = File::open(config_file)?;

  let mut file_contents = String::new();

  file.read_to_string(&mut file_contents)?;

  let configuration: Configuration = serde_yaml::from_str(&file_contents)?;

  Ok(configuration)
}


fn build_route_configuration(config: &Configuration) -> Arc<RouteConfiguration> {
  let mut routes : HashMap<String, Box<RequestHandler>> = HashMap::new();

  let index_handler = IndexHandler::new(config).expect("error creating IndexHandler");
  routes.insert("/".to_string(), Box::new(index_handler));

  for command_info in &config.commands {
    let handler = CommandHandler::new(command_info.clone());
    routes.insert(command_info.http_path.clone(), Box::new(handler));
  }

  for static_path_info in &config.static_paths {
    let mime_type = static_path_info.content_type.parse().expect("invalid mime type");
    let handler = StaticFileHandler::new(
      static_path_info.fs_path.clone(),
      mime_type,
      static_path_info.cache_max_age_seconds);
    routes.insert(static_path_info.http_path.clone(), Box::new(handler));
  }

  Arc::new(RouteConfiguration { routes: routes })
}

fn main() {
  initialize_logging().expect("failed to initialize logging");

  let config_file = env::args().nth(1).expect("config file required as command line argument");

  let config = read_config(&config_file).expect("error reading configuration file");
  info!("config = {:?}", config);

  let listen_addr = config.listen_address.parse().expect("invalid listen_address");

  let route_configuration = build_route_configuration(&config);

  let cpu_pool = futures_cpupool::Builder::new()
    .pool_size(config.threads)
    .name_prefix("server-")
    .create();

  let http_server = Http::new()
    .bind(&listen_addr, move || Ok(
      ThreadedServer { 
        cpu_pool: cpu_pool.clone(),
        route_configuration: Arc::clone(&route_configuration)
      }
    ))
    .expect("bind failed");

  info!("Listening on http://{} with cpu pool size {}",
        http_server.local_addr().unwrap(),
        config.threads);

  http_server.run().expect("http_server.run failed");
}
