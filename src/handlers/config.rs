use futures::future;

use hyper::StatusCode;

use std::borrow::Cow;

pub struct ConfigHandler {
    config_string: String,
}

impl ConfigHandler {
    pub fn new(config: &crate::config::Configuration) -> Self {
        let config_string = format!("{:#?}", config);
        ConfigHandler { config_string }
    }
}

impl crate::server::RequestHandler for ConfigHandler {
    fn handle(&self, _: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        Box::new(future::ok(crate::server::build_response_string(
            StatusCode::OK,
            Cow::from(self.config_string.clone()),
            crate::server::text_plain_content_type_header_value(),
        )))
    }
}
