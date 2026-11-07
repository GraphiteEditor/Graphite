//! Lengths and angles as the node graph carries them, and the units an input remembers its value was typed in.

use dyn_any::DynAny;
use math_parser::quantity::Quantity;
use math_parser::units::Unit;
use math_parser::value::Value;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// The pixels per inch of a document that sets none of its own, CSS's reference pixel, which every SVG reader also assumes.
pub const DEFAULT_PIXELS_PER_INCH: f64 = math_parser::units::DEFAULT_PIXELS_PER_INCH;

/// The scope key under which the graph's runtime injects the document's pixels per inch.
pub const PIXELS_PER_INCH_SCOPE: &str = "pixels-per-inch";

/// A distance across the canvas, measured in pixels: the one type a length travels the node graph as, whatever unit it was typed in.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, DynAny, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(transparent))]
pub struct Length(f64);

impl Length {
	pub const ZERO: Self = Self(0.);

	pub const fn from_pixels(pixels: f64) -> Self {
		Self(pixels)
	}

	pub const fn pixels(self) -> f64 {
		self.0
	}

	pub fn abs(self) -> Self {
		Self(self.0.abs())
	}

	pub fn min(self, other: Self) -> Self {
		Self(self.0.min(other.0))
	}

	pub fn max(self, other: Self) -> Self {
		Self(self.0.max(other.0))
	}
}

/// An angle, which node bodies read in radians or degrees as their math needs.
/// It's held in degrees, so sums and comparisons of whole degrees stay exact.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, DynAny, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(transparent))]
pub struct Angle(f64);

impl Angle {
	pub const ZERO: Self = Self(0.);

	pub const fn from_radians(radians: f64) -> Self {
		Self(radians.to_degrees())
	}

	pub const fn from_degrees(degrees: f64) -> Self {
		Self(degrees)
	}

	pub const fn radians(self) -> f64 {
		self.0.to_radians()
	}

	pub const fn degrees(self) -> f64 {
		self.0
	}

	pub fn abs(self) -> Self {
		Self(self.0.abs())
	}

	pub fn min(self, other: Self) -> Self {
		Self(self.0.min(other.0))
	}

	pub fn max(self, other: Self) -> Self {
		Self(self.0.max(other.0))
	}
}

/// Arithmetic that keeps a measurement's meaning: sums and differences of like measurements, scaling by a plain number, and the
/// plain ratio of two like measurements.
macro_rules! measurement_arithmetic {
	($($measurement:ident),*) => {
		$(
			impl Add for $measurement {
				type Output = Self;
				fn add(self, other: Self) -> Self {
					Self(self.0 + other.0)
				}
			}

			impl Sub for $measurement {
				type Output = Self;
				fn sub(self, other: Self) -> Self {
					Self(self.0 - other.0)
				}
			}

			impl AddAssign for $measurement {
				fn add_assign(&mut self, other: Self) {
					self.0 += other.0;
				}
			}

			impl SubAssign for $measurement {
				fn sub_assign(&mut self, other: Self) {
					self.0 -= other.0;
				}
			}

			impl Neg for $measurement {
				type Output = Self;
				fn neg(self) -> Self {
					Self(-self.0)
				}
			}

			impl Mul<f64> for $measurement {
				type Output = Self;
				fn mul(self, factor: f64) -> Self {
					Self(self.0 * factor)
				}
			}

			impl Mul<$measurement> for f64 {
				type Output = $measurement;
				fn mul(self, measurement: $measurement) -> $measurement {
					$measurement(self * measurement.0)
				}
			}

			impl Div<f64> for $measurement {
				type Output = Self;
				fn div(self, divisor: f64) -> Self {
					Self(self.0 / divisor)
				}
			}

			impl Div for $measurement {
				type Output = f64;
				fn div(self, divisor: Self) -> f64 {
					self.0 / divisor.0
				}
			}

			impl Sum for $measurement {
				fn sum<I: Iterator<Item = Self>>(iterator: I) -> Self {
					Self(iterator.map(|measurement| measurement.0).sum())
				}
			}
		)*
	};
}
measurement_arithmetic!(Length, Angle);

impl std::fmt::Display for Length {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} px", self.0)
	}
}

impl std::fmt::Display for Angle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}°", self.degrees())
	}
}

/// A unit a length can be typed in.
#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum LengthUnit {
	/// A pixel of the canvas, which the document's resolution sizes against an inch.
	#[default]
	Pixel,
	/// A typographic point, 1/72 of an inch.
	Point,
	/// A typographic pica, 12 points or 1/6 of an inch.
	Pica,
	/// An inch, 25.4 millimeters.
	Inch,
	/// A foot, 12 inches.
	Foot,
	/// A yard, 3 feet.
	Yard,
	/// A mile, 5280 feet.
	Mile,
	/// A thou, 1/1000 of an inch.
	Thou,
	/// A micrometer, 1/1000 of a millimeter.
	Micrometer,
	/// A millimeter.
	Millimeter,
	/// A centimeter, 10 millimeters.
	Centimeter,
	/// A meter, 100 centimeters.
	Meter,
	/// A kilometer, 1000 meters.
	Kilometer,
}

