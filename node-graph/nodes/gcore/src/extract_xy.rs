use core_types::Ctx;
use dyn_any::DynAny;
use glam::{DVec2, IVec2, UVec2};

#[derive(Debug, Clone, DynAny, node_macro::Destruct)]
pub struct SplitVec2Output {
	#[output(name = "X")]
	pub x: f64,
	#[output(name = "Y")]
	pub y: f64,
}

/// Obtains the X or Y component of a vec2.
///
/// The inverse of this node is "Vec2 Value", which can have either or both its X and Y parameters exposed as graph inputs.
#[node_macro::node(name("Extract XY"), category("Math: Vector"))]
fn extract_xy<T: Into<DVec2>>(_: impl Ctx, #[implementations(DVec2, IVec2, UVec2)] vector: T, axis: XY) -> f64 {
	match axis {
		XY::X => vector.into().x,
		XY::Y => vector.into().y,
	}
}

#[node_macro::node(name("Split Vec2"), category("Math: Vector"), deconstruct_output)]
fn split_vec2<T: Into<DVec2>>(_: impl Ctx, #[implementations(DVec2, IVec2, UVec2)] vector: T) -> SplitVec2Output {
	let vector = vector.into();
	SplitVec2Output { x: vector.x, y: vector.y }
}

/// The X or Y component of a vec2.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, DynAny, node_macro::ChoiceType, specta::Type, serde::Serialize, serde::Deserialize)]
#[widget(Radio)]
pub enum XY {
	#[default]
	X,
	Y,
}
