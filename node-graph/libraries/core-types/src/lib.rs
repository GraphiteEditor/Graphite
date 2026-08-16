extern crate log;

pub mod arena;
pub mod attribute;
pub mod bounds;
pub mod consts;
pub mod context;
pub mod extent;
pub mod frame_table;
pub mod gpoll;
pub mod list;
pub mod math;
pub mod memo;
pub mod misc;
pub mod node;
pub mod ops;
pub mod record;
pub mod registry;
pub mod render_complexity;
pub mod runtime;
pub mod transform;
pub mod uuid;
pub mod value;

pub use crate as core_types;
pub use blending::*;
pub use color::Color;
pub use context::*;
pub use ctor;
pub use dyn_any::{StaticTypeSized, WasmNotSend, WasmNotSync};
pub use graphene_hash;
pub use graphene_hash::CacheHash;
pub use list::{
	ATTR_BACKGROUND, ATTR_BLEND_MODE, ATTR_CLIP, ATTR_CLIPPING_MASK, ATTR_DIMENSIONS, ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_EDITOR_TEXT_FRAME, ATTR_END,
	ATTR_FONT, ATTR_FONT_SIZE, ATTR_GRADIENT_TYPE, ATTR_LETTER_SPACING, ATTR_LETTER_TILT, ATTR_LINE_HEIGHT, ATTR_LOCATION, ATTR_MAX_HEIGHT, ATTR_MAX_WIDTH, ATTR_NAME, ATTR_OPACITY, ATTR_OPACITY_FILL,
	ATTR_SPREAD_METHOD, ATTR_START, ATTR_TEXT_ALIGN, ATTR_TRANSFORM, ATTR_TYPE,
};
pub use memo::MemoHash;
pub use no_std_types::AsU32;
pub use no_std_types::blending;
pub use no_std_types::choice_type;
pub use no_std_types::color;
pub use no_std_types::shaders;
pub use node::Node;
pub use num_traits;
#[cfg(feature = "wasm")]
pub use tsify;
pub use types::Cow;

mod types;
pub use types::*;

pub trait InputAccessorSource<'a, T>: InputAccessorSourceIdentifier + std::fmt::Debug {
	fn get_input(&'a self, index: usize) -> Option<&'a T>;
	fn set_input(&'a mut self, index: usize, value: T);
}

pub trait InputAccessorSourceIdentifier {
	fn has_identifier(&self, identifier: &str) -> bool;
}

pub trait InputAccessor<'n, Source: 'n>
where
	Self: Sized,
{
	fn new_with_source(source: &'n Source) -> Option<Self>;
}

pub trait NodeInputDecleration {
	const INDEX: usize;
	fn identifier() -> ProtoNodeIdentifier;
	type Result;
}
