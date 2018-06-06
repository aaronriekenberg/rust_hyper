use chrono::prelude::Local;

use futures::future::poll_fn;

use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::{Body, Response, StatusCode};

use std::borrow::Cow;
use std::process::Command;

use tokio_threadpool::blocking;

#[derive(Clone)]
pub struct CommandHandler {
  command_info: ::config::CommandInfo,
  command_line_string: String
}

impl CommandHandler {

  pub fn new(command_info: ::config::CommandInfo) -> Self {

    let mut command_line_string = String::new();

    command_line_string.push_str("$ ");
    command_line_string.push_str(command_info.command());

    for arg in command_info.args() {
      command_line_string.push(' ');
      command_line_string.push_str(arg);
    }

    CommandHandler {
      command_info,
      command_line_string
    }
  }

  fn run_command(&self) -> String {

    let mut command = Command::new(self.command_info.command());

    command.args(self.command_info.args());

    match command.output() {
      Ok(output) => {
        let mut combined_output =
          String::with_capacity(output.stderr.len() + output.stdout.len());
        combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
        combined_output.push_str(&String::from_utf8_lossy(&output.stdout));
        combined_output
      },
      Err(err) => format!("command error: {}", err),
    }
  }

  fn build_pre_string(&self, command_output: String) -> String {

    let mut pre_string = String::with_capacity(command_output.len() + 100);

    pre_string.push_str("Now: ");
    pre_string.push_str(&::utils::local_time_to_string(Local::now()));
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
          title: self.command_info.description();
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

  fn execute_command(&self) -> Response<Body> {

    let command_output = self.run_command();

    let pre_string = self.build_pre_string(command_output);

    let html_string = self.build_html_string(pre_string);

    ::server::build_response_string(
      StatusCode::OK,
      Cow::from(html_string),
      ::server::text_html_content_type_header_value())
  }

}

impl ::server::RequestHandler for CommandHandler {

  fn handle(&self, _: &::server::RequestContext) -> ::server::ResponseFuture {

    let self_clone = self.clone();

    Box::new(
      poll_fn(move || {
        blocking(|| self_clone.execute_command())
          .map_err(|e| ::std::io::Error::new(::std::io::ErrorKind::Other, e))
      })
    )
  }

}
