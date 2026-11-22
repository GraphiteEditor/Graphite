//! Integration tests for the `.gdd` document-storage layer, split by altitude:
//! - [`metadata_tests`] drives a demo document through the conversion bridge in-process (no save/reopen).
//! - [`round_trip_tests`] drives real editor edits through a full `Gdd` save/reopen pipeline.
//!
//! Shared fixtures live in [`test_support`].

mod drag_stability_tests;
mod layout_pass_tests;
mod lifecycle_tests;
mod metadata_tests;
mod relative_positions_tests;
mod round_trip_tests;
pub(crate) mod test_support;
