use core_types::{CacheHash, Ctx};
use dyn_any::DynAny;
use glam::{DVec2, IVec2, UVec2};

/// Obtains the X or Y component of a vec2.
///
/// The inverse of this node is **Combine Vec2**, which composes a vec2 from its X and Y components.
#[node_macro::node(name("Extract XY"), category("Math: Vector"))]
fn extract_xy<T: Into<DVec2>>(_: impl Ctx, #[implementations(DVec2, IVec2, UVec2)] vector: T, axis: XY) -> f64 {
	match axis {
		XY::X => vector.into().x,
		XY::Y => vector.into().y,
	}
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
