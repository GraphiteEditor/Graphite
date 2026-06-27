//! Integration tests for the `.gdd` document-storage layer, split by altitude:
//! - [`metadata_tests`] drives a demo document through the conversion bridge in-process (no save/reopen).
//! - [`round_trip_tests`] drives real editor edits through a full `Gdd` save/reopen pipeline.
//!
//! Shared fixtures live in [`test_support`].

mod metadata_tests;
mod round_trip_tests;
mod test_support;
