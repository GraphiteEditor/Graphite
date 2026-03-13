pub mod any;
pub mod render_cache;
pub mod render_node;
pub mod text;
#[cfg(feature = "wasm")]
pub mod wasm_application_io;
pub use blending_nodes;
pub use brush_nodes as brush;
pub use core_types::*;
pub use graphene_application_io as application_io;
pub use graphene_core;
pub use graphene_core::debug;
pub use graphic_nodes;
pub use graphic_types::{Artboard, Graphic, Vector};
pub use math_nodes;
pub use path_bool_nodes as path_bool;
pub use raster_nodes;
pub use repeat_nodes;
pub use text_nodes;
pub use transform_nodes;
pub use vector_nodes;
pub use vector_types;

/// Backward compatibility re-exports
pub mod vector {
	pub use graphic_types::Vector;
	pub use vector_types::vector::{VectorModification, VectorModificationType, misc, style};
	pub use vector_types::*;

	// Re-export commonly used types and submodules
	pub use vector_types::vector::algorithms;
	pub use vector_types::vector::click_target;
	pub use vector_types::vector::misc::HandleId;
	pub use vector_types::vector::{PointId, RegionId, SegmentId, StrokeId};
	pub use vector_types::vector::{deserialize_hashmap, serialize_hashmap};

	// Re-export HandleExt trait and NoHashBuilder
	pub use vector_types::vector::HandleExt;
	pub use vector_types::vector::NoHashBuilder;

	// Re-export vector node modules and functions
	pub use vector_nodes::*;
}

pub mod graphic {
	pub use graphic_nodes::graphic::*;
	pub use graphic_types::Artboard;
	pub use graphic_types::graphic::*;
}

pub mod artboard {
	pub use graphic_nodes::artboard::*;
	pub use graphic_types::artboard::*;
}

pub mod subpath {
	pub use vector_types::subpath::*;
}

pub mod gradient {
	pub use vector_types::{GradientStop, GradientStops};
}

pub mod transform {
	pub use core_types::transform::*;
	pub use vector_types::ReferencePoint;
}

pub mod repeat {
	pub use repeat_nodes::repeat_nodes::*;
}

pub mod math {
	pub use core_types::math::quad;

	pub mod math_ext {
		pub use vector_types::{QuadExt, RectExt};
	}
}

pub mod logic {
	pub use graphene_core::logic::*;
}

pub mod context {
	pub use graphene_core::context::*;
}

// Re-export graphene_core modules for backward compatibility
pub mod ops {
	pub use core_types::ops::*;
	pub use graphene_core::ops::*;
}

pub mod extract_xy {
	pub use graphene_core::extract_xy::*;
}

pub mod animation {
	pub use graphene_core::animation::*;
}

/// stop gap solutions until all paths have been replaced with their absolute ones
pub mod renderer {
	pub use core_types::math::quad::Quad;
	pub use core_types::math::rect::Rect;
	pub use rendering::*;
}

pub mod raster {
	pub use graphic_types::raster_types::*;
	pub use raster_nodes::adjustments::*;
	pub use raster_nodes::*;
}

pub mod raster_types {
	pub use graphic_types::raster_types::*;
}

pub mod memo {
	pub use core_types::memo::*;
	pub use graphene_core::memo::*;
}
