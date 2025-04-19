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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum BooleanOperation {
	#[default]
	#[icon("BooleanUnion")]
	Union,

	#[icon("BooleanSubtractFront")]
	SubtractFront,

	#[icon("BooleanSubtractBack")]
	SubtractBack,

	#[icon("BooleanIntersect")]
	Intersect,

	#[icon("BooleanDifference")]
	Difference,
}

pub trait ChoiceTypeStatic: Sized + Copy + AsU32 + std::fmt::Display + std::fmt::Debug + Send + Sync {
	const WIDGET_HINT: ChoiceWidgetHint;
	fn list() -> &'static [&'static [(Self, Option<&'static str>)]];
}

pub enum ChoiceWidgetHint {
	Dropdown,
	RadioButtons,
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type)]
pub enum GridType {
	#[default]
	Rectangular,
	Isometric,
}
