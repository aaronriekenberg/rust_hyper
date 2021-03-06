use chrono::prelude::{DateTime, Utc};

use futures::{future, Future};

use hyper::header::{HeaderValue, CACHE_CONTROL, CONTENT_TYPE, IF_MODIFIED_SINCE, LAST_MODIFIED};
use hyper::{Response, StatusCode};

use std::time::SystemTime;

pub struct StaticFileHandler {
    file_path: String,
    content_type_header_value: HeaderValue,
    cache_control_header_value: HeaderValue,
}

impl StaticFileHandler {
    pub fn new(
        file_path: String,
        content_type: &str,
        cache_control: &str,
    ) -> Result<Self, Box<::std::error::Error>> {
        let content_type_header_value = HeaderValue::from_str(content_type)?;
        let cache_control_header_value = HeaderValue::from_str(cache_control)?;

        Ok(StaticFileHandler {
            file_path,
            content_type_header_value,
            cache_control_header_value,
        })
    }
}

fn get_if_modified_since_time(
    req_context: &crate::server::RequestContext,
) -> Option<DateTime<Utc>> {
    let req_headers = req_context.req().headers();

    let mut return_value = None;

    if let Some(header_value) = req_headers.get(IF_MODIFIED_SINCE) {
        if let Ok(str_value) = header_value.to_str() {
            if let Ok(dt) = DateTime::parse_from_rfc2822(str_value) {
                return_value = Some(dt.with_timezone(&Utc));
            }
        }
    }

    return_value
}

fn file_modified_since_header(
    if_modified_since_time_option: &Option<DateTime<Utc>>,
    file_last_modified_result: &::std::io::Result<SystemTime>,
) -> bool {
    let mut return_value = true;

    if let (Some(if_modified_since_time), Ok(file_last_modified)) =
        (if_modified_since_time_option, file_last_modified_result)
    {
        let utc_file_last_modified = crate::utils::system_time_to_utc(*file_last_modified);
        return_value = if_modified_since_time.timestamp() < utc_file_last_modified.timestamp();
    }

    return_value
}

fn build_last_modified_header(
    modified_result: &::std::io::Result<SystemTime>,
) -> Option<HeaderValue> {
    let mut return_value = None;

    if let Ok(modified) = modified_result {
        let utc_modified = crate::utils::system_time_to_utc(*modified);

        let last_modified_value = utc_modified.format("%a, %d %b %Y %H:%M:%S GMT").to_string();

        return_value = Some(HeaderValue::from_str(&last_modified_value).unwrap())
    }

    return_value
}

fn build_file_response(
    content_type_header_value_clone: HeaderValue,
    cache_control_header_value_clone: HeaderValue,
    last_modified_header_value_option: Option<HeaderValue>,
    file: tokio_fs::File,
    metadata: ::std::fs::Metadata,
) -> Box<Future<Item = ::hyper::Response<::hyper::Body>, Error = ::std::io::Error> + Send> {
    let buf: Vec<u8> = Vec::with_capacity(metadata.len() as usize);
    Box::new(
        ::tokio_io::io::read_to_end(file, buf).and_then(move |read_result| {
            let mut response_builder = Response::builder();
            response_builder.status(StatusCode::OK);
            response_builder.header(CONTENT_TYPE, content_type_header_value_clone);
            response_builder.header(CACHE_CONTROL, cache_control_header_value_clone);

            if let Some(last_modified_header_value) = last_modified_header_value_option {
                response_builder.header(LAST_MODIFIED, last_modified_header_value);
            }

            Ok(response_builder.body(From::from(read_result.1)).unwrap())
        }),
    )
}

fn build_not_modified(
    content_type_header_value_clone: HeaderValue,
    cache_control_header_value_clone: HeaderValue,
    last_modified_header_value_option: Option<HeaderValue>,
) -> Box<Future<Item = ::hyper::Response<::hyper::Body>, Error = ::std::io::Error> + Send> {
    let mut response_builder = Response::builder();
    response_builder.status(StatusCode::NOT_MODIFIED);
    response_builder.header(CONTENT_TYPE, content_type_header_value_clone);
    response_builder.header(CACHE_CONTROL, cache_control_header_value_clone);

    if let Some(last_modified_header_value) = last_modified_header_value_option {
        response_builder.header(LAST_MODIFIED, last_modified_header_value);
    }

    Box::new(future::ok(
        response_builder.body(::hyper::Body::empty()).unwrap(),
    ))
}

impl crate::server::RequestHandler for StaticFileHandler {
    fn handle(&self, req_context: &crate::server::RequestContext) -> crate::server::ResponseFuture {
        let file_path_clone = self.file_path.clone();
        let content_type_header_value_clone = self.content_type_header_value.clone();
        let cache_control_header_value_clone = self.cache_control_header_value.clone();

        let if_modified_since_time_option = get_if_modified_since_time(&req_context);

        Box::new(
            ::tokio_fs::file::File::open(file_path_clone)
                .and_then(move |file| {
                    file.metadata().and_then(move |metadata_result| {
                        let (file, metadata) = metadata_result;
                        let file_modified = metadata.modified();

                        let last_modified_header_value_option =
                            build_last_modified_header(&file_modified);

                        if file_modified_since_header(
                            &if_modified_since_time_option,
                            &file_modified,
                        ) {
                            build_file_response(
                                content_type_header_value_clone,
                                cache_control_header_value_clone,
                                last_modified_header_value_option,
                                file,
                                metadata,
                            )
                        } else {
                            build_not_modified(
                                content_type_header_value_clone,
                                cache_control_header_value_clone,
                                last_modified_header_value_option,
                            )
                        }
                    })
                })
                .or_else(|_| Ok(crate::server::build_response_status(StatusCode::NOT_FOUND))),
        )
    }
}
