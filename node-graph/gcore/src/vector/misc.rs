use dyn_any::DynAny;
use glam::DVec2;
use kurbo::Point;

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
}

impl BooleanOperation {
	pub fn list() -> [BooleanOperation; 5] {
		[
			BooleanOperation::Union,
			BooleanOperation::SubtractFront,
			BooleanOperation::SubtractBack,
			BooleanOperation::Intersect,
			BooleanOperation::Difference,
		]
	}

	pub fn icons() -> [&'static str; 5] {
		["BooleanUnion", "BooleanSubtractFront", "BooleanSubtractBack", "BooleanIntersect", "BooleanDifference"]
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
		}
	}
}

pub trait AsU64 {
	fn as_u64(&self) -> u64;
}
impl AsU64 for u32 {
	fn as_u64(&self) -> u64 {
		*self as u64
	}
}
impl AsU64 for u64 {
	fn as_u64(&self) -> u64 {
		*self
	}
}
impl AsU64 for f64 {
	fn as_u64(&self) -> u64 {
		*self as u64
	}
}

pub trait AsI64 {
	fn as_i64(&self) -> i64;
}
impl AsI64 for u32 {
	fn as_i64(&self) -> i64 {
		*self as i64
	}
}
impl AsI64 for u64 {
	fn as_i64(&self) -> i64 {
		*self as i64
	}
}
impl AsI64 for f64 {
	fn as_i64(&self) -> i64 {
		*self as i64
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum GridType {
	#[default]
	Rectangular,
	Isometric,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum ArcType {
	#[default]
	Open,
	Closed,
	PieSlice,
}

pub fn point_to_dvec2(point: Point) -> DVec2 {
	DVec2 { x: point.x, y: point.y }
}
