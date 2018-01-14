use chrono::prelude::Local;
use chrono::{DateTime, TimeZone};

use crypto::digest::Digest;
use crypto::sha2::Sha256;

use std::fs::File;
use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn local_time_to_string(dt: DateTime<Local>) -> String {
  dt.format("%Y-%m-%d %H:%M:%S%.9f %z").to_string()
}

pub fn system_time_to_local(st: &SystemTime) -> DateTime<Local> {
  match st.duration_since(UNIX_EPOCH) {
    Ok(dur) => {
      Local.timestamp(dur.as_secs() as i64, dur.subsec_nanos())
    },
    Err(_) => {
      Local.timestamp(0, 0)
    }
  }
}

pub fn system_time_in_seconds_u64(st: &SystemTime) -> u64 {
  match st.duration_since(UNIX_EPOCH) {
    Ok(dur) => {
      dur.as_secs()
    },
    Err(_) => 0
  }
}

pub fn duration_in_seconds_f64(duration: &Duration) -> f64 {
  (duration.as_secs() as f64) + ((duration.subsec_nanos() as f64) / 1e9)
}

pub fn file_sha256(path: String) -> Result<String, ::std::io::Error> {
  let mut file = File::open(path)?;

  let mut hasher = Sha256::new();

  let mut buffer = vec![0; 1024 * 1024];

  loop {
    let bytes_read = file.read(&mut buffer[..])?;
    match bytes_read {
      0 => break,
      _ => hasher.input(&buffer[0..bytes_read])
    }
  }

  Ok(hasher.result_str())
}
