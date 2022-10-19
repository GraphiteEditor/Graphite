use crate::messages::portfolio::utility_types::Platform;

use once_cell::sync::OnceCell;

pub static GLOBAL_PLATFORM: OnceCell<Platform> = OnceCell::new();
