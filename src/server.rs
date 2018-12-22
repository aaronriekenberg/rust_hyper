use futures::{future, Future};

use log::{info, warn};

use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::{Body, Request, Response, Server, StatusCode};

use std::borrow::Cow;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

pub type HyperHttpClient = ::hyper::Client<
    ::hyper::client::HttpConnector<::hyper::client::connect::dns::TokioThreadpoolGaiResolver>,
    ::hyper::Body,
>;

pub struct ApplicationContext {
    http_client: HyperHttpClient,
}

impl ApplicationContext {
    fn new(http_client: HyperHttpClient) -> Self {
        ApplicationContext { http_client }
    }

    pub fn http_client(&self) -> &HyperHttpClient {
        &self.http_client
    }
}

pub struct RequestContext {
    req: Request<Body>,
    app_context: Arc<ApplicationContext>,
    start_time: Instant,
}

impl RequestContext {
    fn new(req: Request<Body>, app_context: Arc<ApplicationContext>) -> Self {
        RequestContext {
            req,
            app_context,
            start_time: Instant::now(),
        }
    }

    pub fn app_context(&self) -> &Arc<ApplicationContext> {
        &self.app_context
    }

    pub fn req(&self) -> &Request<Body> {
        &self.req
    }
}

struct RequestLogInfo {
    start_time: Instant,
    method: String,
    uri: String,
    version: String,
}

impl RequestLogInfo {
    fn new(req_context: &RequestContext) -> Self {
        let req = &req_context.req;

        RequestLogInfo {
            start_time: req_context.start_time,
            method: req.method().to_string(),
            uri: req.uri().to_string(),
            version: format!("{:?}", req.version()),
        }
    }
}

fn log_request_and_response(req_log_info: RequestLogInfo, resp: &Response<Body>) {
    let response_status = resp.status().as_u16().to_string();

    let duration = crate::utils::duration_in_seconds_f64(req_log_info.start_time.elapsed());

    info!(
        "\"{} {} {}\" {} {:.9}s",
        req_log_info.method, req_log_info.uri, req_log_info.version, response_status, duration
    );
}

#[derive(Debug)]
pub enum HandlerError {
    Hyper(::hyper::Error),
    IoError(::std::io::Error),
}

impl From<::hyper::Error> for HandlerError {
    fn from(err: ::hyper::Error) -> HandlerError {
        HandlerError::Hyper(err)
    }
}

impl From<::std::io::Error> for HandlerError {
    fn from(err: ::std::io::Error) -> HandlerError {
        HandlerError::IoError(err)
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HandlerError::Hyper(ref e) => fmt::Display::fmt(e, f),
            HandlerError::IoError(ref e) => fmt::Display::fmt(e, f),
        }
    }
}

impl error::Error for HandlerError {
    fn description(&self) -> &str {
        match *self {
            HandlerError::Hyper(_) => "Hyper Error",
            HandlerError::IoError(_) => "IO Error",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            HandlerError::Hyper(ref error) => Some(error),
            HandlerError::IoError(ref error) => Some(error),
        }
    }
}

pub type ResponseFuture =
    Box<Future<Item = ::hyper::Response<::hyper::Body>, Error = HandlerError> + Send>;

pub trait RequestHandler: Send + Sync {
    fn handle(&self, req_context: &RequestContext) -> ResponseFuture;
}

pub type RouteConfigurationHandler = Box<dyn RequestHandler>;
pub type RouteConfigurationHandlerMap = HashMap<String, RouteConfigurationHandler>;

pub struct RouteConfiguration {
    path_to_handler: RouteConfigurationHandlerMap,
    not_found_handler: RouteConfigurationHandler,
}

impl RouteConfiguration {
    pub fn new(
        path_to_handler: RouteConfigurationHandlerMap,
        not_found_handler: RouteConfigurationHandler,
    ) -> Self {
        RouteConfiguration {
            path_to_handler,
            not_found_handler,
        }
    }

