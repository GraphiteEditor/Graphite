//! Node migrations, in a module per node crate and a file per step, each registering itself. Scaffold one with
//! `cargo run migration new <identifier>` after bumping a node's `version(N)`.

mod raster_nodes;
