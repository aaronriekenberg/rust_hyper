use mime::Mime;

use hyper::{Body, Response, StatusCode};

use std::borrow::Cow;
use std::fs::File;
use std::io::Read;

pub struct StaticFileHandler {
  file_path: String,
  mime_type: Mime
}

impl StaticFileHandler {

  pub fn new(file_path: String, mime_type: Mime) -> Self {
    StaticFileHandler { 
      file_path,
      mime_type
    }
  }

  fn read_file(&self) -> Result<Vec<u8>, ::std::io::Error> {
    let mut file = File::open(&self.file_path)?;

    let mut file_contents = Vec::new();

    file.read_to_end(&mut file_contents)?;

    Ok(file_contents)
  }

}

impl ::server::RequestHandler for StaticFileHandler {

  fn blocking(&self) -> bool { true }

  fn handle(&self, req_context: &::server::RequestContext) -> Response<Body> {
    debug!("StaticFileHandler.handle req_context = {:?}", req_context);

    match self.read_file() {
      Ok(file_contents) => {
        ::server::build_response_vec(
          StatusCode::OK,
          file_contents,
          Cow::from(self.mime_type.to_string()))
      },
      Err(_) => {
        ::server::build_response_status(StatusCode::NOT_FOUND)
      }
    }
  }

}
