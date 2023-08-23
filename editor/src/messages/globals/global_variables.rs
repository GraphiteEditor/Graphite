use crate::messages::portfolio::utility_types::Platform;

use std::sync::OnceLock;

pub static GLOBAL_PLATFORM: OnceLock<Platform> = OnceLock::new();
