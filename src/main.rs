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
use chrono::{DateTime, TimeZone};

use crypto::digest::Digest;
use crypto::sha2::Sha256;

use futures_cpupool::{CpuFuture, CpuPool};

use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::header;
use hyper::server::{Http, Service, Request, Response};
use hyper::StatusCode;

use mime::Mime;

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::thread;

#[derive(Debug)]
struct RequestContext {
  req: Request,
  start_time: Instant
}

impl RequestContext {

  pub fn new(req: Request) -> Self {
    RequestContext { req: req, start_time: Instant::now() }
  }

}

trait RequestHandler : Send + Sync {
  fn handle(&self, req_context: &RequestContext) -> Response;
}

struct RouteConfiguration {
  path_to_handler: HashMap<String, Box<RequestHandler>>,
  not_found_handler: Box<RequestHandler>
}

fn build_response_status(
  status_code: StatusCode) -> Response {
  Response::new()
    .with_status(status_code)
}

fn build_response_string(
  status_code: StatusCode,
  body: Cow<'static, str>,
  content_type: header::ContentType) -> Response
{
  build_response_status(status_code)
    .with_header(content_type)
    .with_header(header::ContentLength(body.len() as u64))
    .with_body(body)
}

fn build_response_vec(
  status_code: StatusCode,
  body: Vec<u8>,
  content_type: header::ContentType) -> Response
{
  build_response_status(status_code)
    .with_header(content_type)
    .with_header(header::ContentLength(body.len() as u64))
    .with_body(body)
}

fn local_time_to_string(dt: DateTime<Local>) -> String {
  dt.format("%Y-%m-%d %H:%M:%S%.9f %z").to_string()
}

fn system_time_to_local(st: &std::time::SystemTime) -> DateTime<Local> {
  match st.duration_since(UNIX_EPOCH) {
    Ok(dur) => {
      Local.timestamp(dur.as_secs() as i64, dur.subsec_nanos())
    },
    Err(_) => {
      Local.timestamp(0, 0)
    }
  }
}

fn system_time_in_seconds_u64(st: &std::time::SystemTime) -> u64 {
  match st.duration_since(UNIX_EPOCH) {
    Ok(dur) => {
      dur.as_secs()
    },
    Err(_) => 0
  }
}

fn duration_in_seconds_f64(duration: &std::time::Duration) -> f64 {
  (duration.as_secs() as f64) + ((duration.subsec_nanos() as f64) / 1e9)
}

