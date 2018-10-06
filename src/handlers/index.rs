use futures::future;

use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::StatusCode;

use std::borrow::Cow;
use std::time::SystemTime;

pub struct IndexHandler {
    index_string: String,
}

impl IndexHandler {
    pub fn new(config: &::config::Configuration) -> Result<Self, Box<::std::error::Error>> {
        let static_paths_to_include: Vec<_> = config
            .static_paths()
            .iter()
            .filter(|s| s.include_in_main_page())
            .collect();

        let now = SystemTime::now();

        let mut last_modified_string = String::new();
        last_modified_string.push_str("Last Modified: ");
        last_modified_string.push_str(&::utils::local_time_to_string(
            ::utils::system_time_to_local(&now),
        ));

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
              @ if config.proxies().len() > 0 {
                h3 {
                  : "Proxies:"
                }
                ul {
                  @ for proxy_info in config.proxies() {
                    li {
                      a(href = proxy_info.http_path()) {
                        : proxy_info.description()
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
                  @ for static_path_info in &static_paths_to_include {
                    li {
                      a(href = static_path_info.http_path()) {
                        : static_path_info.fs_path()
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

        Ok(IndexHandler { index_string: s })
    }
}

impl ::server::RequestHandler for IndexHandler {
    fn handle(&self, _: &::server::RequestContext) -> ::server::ResponseFuture {
        Box::new(future::ok(::server::build_response_string(
            StatusCode::OK,
            Cow::from(self.index_string.clone()),
            ::server::text_html_content_type_header_value(),
        )))
    }
}
