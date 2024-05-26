use dyn_any::{DynAny, StaticType};

/// Represents different ways of calculating the centroid.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum CentroidType {
	/// The center of mass for the area of a solid shape's interior, as if made out of an infinitely flat material.
	#[default]
	Area,
	/// The center of mass for the arc length of a curved shape's perimeter, as if made out of an infinitely thin wire.
	Length,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum BooleanOperation {
	#[default]
	Union,
	SubtractFront,
	SubtractBack,
	Intersect,
	Difference,
	Divide,
}

impl BooleanOperation {
	pub fn list() -> [BooleanOperation; 6] {
		[
			BooleanOperation::Union,
			BooleanOperation::SubtractFront,
			BooleanOperation::SubtractBack,
			BooleanOperation::Intersect,
			BooleanOperation::Difference,
			BooleanOperation::Divide,
		]
	}

	pub fn icons() -> [&'static str; 6] {
		["BooleanUnion", "BooleanSubtractFront", "BooleanSubtractBack", "BooleanIntersect", "BooleanDifference", "BooleanDivide"]
	}
}

impl core::fmt::Display for BooleanOperation {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			BooleanOperation::Union => write!(f, "Union"),
			BooleanOperation::SubtractFront => write!(f, "Subtract Front"),
			BooleanOperation::SubtractBack => write!(f, "Subtract Back"),
			BooleanOperation::Intersect => write!(f, "Intersect"),
			BooleanOperation::Difference => write!(f, "Difference"),
			BooleanOperation::Divide => write!(f, "Divide"),
		}
	}
}
