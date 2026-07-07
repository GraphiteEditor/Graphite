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
#[node_macro::destructure]
#[derive(Debug, Clone, Copy, PartialEq, DynAny)]
pub struct Vec2Components {
	/// The X component of the vec2.
	pub x: f64,
	/// The Y component of the vec2.
	pub y: f64,
}

/// Decomposes the X and Y components of a vec2.
///
/// The inverse of this node is **Combine Vec2**, which composes a vec2 from its X and Y components.
#[node_macro::node(name("Split Vec2"), category("Math: Vec2"))]
fn split_vec2(_: impl Ctx, #[name("Vec2")] vec2: Item<DVec2>) -> Item<Vec2Components> {
	let vec2 = vec2.into_element();

	Item::new_from_element(Vec2Components { x: vec2.x, y: vec2.y })
}
