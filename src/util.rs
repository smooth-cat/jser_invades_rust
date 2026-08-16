use std::time::{SystemTime, UNIX_EPOCH};

pub fn system_time() -> u128 {
  let millis = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("Time went backwards")
    .as_millis();
  millis
}
