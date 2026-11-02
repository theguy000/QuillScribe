//! Behaviour of one utterance's delivery, against a fake platform.
//!
//! Each part has its own file: the fakes, the delivery rules, copy mode, the
//! verdict rules, and what the platform reports. They are compiled together so
//! a test can use another file's helper.

use super::*;

#[path = "tests/copy_mode.rs"]
mod copy_mode;
#[path = "tests/core.rs"]
mod core;
#[path = "tests/fixtures.rs"]
mod fixtures;
#[path = "tests/platform_readings.rs"]
mod platform_readings;
#[path = "tests/target_rules.rs"]
mod target_rules;
