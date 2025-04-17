use dyn_any::DynAny;

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

impl AsU32 for BooleanOperation {
	fn as_u32(&self) -> u32 {
		*self as u32
	}
}

impl DropdownableStatic for BooleanOperation {
	fn list() -> &'static [&'static [(Self, Option<&'static str>)]] {
		&[&[
			(BooleanOperation::Union, Some("BooleanUnion")),
			(BooleanOperation::SubtractFront, Some("BooleanSubtractFront")),
			(BooleanOperation::SubtractBack, Some("BooleanSubtractBack")),
			(BooleanOperation::Intersect, Some("BooleanIntersect")),
			(BooleanOperation::Difference, Some("BooleanDifference")),
		]]
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

pub trait DropdownableStatic: Sized + Copy + AsU32 + std::fmt::Display + std::fmt::Debug + Send + Sync {
	fn list() -> &'static [&'static [(Self, Option<&'static str>)]];
}

pub trait AsU32 {
	fn as_u32(&self) -> u32;
}
impl AsU32 for u32 {
	fn as_u32(&self) -> u32 {
		*self as u32
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
