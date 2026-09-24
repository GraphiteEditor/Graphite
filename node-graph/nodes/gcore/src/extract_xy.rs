use core_types::list::Item;
use core_types::{CacheHash, Ctx};
use dyn_any::DynAny;
use glam::DVec2;

/// Obtains the X or Y component of a vec2.
///
/// The inverse of this node is **Combine Vec2**, which composes a vec2 from its X and Y components.
#[node_macro::node(name("Extract XY"), category("Math: Vec2"))]
fn extract_xy(_: impl Ctx, vector: Item<DVec2>, axis: Item<XY>) -> Item<f64> {
	let vector = vector.into_element();
	let axis = axis.into_element();

	let result = match axis {
		XY::X => vector.x,
		XY::Y => vector.y,
	};

	Item::new_from_element(result)
}

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