fn handle_not_modified(
  req: &Request,
  data_last_modified: &SystemTime,
  cache_max_age_seconds: u32) -> Option<Response> {

  if let Some(if_modified_since_header) =
     req.headers().get::<header::IfModifiedSince>() {
    let if_modified_since = SystemTime::from(if_modified_since_header.0);
    if system_time_in_seconds_u64(&data_last_modified) <=
       system_time_in_seconds_u64(&if_modified_since) {
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

fn log_request_and_response(
  req_context: &RequestContext,
  resp: &Response) {

  let req = &req_context.req;

  let remote_addr = match req.remote_addr() {
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

  let duration = duration_in_seconds_f64(&req_context.start_time.elapsed());

  info!("{} \"{} {} {}\" {} {} {:.9}s", 
        remote_addr,
        method, uri, version,
        response_status, content_length,
        duration);
}

struct InnerThreadedServer {
  cpu_pool: CpuPool,
  max_pending_tasks: usize,
  pending_tasks: AtomicUsize,
  route_configuration: RouteConfiguration
}

#[derive(Clone)]
struct ThreadedServer {
  inner: Arc<InnerThreadedServer>
}

impl ThreadedServer {

  pub fn new(
    pool_threads: usize,
    max_pending_tasks: usize,
    route_configuration: RouteConfiguration) -> Self {

    let cpu_pool = futures_cpupool::Builder::new()
      .pool_size(pool_threads)
      .name_prefix("server-")
      .create();

    let inner = Arc::new(InnerThreadedServer {
      cpu_pool: cpu_pool,
      max_pending_tasks: max_pending_tasks,
      pending_tasks: AtomicUsize::new(0),
      route_configuration: route_configuration
    });

    ThreadedServer { inner: inner }
  }

  fn wait_for_pending_tasks(&self) {
    let inner = &self.inner;
    loop {
      let pending_tasks = inner.pending_tasks.load(Ordering::SeqCst);
      if pending_tasks < inner.max_pending_tasks {
        break;
      } else {
        warn!("pending tasks is big: {}", pending_tasks);
        thread::sleep(std::time::Duration::from_millis(100));
      }
    }
  }

}

impl Service for ThreadedServer {

  type Request = Request;
  type Response = Response;
  type Error = hyper::Error;
  type Future = CpuFuture<Response, hyper::Error>;

  fn call(&self, req: Request) -> Self::Future {
    let req_context = RequestContext::new(req);

    self.wait_for_pending_tasks();

    let inner = Arc::clone(&self.inner);

    self.inner.pending_tasks.fetch_add(1, Ordering::SeqCst);

    self.inner.cpu_pool.spawn_fn(move || {

      debug!("do_in_thread thread {:?} req_context {:?}", thread::current().name(), req_context);

      let path = req_context.req.path();

      let handler = inner.route_configuration.path_to_handler
        .get(path)
        .unwrap_or(&inner.route_configuration.not_found_handler);

      let response = handler.handle(&req_context);

      log_request_and_response(&req_context, &response);

      inner.pending_tasks.fetch_sub(1, Ordering::SeqCst);

      Ok(response)

    })
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

impl RequestHandler for NotFoundHandler {

  fn handle(&self, _: &RequestContext) -> Response {
    build_response_string(
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
    last_modified_string.push_str(&local_time_to_string(system_time_to_local(&now)));

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

impl RequestHandler for IndexHandler {

  fn handle(&self, req_context: &RequestContext) -> Response {
    if let Some(response) = handle_not_modified(
      &req_context.req,
      &self.creation_time,
      self.cache_max_age_seconds) {
      return response;
    }

    build_response_string(
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
    pre_string.push_str(&local_time_to_string(Local::now()));
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

  fn handle(&self, _: &RequestContext) -> Response {
    let command_output = self.run_command();

    let pre_string = self.build_pre_string(command_output);

    let html_string = self.build_html_string(pre_string);

    build_response_string(
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

impl RequestHandler for StaticFileHandler {

  fn handle(&self, req_context: &RequestContext) -> Response {
    debug!("StaticFileHandler.handle req_context = {:?}", req_context);

    let file_metadata =
      match fs::metadata(&self.file_path) {
        Ok(metadata) => metadata,
        Err(_) => return build_response_status(StatusCode::NotFound)
      };

    let file_modified =
      match file_metadata.modified() {
        Ok(file_modified) => file_modified,
        Err(_) => return build_response_status(StatusCode::NotFound)
      };

    if let Some(response) = handle_not_modified(
      &req_context.req,
      &file_modified,
      self.cache_max_age_seconds) {
      return response;
    }

    match self.read_file() {
      Ok(file_contents) => {
        build_response_vec(
          StatusCode::Ok,
          file_contents,
          header::ContentType(self.mime_type.clone()))
          .with_header(header::LastModified(file_modified.into()))
          .with_header(header::CacheControl(
             vec![header::CacheDirective::Public,
                  header::CacheDirective::MaxAge(self.cache_max_age_seconds)]))
      },
      Err(_) => {
        build_response_status(StatusCode::NotFound)
      }
    }
  }

}

fn initialize_logging() -> Result<(), fern::InitError>{
  fern::Dispatch::new()
    .level(log::LogLevelFilter::Info)
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

fn build_route_configuration(config: &Configuration) -> RouteConfiguration {
  let mut path_to_handler : HashMap<String, Box<RequestHandler>> = HashMap::new();

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

  RouteConfiguration { 
    path_to_handler: path_to_handler,
    not_found_handler: not_found_handler
  }
}

fn create_threaded_server(config: &Configuration) -> ThreadedServer {

  let route_configuration = build_route_configuration(&config);

  ThreadedServer::new(
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