    pub fn path_to_handler(&self) -> &RouteConfigurationHandlerMap {
        &self.path_to_handler
    }

    pub fn not_found_handler(&self) -> &RouteConfigurationHandler {
        &self.not_found_handler
    }
}

pub fn text_plain_content_type_header_value() -> HeaderValue {
    HeaderValue::from_static("text/plain")
}

pub fn text_html_content_type_header_value() -> HeaderValue {
    HeaderValue::from_static("text/html")
}

pub fn application_json_content_type_header_value() -> HeaderValue {
    HeaderValue::from_static("application/json")
}

pub fn build_response_status(status_code: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .body(Body::empty())
        .unwrap()
}

pub fn build_response_string(
    status_code: StatusCode,
    body: Cow<'static, str>,
    content_type: HeaderValue,
) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, content_type)
        .body(From::from(body))
        .unwrap()
}

struct InnerThreadedServer {
    application_context: Arc<ApplicationContext>,
    route_configuration: RouteConfiguration,
}

#[derive(Clone)]
struct ThreadedServer {
    inner: Arc<InnerThreadedServer>,
}

impl ThreadedServer {
    fn new(
        application_context: Arc<ApplicationContext>,
        route_configuration: RouteConfiguration,
    ) -> Self {
        ThreadedServer {
            inner: Arc::new(InnerThreadedServer {
                application_context,
                route_configuration,
            }),
        }
    }
}

impl ThreadedServer {
    fn call(&self, req: Request<Body>) -> ResponseFuture {
        let req_context = RequestContext::new(req, Arc::clone(&self.inner.application_context));

        let req_log_info = RequestLogInfo::new(&req_context);

        let route_configuration = &self.inner.route_configuration;

        let handler = route_configuration
            .path_to_handler()
            .get(req_context.req.uri().path())
            .unwrap_or(route_configuration.not_found_handler());

        Box::new(
            handler
                .handle(&req_context)
                .then(move |result| match result {
                    Ok(resp) => {
                        log_request_and_response(req_log_info, &resp);
                        Ok(resp)
                    }
                    Err(e) => {
                        match e {
                            HandlerError::Hyper(e) => warn!("hyper handler error: {}", e),
                            HandlerError::IoError(e) => warn!("io handler error: {}", e),
                        }
                        let resp = build_response_status(StatusCode::INTERNAL_SERVER_ERROR);
                        log_request_and_response(req_log_info, &resp);
                        Ok(resp)
                    }
                }),
        )
    }
}

pub struct ServerConfiguration {
    listen_addr: SocketAddr,
    tcp_nodelay: bool,
}

impl ServerConfiguration {
    pub fn new(listen_addr: SocketAddr, tcp_nodelay: bool) -> Self {
        ServerConfiguration {
            listen_addr,
            tcp_nodelay,
        }
    }
}

pub fn run_forever(
    server_configuration: ServerConfiguration,
    route_configuration: RouteConfiguration,
) -> Result<(), Box<error::Error>> {
    ::hyper::rt::run(future::lazy(move || {
        let mut http_connector =
            ::hyper::client::HttpConnector::new_with_tokio_threadpool_resolver();
        http_connector.set_nodelay(server_configuration.tcp_nodelay);

        let http_client = ::hyper::client::Client::builder().build(http_connector);

        let application_context = Arc::new(ApplicationContext::new(http_client));

        let threaded_server = ThreadedServer::new(application_context, route_configuration);

        let server = Server::bind(&server_configuration.listen_addr)
            .tcp_nodelay(server_configuration.tcp_nodelay)
            .serve(move || {
                let threaded_server_clone = threaded_server.clone();

                service_fn(move |req: Request<Body>| threaded_server_clone.call(req))
            })
            .map_err(|e| warn!("serve error: {}", e));

        info!("Listening on http://{}", server_configuration.listen_addr);

        server
    }));

    Err(From::from("run_forever exiting"))
}
