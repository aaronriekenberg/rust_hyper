use futures::Future;

use hyper::StatusCode;

use std::borrow::Cow;
use std::process::Command;
use std::sync::Arc;

use tokio_process::CommandExt;

struct InnerAPIHandler {
    command_info: crate::config::CommandInfo,
    command_line_string: String,
}

impl InnerAPIHandler {
    fn run_command(
        &self,
    ) -> Box<Future<Item = String, Error = crate::server::HandlerError> + Send> {
        let mut command = Command::new(self.command_info.command());

        command.args(self.command_info.args());

        Box::new(
            command
                .output_async()
                .and_then(move |output| {
                    let mut combined_output =
                        String::with_capacity(output.stderr.len() + output.stdout.len());
                    combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
                    combined_output.push_str(&String::from_utf8_lossy(&output.stdout));
                    Ok(combined_output)
                })
                .or_else(move |err| Ok(format!("command error: {}", err))),
        )
    }
}

pub struct APIHandler {
    inner: Arc<InnerAPIHandler>,
}

impl APIHandler {
    pub fn new(command_info: crate::config::CommandInfo) -> Self {
        let mut command_line_string = String::new();

        command_line_string.push_str(command_info.command());

        for arg in command_info.args() {
            command_line_string.push(' ');
            command_line_string.push_str(arg);
        }

        APIHandler {
            inner: Arc::new(InnerAPIHandler {
                command_info,
                command_line_string,
            }),
        }
    }
}

#[derive(Serialize)]
struct APIResponse {
    now: String,
    command_line: String,
    output: String,
}

impl crate::server::RequestHandler for APIHandler {
    fn handle(&self, _: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        let inner_clone = Arc::clone(&self.inner);

        Box::new(self.inner.run_command().and_then(move |command_output| {
            let api_response = APIResponse {
                now: crate::utils::local_time_now_to_string(),
                command_line: inner_clone.command_line_string.clone(),
                output: command_output,
            };

            match ::serde_json::to_string(&api_response) {
                Ok(json_string) => Ok(crate::server::build_response_string(
                    StatusCode::OK,
                    Cow::from(json_string),
                    crate::server::application_json_content_type_header_value(),
                )),
                Err(_) => Ok(crate::server::build_response_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                )),
            }
        }))
    }
}
