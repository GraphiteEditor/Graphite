//! Nodes converting between plain numbers and the lengths and angles the graph carries, sizing physical units by the document's resolution.

use core_types::Ctx;
use core_types::list::Item;
use core_types::units::{Angle, AngleUnit, Length, LengthUnit};

/// Constructs a length, typed in pixels or a physical unit like millimeters or inches, which the document's resolution sizes in pixels.
#[node_macro::node(category("Value"))]
fn length_value(_: impl Ctx, _primary: (), length: Item<Length>) -> Item<Length> {
	length
}

/// Constructs an angle, typed in degrees, radians, or turns.
#[node_macro::node(category("Value"))]
fn angle_value(_: impl Ctx, _primary: (), angle: Item<Angle>) -> Item<Angle> {
	angle
}

/// Converts a number into a length, reading it in the chosen unit.
///
/// A physical unit like millimeters is sized in pixels by the document's resolution.
#[node_macro::node(category("Math: Units"))]
fn number_to_length(
	_: impl Ctx,
	/// The number to read as a length.
	number: Item<f64>,
	/// The unit the number is in.
	unit: Item<LengthUnit>,
	#[scope("pixels-per-inch")] pixels_per_inch: Item<f64>,
) -> Item<Length> {
	let (number, attributes) = number.into_parts();
	let pixels = unit.element().convert(number, LengthUnit::Pixel, *pixels_per_inch.element());

	Item::from_parts(Length::from_pixels(pixels), attributes)
}

/// Converts a length into a number, measuring it in the chosen unit.
///
/// A physical unit like millimeters is sized in pixels by the document's resolution.
#[node_macro::node(category("Math: Units"))]
fn length_to_number(
	_: impl Ctx,
	/// The length to measure.
	length: Item<Length>,
	/// The unit to measure the length in.
	unit: Item<LengthUnit>,
	#[scope("pixels-per-inch")] pixels_per_inch: Item<f64>,
) -> Item<f64> {
	let (length, attributes) = length.into_parts();
	let number = LengthUnit::Pixel.convert(length.pixels(), *unit.element(), *pixels_per_inch.element());

	Item::from_parts(number, attributes)
}

/// Converts a number into an angle, reading it in the chosen unit.
#[node_macro::node(category("Math: Units"))]
fn number_to_angle(
	_: impl Ctx,
	/// The number to read as an angle.
	number: Item<f64>,
	/// The unit the number is in.
	unit: Item<AngleUnit>,
) -> Item<Angle> {
	let (number, attributes) = number.into_parts();

	Item::from_parts(unit.element().to_angle(number), attributes)
}

/// Converts an angle into a number, measuring it in the chosen unit.
#[node_macro::node(category("Math: Units"))]
fn angle_to_number(
	_: impl Ctx,
	/// The angle to measure.
	angle: Item<Angle>,
	/// The unit to measure the angle in.
	unit: Item<AngleUnit>,
) -> Item<f64> {
	let (angle, attributes) = angle.into_parts();

	Item::from_parts(unit.element().from_angle(angle), attributes)
}