impl LengthUnit {
	pub const ALL: [Self; 13] = [
		Self::Pixel,
		Self::Point,
		Self::Pica,
		Self::Inch,
		Self::Foot,
		Self::Yard,
		Self::Mile,
		Self::Thou,
		Self::Micrometer,
		Self::Millimeter,
		Self::Centimeter,
		Self::Meter,
		Self::Kilometer,
	];

	/// The unit's symbol as the expression language spells it, like `mm`.
	pub fn symbol(self) -> &'static str {
		match self {
			Self::Pixel => "px",
			Self::Point => "pt",
			Self::Pica => "pc",
			Self::Inch => "in",
			Self::Foot => "ft",
			Self::Yard => "yd",
			Self::Mile => "mi",
			Self::Thou => "thou",
			Self::Micrometer => "µm",
			Self::Millimeter => "mm",
			Self::Centimeter => "cm",
			Self::Meter => "m",
			Self::Kilometer => "km",
		}
	}

	/// The unit a math expression's unit is, if it is one of these.
	pub fn from_unit(unit: Unit) -> Option<Self> {
		Self::ALL.into_iter().find(|candidate| candidate.unit() == unit)
	}

	/// The expression language's unit of the same size.
	pub fn unit(self) -> Unit {
		Unit::named(self.symbol()).unwrap_or(Unit::NONE)
	}

	/// Whether the unit is a physical size, which converts to pixels by the document's resolution.
	pub fn is_physical(self) -> bool {
		self != Self::Pixel
	}

	/// Converts a number in this unit into another, exactly where the ratio between them allows.
	pub fn convert(self, value: f64, into: Self, pixels_per_inch: f64) -> f64 {
		if self == into {
			return value;
		}
		Quantity::new(Value::from_f64(value), self.unit())
			.convert(into.unit(), pixels_per_inch)
			.ok()
			.and_then(|converted| converted.as_real())
			.unwrap_or(value)
	}
}

/// A unit an angle can be typed in.
#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum AngleUnit {
	/// A degree, 1/360 of a turn.
	#[default]
	Degree,
	/// A radian, the angle of an arc as long as its radius, 1/(2π) of a turn.
	Radian,
	/// A whole turn around the circle.
	Turn,
}

impl AngleUnit {
	pub const ALL: [Self; 3] = [Self::Degree, Self::Radian, Self::Turn];

	/// The unit's symbol as the expression language spells it, like `°`.
	pub fn symbol(self) -> &'static str {
		match self {
			Self::Degree => "°",
			Self::Radian => "rad",
			Self::Turn => "turn",
		}
	}

	pub fn from_unit(unit: Unit) -> Option<Self> {
		Self::ALL.into_iter().find(|candidate| candidate.unit() == unit)
	}

	pub fn unit(self) -> Unit {
		Unit::named(self.symbol()).unwrap_or(Unit::NONE)
	}

	pub fn to_angle(self, value: f64) -> Angle {
		match self {
			Self::Degree => Angle::from_degrees(value),
			Self::Radian => Angle::from_radians(value),
			Self::Turn => Angle::from_degrees(value * 360.),
		}
	}

	pub fn from_angle(self, angle: Angle) -> f64 {
		match self {
			Self::Degree => angle.degrees(),
			Self::Radian => angle.radians(),
			Self::Turn => angle.degrees() / 360.,
		}
	}

	/// A number of degrees measured in this unit, which stays exact in degrees rather than round-tripping through radians.
	pub fn from_degrees(self, degrees: f64) -> f64 {
		match self {
			Self::Degree => degrees,
			_ => self.from_angle(Angle::from_degrees(degrees)),
		}
	}
}

/// A length as typed into an input, kept in its unit so a physical size stays physical when the document's resolution changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, DynAny, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypedLength {
	pub value: f64,
	pub unit: LengthUnit,
}

impl TypedLength {
	pub const fn pixels(pixels: f64) -> Self {
		Self {
			value: pixels,
			unit: LengthUnit::Pixel,
		}
	}

	pub fn to_length(self, pixels_per_inch: f64) -> Length {
		Length::from_pixels(self.unit.convert(self.value, LengthUnit::Pixel, pixels_per_inch))
	}
}

/// An angle as typed into an input, kept in its unit for display.
#[derive(Clone, Copy, Debug, Default, PartialEq, DynAny, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypedAngle {
	pub value: f64,
	pub unit: AngleUnit,
}

impl TypedAngle {
	pub const fn degrees(degrees: f64) -> Self {
		Self {
			value: degrees,
			unit: AngleUnit::Degree,
		}
	}

