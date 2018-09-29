use chrono::prelude::{DateTime, Local, TimeZone, Utc};

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

pub fn system_time_to_utc(st: &SystemTime) -> DateTime<Utc> {
  match st.duration_since(UNIX_EPOCH) {
    Ok(dur) => {
      Utc.timestamp(dur.as_secs() as i64, dur.subsec_nanos())
    },
    Err(_) => {
      Utc.timestamp(0, 0)
    }
  }
}

pub fn duration_in_seconds_f64(duration: &Duration) -> f64 {
  (duration.as_secs() as f64) + ((duration.subsec_nanos() as f64) / 1e9)
}
