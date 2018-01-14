use mime::Mime;

use hyper::header;
use hyper::server::Response;
use hyper::StatusCode;

use std::fs;
use std::fs::File;
use std::io::Read;

pub struct StaticFileHandler {
  file_path: String,
  mime_type: Mime,
  cache_max_age_seconds: u32
}

impl StaticFileHandler {

  pub fn new(file_path: String, mime_type: Mime, cache_max_age_seconds: u32) -> Self {
    StaticFileHandler { 
      file_path: file_path,
      mime_type: mime_type,
      cache_max_age_seconds: cache_max_age_seconds
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

  fn use_worker_threadpool(&self) -> bool { true }

  fn handle(&self, req_context: &::server::RequestContext) -> Response {
    debug!("StaticFileHandler.handle req_context = {:?}", req_context);

    let file_metadata =
      match fs::metadata(&self.file_path) {
        Ok(metadata) => metadata,
        Err(_) => return ::server::build_response_status(StatusCode::NotFound)
      };

    let file_modified =
      match file_metadata.modified() {
        Ok(file_modified) => file_modified,
        Err(_) => return ::server::build_response_status(StatusCode::NotFound)
      };

    if let Some(response) = ::server::handle_not_modified(
      &req_context,
      &file_modified,
      self.cache_max_age_seconds) {
      return response;
    }

    match self.read_file() {
      Ok(file_contents) => {
        ::server::build_response_vec(
          StatusCode::Ok,
          file_contents,
          header::ContentType(self.mime_type.clone()))
          .with_header(header::LastModified(file_modified.into()))
          .with_header(header::CacheControl(
             vec![header::CacheDirective::Public,
                  header::CacheDirective::MaxAge(self.cache_max_age_seconds)]))
      },
      Err(_) => {
        ::server::build_response_status(StatusCode::NotFound)
      }
    }
  }

}
