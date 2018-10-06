use futures::Future;

use hyper::header::{HeaderValue, CACHE_CONTROL, CONTENT_TYPE, LAST_MODIFIED};
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

fn build_last_modified(modified_result: ::std::io::Result<SystemTime>) -> Option<HeaderValue> {
    match modified_result {
        Ok(modified) => {
            let utc_modified = ::utils::system_time_to_utc(&modified);

            let last_modified_value = utc_modified.format("%a, %d %b %Y %H:%M:%S GMT").to_string();

            Some(HeaderValue::from_str(&last_modified_value).unwrap())
        }
        Err(_) => None,
    }
}

impl ::server::RequestHandler for StaticFileHandler {
    fn handle(&self, _: &::server::RequestContext) -> ::server::ResponseFuture {
        let file_path_clone = self.file_path.clone();
        let content_type_header_value_clone = self.content_type_header_value.clone();
        let cache_control_header_value_clone = self.cache_control_header_value.clone();

        Box::new(
            ::tokio_fs::file::File::open(file_path_clone)
                .and_then(move |file| {
                    file.metadata().and_then(move |metadata_result| {
                        let metadata = metadata_result.1;
                        let last_modified_header_value_option =
                            build_last_modified(metadata.modified());

                        let buf: Vec<u8> = Vec::with_capacity(metadata.len() as usize);
                        ::tokio_io::io::read_to_end(metadata_result.0, buf).and_then(
                            move |read_result| {
                                let mut response_builder = Response::builder();
                                response_builder.status(StatusCode::OK);
                                response_builder
                                    .header(CONTENT_TYPE, content_type_header_value_clone);
                                response_builder
                                    .header(CACHE_CONTROL, cache_control_header_value_clone);

                                if let Some(last_modified_header_value) =
                                    last_modified_header_value_option
                                {
                                    response_builder
                                        .header(LAST_MODIFIED, last_modified_header_value);
                                }

                                Ok(response_builder.body(From::from(read_result.1)).unwrap())
                            },
                        )
                    })
                }).or_else(|_| Ok(::server::build_response_status(StatusCode::NOT_FOUND))),
        )
    }
}
