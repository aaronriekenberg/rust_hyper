use chrono::prelude::Local;

use futures::Future;

use horrorshow::helper::doctype;
use horrorshow::Template;

use hyper::{Body, Client, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper::rt::Stream;

use std::borrow::Cow;
use std::sync::Arc;

struct InnerProxyHandler {
  uri: Uri,
  proxy_info: ::config::ProxyInfo,
  client: Arc<Client<HttpConnector, Body>>
}

impl InnerProxyHandler {

  fn fetch_proxy(&self) -> Box<Future<Item=String, Error=::std::io::Error> + Send> {

    Box::new(
      self.client.get(self.uri.clone())
        .and_then(move |response| {
          response.into_body().concat2()
        })
        .and_then(|body| {
          Ok(String::from_utf8_lossy(&body).into_owned())
        })
        .or_else(move |err| {
          Ok(format!("proxy error: {}", err))
        })
      )
  }


  fn build_pre_string(&self, proxy_output: String) -> String {

    let mut pre_string = String::with_capacity(proxy_output.len() + 100);

    pre_string.push_str("Now: ");
    pre_string.push_str(&::utils::local_time_to_string(Local::now()));
    pre_string.push_str("\n\nGET ");
    pre_string.push_str(&self.proxy_info.url());
    pre_string.push_str("\n\n");
    pre_string.push_str(&proxy_output);

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
  inner: Arc<InnerProxyHandler>
}

impl ProxyHandler {

  pub fn new(
    proxy_info: ::config::ProxyInfo,
    proxy_http_client: Arc<Client<HttpConnector, Body>>) -> Result<Self, Box<::std::error::Error>> {

    let uri = proxy_info.url().parse()?;

    Ok(ProxyHandler {
      inner: Arc::new(
        InnerProxyHandler {
          uri,
          proxy_info,
          client: proxy_http_client
        }
      )
    })
  }

}

impl ::server::RequestHandler for ProxyHandler {

  fn handle(&self, _: &::server::RequestContext) -> ::server::ResponseFuture {

    let inner_clone = Arc::clone(&self.inner);

    Box::new(
      self.inner.fetch_proxy()
        .and_then(move |proxy_output| {

          let pre_string = inner_clone.build_pre_string(proxy_output);

          let html_string = inner_clone.build_html_string(pre_string);

          Ok(::server::build_response_string(
            StatusCode::OK,
            Cow::from(html_string),
            ::server::text_html_content_type_header_value()))

        }))
  }

}

