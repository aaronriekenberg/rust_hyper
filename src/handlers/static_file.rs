use futures::Future;

use hyper::StatusCode;
use hyper::header::HeaderValue;

pub struct StaticFileHandler {
  file_path: String,
  content_type_header_value: HeaderValue
}

impl StaticFileHandler {

  pub fn new(file_path: String, content_type: &str) -> Result<Self, Box<::std::error::Error>> {
    let content_type_header_value = HeaderValue::from_str(content_type)?;

    Ok(StaticFileHandler {
      file_path,
      content_type_header_value
    })
  }

}

impl ::server::RequestHandler for StaticFileHandler {

  fn handle(&self, _: &::server::RequestContext) -> ::server::ResponseFuture {
    let file_path_clone = self.file_path.clone();
    let content_type_header_value_clone = self.content_type_header_value.clone();

    Box::new(::tokio_fs::file::File::open(file_path_clone)
      .and_then(move |file| {
        let buf: Vec<u8> = Vec::new();
        ::tokio_io::io::read_to_end(file, buf)
          .and_then(move |item| {
            Ok(::server::build_response_vec(
              StatusCode::OK,
              item.1,
              content_type_header_value_clone))
          })
      })
      .or_else(|_| Ok(::server::build_response_status(StatusCode::NOT_FOUND))))
  }

}
