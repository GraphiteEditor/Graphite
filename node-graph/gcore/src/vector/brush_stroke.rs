use dyn_any::{DynAny, StaticType};
use glam::DVec2;
use std::hash::{Hash, Hasher};

use crate::Color;

/// The style of a brush.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushStyle {
	pub color: Color,
	pub diameter: f64,
	pub hardness: f64,
	pub flow: f64,
}

impl Default for BrushStyle {
	fn default() -> Self {
		Self {
			color: Color::BLACK,
			diameter: 40.0,
			hardness: 50.0,
			flow: 100.0,
		}
	}
}

impl Hash for BrushStyle {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.color.hash(state);
		self.diameter.to_bits().hash(state);
		self.hardness.to_bits().hash(state);
		self.flow.to_bits().hash(state);
	}
}

/// A single sample of brush parameters across the brush stroke.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushInputSample {
	pub position: DVec2,
	// Future work: pressure, stylus angle, etc.
}

impl Hash for BrushInputSample {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.position.x.to_bits().hash(state);
		self.position.y.to_bits().hash(state);
	}
}

/// The parameters for a single stroke brush.
#[derive(Clone, Debug, PartialEq, Hash, Default, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushStroke {
	pub style: BrushStyle,
	pub trace: Vec<BrushInputSample>,
}
