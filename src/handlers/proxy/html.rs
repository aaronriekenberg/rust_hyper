use futures::future;

use horrorshow::helper::doctype;
use horrorshow::prelude::Raw;
use horrorshow::Template;
use horrorshow::{append_html, html};

use hyper::StatusCode;

use std::borrow::Cow;

pub struct HTMLHandler {
    html: String,
}

impl HTMLHandler {
    pub fn new(proxy_info: crate::config::ProxyInfo) -> Result<Self, Box<::std::error::Error>> {
        let mut request_string = String::new();

        request_string.push_str("GET ");
        request_string.push_str(proxy_info.url());

        let mut onload_string = String::new();
        onload_string.push_str("onload('");
        onload_string.push_str(&request_string);
        onload_string.push_str("', '");
        onload_string.push_str(proxy_info.api_path());
        onload_string.push_str("')");

        let html = html! {
            : doctype::HTML;
            html {
              head {
                title: proxy_info.description();
                meta(name = "viewport", content = "width=device-width, initial-scale=1");
                link(rel = "stylesheet", type = "text/css", href = "/style.css");
                script(src = "/proxy.js") {}
              }
              body(onload = Raw(onload_string)) {
                  div {
                      a(href = "..") {
                          : ".."
                      }
                      : Raw("&nbsp");
                      input(type = "checkbox", id = "autoRefresh");
                      label(for = "autoRefresh") {
                          : "Auto Refresh"
                      }
                  }
                  pre {}
              }
            }
        }
        .into_string()?;

        Ok(HTMLHandler { html })
    }
}

impl crate::server::RequestHandler for HTMLHandler {
    fn handle(&self, _: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        Box::new(future::ok(crate::server::build_response_string(
            StatusCode::OK,
            Cow::from(self.html.clone()),
            crate::server::text_html_content_type_header_value(),
        )))
    }
}