	pub fn to_angle(self) -> Angle {
		self.unit.to_angle(self.value)
	}

	/// The angle in degrees, exactly as typed when it was typed in degrees.
	pub fn to_degrees(self) -> f64 {
		match self.unit {
			AngleUnit::Degree => self.value,
			_ => self.to_angle().degrees(),
		}
	}
}

// A plain number wired into a length or angle reads in the unit its field shows, pixels or degrees
impl crate::ops::Convert<Length, ()> for f64 {
	fn convert(self, _: crate::transform::Footprint, _converter: ()) -> Length {
		Length::from_pixels(self)
	}
}

impl crate::ops::Convert<Length, ()> for i64 {
	fn convert(self, _: crate::transform::Footprint, _converter: ()) -> Length {
		Length::from_pixels(self as f64)
	}
}

impl crate::ops::Convert<Angle, ()> for f64 {
	fn convert(self, _: crate::transform::Footprint, _converter: ()) -> Angle {
		Angle::from_degrees(self)
	}
}

impl crate::ops::Convert<Angle, ()> for i64 {
	fn convert(self, _: crate::transform::Footprint, _converter: ()) -> Angle {
		Angle::from_degrees(self as f64)
	}
}

impl crate::ops::Convert<String, ()> for Length {
	fn convert(self, _: crate::transform::Footprint, _converter: ()) -> String {
		format!("{} px", crate::math::float_noise::round_away_float_noise(self.pixels()))
	}
}

impl crate::ops::Convert<String, ()> for Angle {
	fn convert(self, _: crate::transform::Footprint, _converter: ()) -> String {
		format!("{}°", crate::math::float_noise::round_away_float_noise(self.degrees()))
	}
}

// A length's bounds are in pixels and an angle's in degrees, the units their fields show
impl crate::misc::Clampable for Length {
	fn clamp_hard_min(self, min: f64) -> Self {
		Self(self.0.max(min))
	}
	fn clamp_hard_max(self, max: f64) -> Self {
		Self(self.0.min(max))
	}
}

impl crate::misc::Clampable for Angle {
	fn clamp_hard_min(self, min: f64) -> Self {
		Self(self.0.max(min))
	}
	fn clamp_hard_max(self, max: f64) -> Self {
		Self(self.0.min(max))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_unit_is_one_of_the_expression_language() {
		for unit in LengthUnit::ALL {
			assert_eq!(LengthUnit::from_unit(unit.unit()), Some(unit), "`{}`", unit.symbol());
			assert_eq!(unit.unit().dimension(), math_parser::units::Dimension::LENGTH);
		}
		for unit in AngleUnit::ALL {
			assert_eq!(AngleUnit::from_unit(unit.unit()), Some(unit), "`{}`", unit.symbol());
		}
	}

	#[test]
	fn typed_lengths_convert_to_pixels_by_the_resolution() {
		assert_eq!(TypedLength { value: 1., unit: LengthUnit::Inch }.to_length(96.), Length::from_pixels(96.));
		assert_eq!(TypedLength { value: 1., unit: LengthUnit::Inch }.to_length(300.), Length::from_pixels(300.));
		assert_eq!(TypedLength { value: 72., unit: LengthUnit::Point }.to_length(96.), Length::from_pixels(96.));
		assert_eq!(TypedLength::pixels(12.).to_length(300.), Length::from_pixels(12.));
		let millimeters = TypedLength {
			value: 25.4,
			unit: LengthUnit::Millimeter,
		}
		.to_length(96.)
		.pixels();
		assert!((millimeters - 96.).abs() < 1e-9);
	}

	#[test]
	fn angles_read_in_any_unit() {
		assert!((TypedAngle::degrees(90.).to_angle().radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
		assert!((AngleUnit::Turn.to_angle(0.25).degrees() - 90.).abs() < 1e-12);
		assert!((AngleUnit::Turn.from_angle(Angle::from_degrees(180.)) - 0.5).abs() < 1e-12);
	}

	// Arithmetic on whole degrees compares equal to the degrees it should give
	#[test]
	fn whole_degrees_stay_exact_through_arithmetic() {
		assert_eq!(Angle::from_degrees(1.) + Angle::from_degrees(5.), Angle::from_degrees(6.));
		assert_eq!(AngleUnit::Turn.to_angle(0.25), Angle::from_degrees(90.));
	}

	// 15° doesn't survive the trip through radians bit for bit
	#[test]
	fn degrees_stay_exact_in_degrees() {
		assert_eq!(TypedAngle::degrees(15.).to_degrees(), 15.);
		assert_eq!(AngleUnit::Degree.from_degrees(15.), 15.);
		assert!((AngleUnit::Turn.from_degrees(90.) - 0.25).abs() < 1e-12);
	}
}
