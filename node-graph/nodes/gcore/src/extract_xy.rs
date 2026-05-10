use core_types::list::Item;
use core_types::{CacheHash, Ctx};
use dyn_any::DynAny;
use glam::{DVec2, IVec2, UVec2};

/// Obtains the X or Y component of a vec2.
///
/// The inverse of this node is "Vec2 Value", which can have either or both its X and Y parameters exposed as graph inputs.
#[node_macro::node(name("Extract XY"), category("Math: Vector"))]
fn extract_xy<T: Into<DVec2>>(_: impl Ctx, #[implementations(Item<DVec2>, Item<IVec2>, Item<UVec2>)] vector: Item<T>, axis: Item<XY>) -> Item<f64> {
	let vector = vector.into_element();
	let axis = axis.into_element();
	let result = match axis {
		XY::X => vector.into().x,
		XY::Y => vector.into().y,
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
