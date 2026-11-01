//! Node migrations, in a module per node crate and a file per step, each registering itself. Scaffold one with
//! `cargo run migration new <identifier>` after bumping a node's `version(N)`.

mod graphene_core;
mod raster_nodes;

/// The scope input of nodes that came to take the GPU pipeline, the identifier frozen as it was when they did.
fn raster_kernel_pipeline() -> migration_core::NodeInput {
	migration_core::NodeInput::Scope("raster_nodes::raster_kernel::RasterKernelPipelineNode".into())
}
