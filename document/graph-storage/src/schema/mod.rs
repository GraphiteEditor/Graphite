//! Every type a document stores, and the trait impls its encoding depends on. Behavior lives with the rest of the crate.
//!
//! `cargo run migration snapshot` freezes this module into `migration-formats` as one format version, and
//! `cargo run migration check` fails when it no longer matches the latest snapshot. Keep it to definitions and the impls
//! serde or the derives need, so a change here is a change to the format.

mod attributes;
mod delta;
mod ids;
mod model;
mod prior;
mod registry;
mod resources;
mod session;
mod session_state;
mod value;

pub use attributes::*;
pub use delta::*;
pub use ids::*;
pub use model::*;
pub use prior::*;
pub use registry::*;
pub use resources::*;
pub use session::*;
pub use session_state::*;
pub use value::*;
