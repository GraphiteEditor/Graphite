// Bumped past the default 128 because the deeply-generic message-passing types pull in wgpu/naga
// trait chains that overflow the trait resolver under `--tests`. Set to the same value the compiler suggests.
#![recursion_limit = "256"]

extern crate graphite_proc_macros;

// `macro_use` puts these macros into scope for all descendant code files
#[macro_use]
mod macros;
#[macro_use]
extern crate log;

pub mod application;
pub mod consts;
pub mod dispatcher;
pub mod messages;
pub mod node_graph_executor;
#[cfg(test)]
pub mod test_utils;
pub mod utility_traits;
pub mod utility_types;
