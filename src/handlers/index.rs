use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::{Body, Response, StatusCode};

use std::borrow::Cow;
use std::time::SystemTime;

pub struct IndexHandler {
  index_string: String,
  creation_time: SystemTime,
  cache_max_age_seconds: u32
}

impl IndexHandler {

  pub fn new(config: &::config::Configuration) -> Result<Self, Box<::std::error::Error>> {

    let static_paths_to_include: Vec<_> = 
      config.static_paths().iter().filter(|s| s.include_in_main_page()).collect();

    let now = SystemTime::now();

    let mut last_modified_string = String::new();
    last_modified_string.push_str("Last Modified: ");
    last_modified_string.push_str(&::utils::local_time_to_string(::utils::system_time_to_local(&now)));

    let s = html! {
      : doctype::HTML;
      html {
        head {
          title: config.main_page_info().title();
          meta(name = "viewport", content = "width=device, initial-scale=1");
          link(rel = "stylesheet", type = "text/css", href = "style.css");
        }
        body {
          h2 {
            : config.main_page_info().title();
          }
          @ if config.commands().len() > 0 {
            h3 {
              : "Comamnds:"
            }
            ul {
              @ for command_info in config.commands() {
                li {
                  a(href = command_info.http_path()) {
                    : command_info.description()
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
                  a(href = static_path.http_path()) {
                    : static_path.fs_path()
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

    Ok(
      IndexHandler {
        index_string: s,
        creation_time: now,
        cache_max_age_seconds: config.main_page_info().cache_max_age_seconds()
      }
    )
  }

}

impl ::server::RequestHandler for IndexHandler {

  fn handle(&self, req_context: &::server::RequestContext) -> Response<Body> {

    match ::server::handle_not_modified(
      &req_context,
      &self.creation_time,
      self.cache_max_age_seconds) {

      Some(response) => response,

      None =>
        ::server::build_response_string(
          StatusCode::OK,
          Cow::from(self.index_string.clone()),
          Cow::from("text/html"))
          //.with_header(header::LastModified(From::from(self.creation_time)))
          //.with_header(header::CacheControl(
          //   vec![header::CacheDirective::Public,
          //        header::CacheDirective::MaxAge(self.cache_max_age_seconds)]))

    }
  }

}
