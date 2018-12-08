use futures::future;

use horrorshow::helper::doctype;
use horrorshow::Template;
use horrorshow::{append_html, html};

use hyper::StatusCode;

use std::borrow::Cow;

pub struct IndexHandler {
    index_string: String,
}

impl IndexHandler {
    pub fn new(
        config: &crate::config::Configuration,
        environment: &crate::environment::Environment,
    ) -> Result<Self, Box<::std::error::Error>> {
        let static_paths_to_include: Vec<_> = config
            .static_paths()
            .iter()
            .filter(|s| s.include_in_main_page())
            .collect();

        let mut last_modified_string = String::new();
        last_modified_string.push_str("Last Modified: ");
        last_modified_string.push_str(&crate::utils::local_time_now_to_string());

        let mut git_hash_string = String::new();
        git_hash_string.push_str("Git Hash: ");
        git_hash_string.push_str(&environment.git_hash());

        let s = html! {
          : doctype::HTML;
          html {
            head {
              title: config.main_page_info().title();
              meta(name = "viewport", content = "width=device-width, initial-scale=1");
              link(rel = "stylesheet", type = "text/css", href = "/style.css");
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
                      a(href = command_info.html_path()) {
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
                      a(href = proxy_info.html_path()) {
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
              h3 {
                : "Debugging:"
              }
              ul {
                li {
                  a(href = "/configuration") {
                    : "configuration"
                  }
                }
                li {
                  a(href = "/environment") {
                    : "environment"
                  }
                }
              }
              hr;
              small {
                : &last_modified_string
              }
              br;
              small {
                : &git_hash_string
              }
            }
          }
        }
        .into_string()?;

        Ok(IndexHandler { index_string: s })
    }
}

impl crate::server::RequestHandler for IndexHandler {
    fn handle(&self, _: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        Box::new(future::ok(crate::server::build_response_string(
            StatusCode::OK,
            Cow::from(self.index_string.clone()),
            crate::server::text_html_content_type_header_value(),
        )))
    }
}
