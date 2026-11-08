//! Nodes converting between plain numbers and the lengths and angles the graph carries, sizing physical units by the document's resolution.

use core_types::Ctx;
use core_types::list::Item;
use core_types::units::{Angle, AngleUnit, AngleVec2, Length, LengthUnit, LengthVec2};
use glam::DVec2;

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

/// Converts a Vec2 into a pair of lengths, reading both components in the chosen unit.
///
/// A physical unit like millimeters is sized in pixels by the document's resolution.
#[node_macro::node(category("Math: Units"), name("Vec2 to Length Vec2"))]
fn vec2_to_length_vec2(
	_: impl Ctx,
	/// The Vec2 to read as a pair of lengths.
	#[name("Vec2")]
	vec2: Item<DVec2>,
	/// The unit both components are in.
	unit: Item<LengthUnit>,
	#[scope("pixels-per-inch")] pixels_per_inch: Item<f64>,
) -> Item<LengthVec2> {
	let (vec2, attributes) = vec2.into_parts();
	let (unit, pixels_per_inch) = (*unit.element(), *pixels_per_inch.element());
	let pixels = DVec2::new(unit.convert(vec2.x, LengthUnit::Pixel, pixels_per_inch), unit.convert(vec2.y, LengthUnit::Pixel, pixels_per_inch));

	Item::from_parts(LengthVec2::from_pixels(pixels), attributes)
}

/// Converts a pair of lengths into a Vec2, measuring both components in the chosen unit.
///
/// A physical unit like millimeters is sized in pixels by the document's resolution.
#[node_macro::node(category("Math: Units"), name("Length Vec2 to Vec2"))]
fn length_vec2_to_vec2(
	_: impl Ctx,
	/// The pair of lengths to measure.
	#[name("Length Vec2")]
	lengths: Item<LengthVec2>,
	/// The unit to measure both components in.
	unit: Item<LengthUnit>,
	#[scope("pixels-per-inch")] pixels_per_inch: Item<f64>,
) -> Item<DVec2> {
	let (lengths, attributes) = lengths.into_parts();
	let (unit, pixels_per_inch) = (*unit.element(), *pixels_per_inch.element());
	let pixels = lengths.pixels();
	let vec2 = DVec2::new(LengthUnit::Pixel.convert(pixels.x, unit, pixels_per_inch), LengthUnit::Pixel.convert(pixels.y, unit, pixels_per_inch));

	Item::from_parts(vec2, attributes)
}

/// Converts a Vec2 into a pair of angles, reading both components in the chosen unit.
#[node_macro::node(category("Math: Units"), name("Vec2 to Angle Vec2"))]
fn vec2_to_angle_vec2(
	_: impl Ctx,
	/// The Vec2 to read as a pair of angles.
	#[name("Vec2")]
	vec2: Item<DVec2>,
	/// The unit both components are in.
	unit: Item<AngleUnit>,
) -> Item<AngleVec2> {
	let (vec2, attributes) = vec2.into_parts();
	let unit = *unit.element();
	let degrees = DVec2::new(unit.to_angle(vec2.x).degrees(), unit.to_angle(vec2.y).degrees());

	Item::from_parts(AngleVec2::from_degrees(degrees), attributes)
}

/// Converts a pair of angles into a Vec2, measuring both components in the chosen unit.
#[node_macro::node(category("Math: Units"), name("Angle Vec2 to Vec2"))]
fn angle_vec2_to_vec2(
	_: impl Ctx,
	/// The pair of angles to measure.
	#[name("Angle Vec2")]
	angles: Item<AngleVec2>,
	/// The unit to measure both components in.
	unit: Item<AngleUnit>,
) -> Item<DVec2> {
	let (angles, attributes) = angles.into_parts();
	let unit = *unit.element();
	let vec2 = DVec2::new(unit.from_angle(angles.x()), unit.from_angle(angles.y()));

	Item::from_parts(vec2, attributes)
}
