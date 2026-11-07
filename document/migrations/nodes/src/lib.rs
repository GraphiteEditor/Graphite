//! Node migrations, in a module per node crate and a file per step, each registering itself. Scaffold one with
//! `cargo run migration new <identifier>` after bumping a node's `version(N)`.

mod blending_nodes;
mod core_types;
mod graphene_core;
mod graphene_std;
mod graphic_nodes;
mod math_nodes;
mod raster_nodes;
mod repeat_nodes;
mod transform_nodes;
mod vector_nodes;
