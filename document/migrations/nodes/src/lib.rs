//! Node migrations, in a module per node crate and a file per step, each registering itself. Scaffold one with
//! `cargo run migration new <identifier>` after bumping a node's `version(N)`.

mod blending_nodes;
mod graphene_core;
mod graphene_std;
mod math_nodes;
mod raster_nodes;
mod vector_nodes;
