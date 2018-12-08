use futures::Future;

use hyper::rt::Stream;
use hyper::{StatusCode, Uri};

use serde_derive::Serialize;

use std::borrow::Cow;
use std::sync::Arc;

#[derive(Default)]
struct ResponseInfo {
    version: String,
    status: String,
    headers: String,
    body: String,
}

struct InnerAPIHandler {
    uri: Uri,
}

impl InnerAPIHandler {
    fn fetch_proxy(
        &self,
        http_client: &crate::server::HyperHttpClient,
    ) -> Box<Future<Item = ResponseInfo, Error = crate::server::HandlerError> + Send> {
        Box::new(
            http_client
                .get(self.uri.clone())
                .and_then(|response| {
                    let version = format!("{:?}", response.version());
                    let status = format!("{}", response.status());
                    let headers = format!("{:#?}", response.headers());
                    response
                        .into_body()
                        .concat2()
                        .then(move |result| match result {
                            Ok(body) => Ok(ResponseInfo {
                                version,
                                status,
                                headers,
                                body: String::from_utf8_lossy(&body).into_owned(),
                            }),
                            Err(e) => Ok(ResponseInfo {
                                version,
                                status,
                                headers,
                                body: format!("proxy body error: {}", e),
                            }),
                        })
                })
                .or_else(|err| {
                    Ok(ResponseInfo {
                        body: format!("proxy error: {}", err),
                        ..Default::default()
                    })
                }),
        )
    }
}

pub struct APIHandler {
    inner: Arc<InnerAPIHandler>,
}

impl APIHandler {
    pub fn new(proxy_info: crate::config::ProxyInfo) -> Result<Self, Box<::std::error::Error>> {
        let uri = proxy_info.url().parse()?;

        Ok(APIHandler {
            inner: Arc::new(InnerAPIHandler { uri }),
        })
    }
}

#[derive(Serialize)]
struct APIResponse {
    now: String,
    method: String,
    url: String,
    version: String,
    status: String,
    headers: String,
    body: String,
}

impl crate::server::RequestHandler for APIHandler {
    fn handle(&self, req_context: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        let inner_clone = Arc::clone(&self.inner);

        let http_client = req_context.app_context().http_client();

        Box::new(
            self.inner
                .fetch_proxy(http_client)
                .and_then(move |response_info| {
                    let api_response = APIResponse {
                        now: crate::utils::local_time_now_to_string(),
                        method: "GET".to_string(),
                        url: inner_clone.uri.to_string(),
                        version: response_info.version,
                        status: response_info.status,
                        headers: response_info.headers,
                        body: response_info.body,
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
                }),
        )
    }
}
