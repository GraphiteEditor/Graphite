use core_types::list::Item;
use core_types::{CacheHash, Ctx};
use dyn_any::DynAny;
use glam::DVec2;

/// The X or Y component of a vec2.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum XY {
	#[default]
	X,
	Y,
}

/// The X and Y components of a vec2, split into separate node outputs.
#[derive(Debug, Clone, PartialEq, DynAny, node_macro::Destructure)]
pub struct Vec2Components {
	/// The X component of the vec2.
	pub x: Item<f64>,
	/// The Y component of the vec2.
	pub y: Item<f64>,
}

/// Decomposes the X and Y components of a vec2.
///
/// The inverse of this node is **Combine Vec2**, which composes a vec2 from its X and Y components.
#[node_macro::node(name("Split Vec2"), category("Math: Vec2"), destructure_output)]
fn split_vec2(_: impl Ctx, #[name("Vec2")] vec2: Item<DVec2>) -> Vec2Components {
	let (vec2, attributes) = vec2.into_parts();

	Vec2Components {
		x: Item::from_parts(vec2.x, attributes.clone()),
		y: Item::from_parts(vec2.y, attributes),
	}
}
