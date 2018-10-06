use chrono::prelude::Local;

use futures::Future;

use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::rt::Stream;
use hyper::{StatusCode, Uri};

use std::borrow::Cow;
use std::sync::Arc;

#[derive(Default)]
struct ResponseInfo {
    version: String,
    status: String,
    headers: String,
    body: String,
}

struct InnerProxyHandler {
    uri: Uri,
    proxy_info: ::config::ProxyInfo,
}

impl InnerProxyHandler {
    fn fetch_proxy(
        &self,
        http_client: &::server::HyperHttpClient,
    ) -> Box<Future<Item = ResponseInfo, Error = ::server::HandlerError> + Send> {
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
                }).or_else(|err| {
                    Ok(ResponseInfo {
                        body: format!("proxy error: {}", err),
                        ..Default::default()
                    })
                }),
        )
    }

    fn build_pre_string(&self, response_info: ResponseInfo) -> String {
        let mut pre_string =
            String::with_capacity(response_info.headers.len() + response_info.body.len() + 100);

        pre_string.push_str("Now: ");
        pre_string.push_str(&::utils::local_time_to_string(Local::now()));
        pre_string.push_str("\n\nGET ");
        pre_string.push_str(&self.proxy_info.url());
        pre_string.push_str("\n\nResponse Status: ");
        pre_string.push_str(&response_info.version);
        pre_string.push_str(" ");
        pre_string.push_str(&response_info.status);
        pre_string.push_str("\n\nResponse Headers: ");
        pre_string.push_str(&response_info.headers);
        pre_string.push_str("\n\n");
        pre_string.push_str(&response_info.body);

        pre_string
    }

    fn build_html_string(&self, pre_string: String) -> String {
        let html_string = html! {
          : doctype::HTML;
          html {
            head {
              title: self.proxy_info.description();
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
}

pub struct ProxyHandler {
    inner: Arc<InnerProxyHandler>,
}

impl ProxyHandler {
    pub fn new(proxy_info: ::config::ProxyInfo) -> Result<Self, Box<::std::error::Error>> {
        let uri = proxy_info.url().parse()?;

        Ok(ProxyHandler {
            inner: Arc::new(InnerProxyHandler { uri, proxy_info }),
        })
    }
}

impl ::server::RequestHandler for ProxyHandler {
    fn handle(&self, req_context: &::server::RequestContext) -> ::server::ResponseFuture {
        let inner_clone = Arc::clone(&self.inner);

        let http_client = req_context.app_context().http_client();

        Box::new(
            self.inner
                .fetch_proxy(http_client)
                .and_then(move |response_info| {
                    let pre_string = inner_clone.build_pre_string(response_info);

                    let html_string = inner_clone.build_html_string(pre_string);

                    Ok(::server::build_response_string(
                        StatusCode::OK,
                        Cow::from(html_string),
                        ::server::text_html_content_type_header_value(),
                    ))
                }),
        )
    }
}
