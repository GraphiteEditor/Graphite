//! The stored document types of every format version, frozen by `cargo run migration snapshot` and never edited by hand.
//!
//! `v<N>` declares the types that changed in version N, plus every type reaching one, and re-exports the rest from
//! `v<N - 1>`. A type that didn't change is the same Rust type in both versions, so a format step only converts what did.
//! `latest` matches the live types in `graph-storage`, which `cargo run migration check` enforces.

pub mod v1;
pub mod v2;
pub use v2 as latest;

mod steps;
