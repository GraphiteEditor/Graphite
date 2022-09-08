extern crate graphite_proc_macros;

// `macro_use` puts these macros into scope for all descendant code files
#[macro_use]
mod macros;

// `macro_use` puts the log macros (`error!`, `warn!`, `debug!`, `info!` and `trace!`) in scope for the crate
#[macro_use]
extern crate log;

pub mod application;
pub mod consts;
pub mod dispatcher;
pub mod messages;
pub mod test_utils;
pub mod utility_traits;
