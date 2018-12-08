use futures::future;

use horrorshow::helper::doctype;
use horrorshow::prelude::Raw;
use horrorshow::Template;

use hyper::StatusCode;

use std::borrow::Cow;

pub struct HTMLHandler {
    html: String,
}

impl HTMLHandler {
    pub fn new(command_info: crate::config::CommandInfo) -> Result<Self, Box<::std::error::Error>> {
        let mut command_line_string = String::new();

        command_line_string.push_str(command_info.command());

        for arg in command_info.args() {
            command_line_string.push(' ');
            command_line_string.push_str(arg);
        }

        let mut onload_string = String::new();
        onload_string.push_str("onload(\"");
        onload_string.push_str(&command_line_string);
        onload_string.push_str("\", \"");
        onload_string.push_str(command_info.api_path());
        onload_string.push_str("\")");

        let html = html! {
            : doctype::HTML;
            html {
              head {
                title: command_info.description();
                meta(name = "viewport", content = "width=device, initial-scale=1");
                link(rel = "stylesheet", type = "text/css", href = "/style.css");
                script(src = "/command.js") {}
              }
              body(onload = onload_string) {
                  div {
                      a(href = "..") {
                          : ".."
                      }
                      : Raw("&nbsp");
                      input(type = "checkbox", id = "autoRefresh", checked);
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
