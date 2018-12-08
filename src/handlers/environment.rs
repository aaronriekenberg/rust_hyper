use futures::future;

use hyper::StatusCode;

use std::borrow::Cow;

pub struct EnvironmentHandler {
    environment_string: String,
}

impl EnvironmentHandler {
    pub fn new(environment: &crate::environment::Environment) -> Self {
        let environment_string = format!("{:#?}", environment);
        EnvironmentHandler { environment_string }
    }
}

impl crate::server::RequestHandler for EnvironmentHandler {
    fn handle(&self, _: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        Box::new(future::ok(crate::server::build_response_string(
            StatusCode::OK,
            Cow::from(self.environment_string.clone()),
            crate::server::text_plain_content_type_header_value(),
        )))
    }
}
