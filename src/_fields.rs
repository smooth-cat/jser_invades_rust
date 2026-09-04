//! Field-based api declarations — every accessor trait of this crate,
//! declared once here and implemented by `#[props]` structs anywhere.

use duck_trait::fields;

// declare fields here, e.g. fields!(value, hp, can_kill)
fields! {
  hp,
  can_kill,
}