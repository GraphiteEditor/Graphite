use crate::messages::portfolio::document::utility_types::misc::Platform;

use once_cell::sync::OnceCell;

pub static GLOBAL_PLATFORM: OnceCell<Platform> = OnceCell::new();
