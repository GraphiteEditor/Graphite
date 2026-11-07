//! Units of measure: the units the language knows, the products of them a value is measured in, and the exact conversions
//! between units of one dimension.

use crate::ast::BinaryOp;
use crate::value::Number;
use std::f64::consts::PI;
use std::fmt;

/// The pixels per inch a host supplies when it gives no other, CSS's reference pixel.
pub const DEFAULT_PIXELS_PER_INCH: f64 = 96.;

/// The exponents of length, mass, time, and angle that a unit or a value measures, so a speed is `[1, 0, -1, 0]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Dimension(pub [i16; 4]);

impl Dimension {
	pub const NONE: Self = Self([0; 4]);
	pub const LENGTH: Self = Self([1, 0, 0, 0]);
	pub const MASS: Self = Self([0, 1, 0, 0]);
	pub const TIME: Self = Self([0, 0, 1, 0]);
	pub const ANGLE: Self = Self([0, 0, 0, 1]);

	const AREA: Self = Self([2, 0, 0, 0]);
	const VOLUME: Self = Self([3, 0, 0, 0]);
	const FREQUENCY: Self = Self([0, 0, -1, 0]);
	const SPEED: Self = Self([1, 0, -1, 0]);
	const ACCELERATION: Self = Self([1, 0, -2, 0]);
	const ANGULAR_SPEED: Self = Self([0, 0, -1, 1]);

	pub fn is_none(self) -> bool {
		self == Self::NONE
	}

	/// Whether the two share a base dimension, like an angular speed and a time, so a product of them can cancel it.
	fn overlaps(self, other: Self) -> bool {
		self.0.iter().zip(other.0).any(|(a, b)| *a != 0 && b != 0)
	}

	/// What this dimension is called, like `length` or `speed`.
	pub fn noun(self) -> &'static str {
		let description = self.description();
		description.split_once(' ').map_or(description, |(_, noun)| noun)
	}

	/// What a value of this dimension is called, for error messages.
	pub fn description(self) -> &'static str {
		match self {
			Self::NONE => "a plain number",
			Self::LENGTH => "a length",
			Self::AREA => "an area",
			Self::VOLUME => "a volume",
			Self::MASS => "a mass",
			Self::TIME => "a duration",
			Self::ANGLE => "an angle",
			Self::FREQUENCY => "a frequency",
			Self::SPEED => "a speed",
			Self::ACCELERATION => "an acceleration",
			Self::ANGULAR_SPEED => "an angular speed",
			_ => "a compound quantity",
		}
	}
}

/// A size as an exact ratio of integers, times a power of π (since degrees and turns are fractions of radians' half and
/// whole turn) and a power of the inch per pixel (since a pixel is an inch over the host's pixels per inch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Size {
	numerator: i128,
	denominator: i128,
	pi: i8,
	/// The power of the host's pixels per inch the size divides by.
	pixel: i8,
}

const fn greatest_common_divisor(mut a: i128, mut b: i128) -> i128 {
	a = a.abs();
	b = b.abs();
	while b != 0 {
		let remainder = a % b;
		a = b;
		b = remainder;
	}
	a
}

impl Size {
	pub(crate) const ONE: Self = Self::exact(1, 1);

	const fn exact(numerator: i128, denominator: i128) -> Self {
		Self {
			numerator,
			denominator,
			pi: 0,
			pixel: 0,
		}
	}

	const fn with_pi(self, pi: i8) -> Self {
		Self { pi, ..self }
	}

	/// The size in lowest terms with a positive denominator, or `None` for a zero denominator.
	fn reduced(numerator: i128, denominator: i128, pi: i8, pixel: i8) -> Option<Self> {
		if denominator == 0 {
			return None;
		}
		let divisor = greatest_common_divisor(numerator, denominator);
		let sign = denominator.signum();
		Some(Self {
			numerator: sign * numerator / divisor,
			denominator: sign * denominator / divisor,
			pi,
			pixel,
		})
	}

	fn times(self, other: Self) -> Option<Self> {
		// Cross-reducing first keeps the products as small as they can be
		let left = greatest_common_divisor(self.numerator, other.denominator).max(1);
		let right = greatest_common_divisor(other.numerator, self.denominator).max(1);
		let numerator = (self.numerator / left).checked_mul(other.numerator / right)?;
		let denominator = (self.denominator / right).checked_mul(other.denominator / left)?;
		Self::reduced(numerator, denominator, self.pi.checked_add(other.pi)?, self.pixel.checked_add(other.pixel)?)
	}

	fn inverse(self) -> Option<Self> {
		Self::reduced(self.denominator, self.numerator, self.pi.checked_neg()?, self.pixel.checked_neg()?)
	}

	/// The reciprocal of a size that is a ratio of integers alone, which is always representable.
	pub(crate) fn inverse_exact(self) -> Self {
		self.inverse().unwrap_or(Self::ONE)
	}

	fn over(self, other: Self) -> Option<Self> {
		self.times(other.inverse()?)
	}

	/// The size raised to an integer power, in O(|exponent|) exact multiplications.
	fn power(self, exponent: i8) -> Option<Self> {
		let base = if exponent < 0 { self.inverse()? } else { self };
		(0..exponent.unsigned_abs()).try_fold(Self::ONE, |power, _| power.times(base))
	}

	pub(crate) fn is_one(self) -> bool {
		self == Self::ONE
	}

	/// The size split into its numerator with any powers of π and the pixels per inch, and its denominator, so a product can
	/// scale one operand by each before combining them, rounding no more than the conversions would on their own.
	pub(crate) fn split(self) -> (Self, Self) {
		(Self { denominator: 1, ..self }, Self::exact(self.denominator, 1))
	}

	/// The size as a ratio of integers with the pixels per inch substituted, or `None` where a power of π remains or the
	/// pixels per inch is no short decimal.
	pub(crate) fn rational(self, pixels_per_inch: f64) -> Option<(i128, i128)> {
		if self.pi != 0 {
			return None;
		}
		if self.pixel == 0 {
			return Some((self.numerator, self.denominator));
		}

		// A pixel is an inch over the pixels per inch, read exactly when it has at most six decimal places
		let (pixels, per) = (0..=6).find_map(|places| {
			let scale = 10_i128.pow(places);
			let scaled = pixels_per_inch * scale as f64;
			(scaled.fract() == 0. && scaled.abs() < (1_u64 << 53) as f64 && scaled > 0.).then_some((scaled as i128, scale))
		})?;
		let size = self.with_pixel(0).times(Self::reduced(per, pixels, 0, 0)?.power(self.pixel)?)?;
		Some((size.numerator, size.denominator))
	}

	const fn with_pixel(self, pixel: i8) -> Self {
		Self { pixel, ..self }
	}

	/// Multiplies a number by the size: by the numerator, then the denominator, so an integer stays exact and a real rounds
	/// once for each, then by any powers of π and the pixels per inch.
	pub(crate) fn apply(self, number: Number, pixels_per_inch: f64) -> Option<Number> {
		let integer = |integer: i128| i64::try_from(integer).map_or(Number::Real(integer as f64), Number::Integer);

		if let Number::Integer(value) = number
			&& self.pi == 0
			&& self.pixel == 0
			&& let Some(scaled) = (value as i128).checked_mul(self.numerator)
			&& scaled % self.denominator == 0
			&& let Ok(exact) = i64::try_from(scaled / self.denominator)
		{
			return Some(Number::Integer(exact));
		}

		let mut result = number;
		if self.numerator != 1 {
			result = result.binary_op(BinaryOp::Mul, integer(self.numerator))?.canonical();
		}
		if self.denominator != 1 {
			result = result.binary_op(BinaryOp::Div, integer(self.denominator))?.canonical();
		}
		let power_of = |base: f64, exponent: i8| Number::Real(base.powi(exponent.unsigned_abs() as i32));
		if self.pi != 0 {
			let operation = if self.pi > 0 { BinaryOp::Mul } else { BinaryOp::Div };
			result = result.binary_op(operation, power_of(PI, self.pi))?.canonical();
		}
		if self.pixel != 0 {
			let operation = if self.pixel > 0 { BinaryOp::Div } else { BinaryOp::Mul };
			result = result.binary_op(operation, power_of(pixels_per_inch, self.pixel))?.canonical();
		}
		Some(result)
	}
}

/// One entry of the unit table.
#[derive(Debug)]
pub struct UnitDefinition {
	/// The spelling results are written with.
	pub symbol: &'static str,
	/// The unit's name, for documentation and completions.
	pub name: &'static str,
	/// Every spelling naming the unit, its symbol first, then its other abbreviations and its singular and plural names.
	pub spellings: &'static [&'static str],
	pub dimension: Dimension,
	/// The size in the coherent unit of its dimension: meters, kilograms, seconds, radians, and their products.
	size: Size,
}

const fn unit(symbol: &'static str, name: &'static str, spellings: &'static [&'static str], dimension: Dimension, size: Size) -> UnitDefinition {
	UnitDefinition {
		symbol,
		name,
		spellings,
		dimension,
		size,
	}
}

const fn ratio(numerator: i128, denominator: i128) -> Size {
	Size::exact(numerator, denominator)
}

const INCH: Size = ratio(127, 5000);
const GALLON: Size = ratio(3_785_411_784, 1_000_000_000_000);

/// Every unit the language knows, related units together. The metric prefixes are spelled out per unit rather than applying to
/// any unit, since `min`, `ft`, and `cd` would otherwise also read as a milli-inch, a femto-tonne, and a centi-day.
pub const UNITS: [UnitDefinition; 54] = [
	// Length
	unit("px", "pixel", &["px", "pixel", "pixels"], Dimension::LENGTH, INCH.with_pixel(1)),
	unit("pt", "point", &["pt", "point", "points"], Dimension::LENGTH, ratio(127, 360_000)),
	unit("pc", "pica", &["pc", "pica", "picas"], Dimension::LENGTH, ratio(127, 30_000)),
	unit("nm", "nanometer", &["nm", "nanometer", "nanometers"], Dimension::LENGTH, ratio(1, 1_000_000_000)),
	unit(
		"µm",
		"micrometer",
		&["µm", "μm", "um", "micrometer", "micrometers", "micron", "microns"],
		Dimension::LENGTH,
		ratio(1, 1_000_000),
	),
	unit("mm", "millimeter", &["mm", "millimeter", "millimeters"], Dimension::LENGTH, ratio(1, 1000)),
	unit("cm", "centimeter", &["cm", "centimeter", "centimeters"], Dimension::LENGTH, ratio(1, 100)),
	unit("m", "meter", &["m", "meter", "meters"], Dimension::LENGTH, ratio(1, 1)),
	unit("km", "kilometer", &["km", "kilometer", "kilometers"], Dimension::LENGTH, ratio(1000, 1)),
	unit("thou", "thou", &["thou"], Dimension::LENGTH, ratio(127, 5_000_000)),
	unit("in", "inch", &["in", "\"", "inch", "inches"], Dimension::LENGTH, INCH),
	unit("ft", "foot", &["ft", "'", "foot", "feet"], Dimension::LENGTH, ratio(381, 1250)),
	unit("yd", "yard", &["yd", "yard", "yards"], Dimension::LENGTH, ratio(1143, 1250)),
	unit("mi", "mile", &["mi", "mile", "miles"], Dimension::LENGTH, ratio(201_168, 125)),
	unit("nmi", "nautical mile", &["nmi"], Dimension::LENGTH, ratio(1852, 1)),
	// Area
	unit("are", "are", &["are", "ares"], Dimension::AREA, ratio(100, 1)),
	unit("hectare", "hectare", &["hectare", "hectares"], Dimension::AREA, ratio(10_000, 1)),
	unit("acre", "acre", &["acre", "acres"], Dimension::AREA, ratio(40_468_564_224, 10_000_000)),
	// Volume
	unit("ml", "milliliter", &["ml", "milliliter", "milliliters"], Dimension::VOLUME, ratio(1, 1_000_000)),
	unit("cl", "centiliter", &["cl", "centiliter", "centiliters"], Dimension::VOLUME, ratio(1, 100_000)),
	unit("l", "liter", &["l", "liter", "liters"], Dimension::VOLUME, ratio(1, 1000)),
	unit("floz", "fluid ounce", &["floz"], Dimension::VOLUME, ratio(3_785_411_784, 128_000_000_000_000)),
	unit("cup", "cup", &["cup", "cups"], Dimension::VOLUME, ratio(3_785_411_784, 16_000_000_000_000)),
	unit("pint", "pint", &["pint", "pints"], Dimension::VOLUME, ratio(3_785_411_784, 8_000_000_000_000)),
	unit("qt", "quart", &["qt", "quart", "quarts"], Dimension::VOLUME, ratio(3_785_411_784, 4_000_000_000_000)),
	unit("gal", "gallon", &["gal", "gallon", "gallons"], Dimension::VOLUME, GALLON),
	// Mass
	unit("µg", "microgram", &["µg", "μg", "ug", "microgram", "micrograms"], Dimension::MASS, ratio(1, 1_000_000_000)),
	unit("mg", "milligram", &["mg", "milligram", "milligrams"], Dimension::MASS, ratio(1, 1_000_000)),
	unit("g", "gram", &["g", "gram", "grams"], Dimension::MASS, ratio(1, 1000)),
	unit("kg", "kilogram", &["kg", "kilogram", "kilograms"], Dimension::MASS, ratio(1, 1)),
	unit("tonne", "tonne", &["tonne", "tonnes"], Dimension::MASS, ratio(1000, 1)),
	unit("gr", "grain", &["gr", "grain", "grains"], Dimension::MASS, ratio(6_479_891, 100_000_000_000)),
	unit("oz", "ounce", &["oz", "ounce", "ounces"], Dimension::MASS, ratio(28_349_523_125, 1_000_000_000_000)),
	unit("lb", "pound", &["lb", "lbs", "pound", "pounds"], Dimension::MASS, ratio(45_359_237, 100_000_000)),
	unit("ton", "ton", &["ton", "tons"], Dimension::MASS, ratio(90_718_474, 100_000)),
	// Time
	unit("ns", "nanosecond", &["ns", "nanosecond", "nanoseconds"], Dimension::TIME, ratio(1, 1_000_000_000)),
	unit("µs", "microsecond", &["µs", "μs", "us", "microsecond", "microseconds"], Dimension::TIME, ratio(1, 1_000_000)),
	unit("ms", "millisecond", &["ms", "millisecond", "milliseconds"], Dimension::TIME, ratio(1, 1000)),
	unit("s", "second", &["s", "sec", "second", "seconds"], Dimension::TIME, ratio(1, 1)),
	unit("min", "minute", &["min", "minute", "minutes"], Dimension::TIME, ratio(60, 1)),
	unit("h", "hour", &["h", "hr", "hour", "hours"], Dimension::TIME, ratio(3600, 1)),
	unit("day", "day", &["day", "days"], Dimension::TIME, ratio(86_400, 1)),
	// Angle
	unit("rad", "radian", &["rad", "radian", "radians"], Dimension::ANGLE, ratio(1, 1)),
	unit("°", "degree", &["°", "deg", "degree", "degrees"], Dimension::ANGLE, ratio(1, 180).with_pi(1)),
	unit("turn", "turn", &["turn", "turns"], Dimension::ANGLE, ratio(2, 1).with_pi(1)),
	// Frequency
	unit("Hz", "hertz", &["Hz", "hertz"], Dimension::FREQUENCY, ratio(1, 1)),
	unit("kHz", "kilohertz", &["kHz", "kilohertz"], Dimension::FREQUENCY, ratio(1000, 1)),
	unit("MHz", "megahertz", &["MHz", "megahertz"], Dimension::FREQUENCY, ratio(1_000_000, 1)),
	unit("GHz", "gigahertz", &["GHz", "gigahertz"], Dimension::FREQUENCY, ratio(1_000_000_000, 1)),
	// Angular speed
	unit("rpm", "revolution per minute", &["rpm"], Dimension::ANGULAR_SPEED, ratio(1, 30).with_pi(1)),
	unit("rps", "revolution per second", &["rps"], Dimension::ANGULAR_SPEED, ratio(2, 1).with_pi(1)),
	// Speed
	unit("mph", "mile per hour", &["mph"], Dimension::SPEED, ratio(1397, 3125)),
	unit("knot", "knot", &["knot", "knots"], Dimension::SPEED, ratio(463, 900)),
	// Dimensionless
	unit("%", "percent", &["%", "percent"], Dimension::NONE, ratio(1, 100)),
];

/// An index into [`UNITS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId(u8);

impl UnitId {
	pub fn definition(self) -> &'static UnitDefinition {
		&UNITS[self.0 as usize]
	}

	/// The unit whose symbol this is, found as the table is compiled, so naming a unit that isn't in it fails to build.
	const fn of_symbol(symbol: &str) -> Self {
		let mut index = 0;
		while index < UNITS.len() {
			if bytes_equal(UNITS[index].symbol.as_bytes(), symbol.as_bytes()) {
				return Self(index as u8);
			}
			index += 1;
		}
		panic!("no unit has this symbol");
	}

	pub const RADIAN: Self = Self::of_symbol("rad");
	pub const PIXEL: Self = Self::of_symbol("px");
}

const fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
	if a.len() != b.len() {
		return false;
	}
	let mut index = 0;
	while index < a.len() {
		if a[index] != b[index] {
			return false;
		}
		index += 1;
	}
	true
}

/// The unit a spelling names, like `mm` or `inches`, read after any `\` prefix.
pub fn unit_named(name: &str) -> Option<UnitId> {
	let name = name.strip_prefix('\\').unwrap_or(name);
	UNITS.iter().position(|unit| unit.spellings.contains(&name)).map(|index| UnitId(index as u8))
}

/// The unit each base dimension is coherently measured in, so a product of them needs no conversion factor.
fn coherent_unit(base: usize) -> UnitId {
	const COHERENT: [UnitId; 4] = [UnitId::of_symbol("m"), UnitId::of_symbol("kg"), UnitId::of_symbol("s"), UnitId::RADIAN];
	COHERENT[base]
}

/// A unit raised to a nonzero power within a [`Unit`], where an empty slot has unit 0 and exponent 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
struct Factor {
	/// One more than the unit's index, so 0 marks an empty slot.
	unit: u8,
	exponent: i8,
}

impl Factor {
	fn id(self) -> UnitId {
		UnitId(self.unit - 1)
	}
}

/// What a value is measured in: a product of at most four units from the table, each raised to a nonzero integer power, like
/// `km/h` or `m^2`. A value without one is plain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Unit {
	/// Ordered by unit, with the empty slots last, so equal products compare equal.
	factors: [Factor; 4],
}

/// Why units can't combine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitError {
	/// The units' sizes or powers are too far out to represent.
	Overflow,
	/// A power leaves a fractional exponent on a unit, like `sqrt(1mm)`.
	FractionalPower,
	/// The units measure different dimensions.
	Incompatible,
}

impl Unit {
	pub const NONE: Self = Self {
		factors: [Factor { unit: 0, exponent: 0 }; 4],
	};

	/// The unit alone, to the first power.
	pub const fn of(unit: UnitId) -> Self {
		let mut result = Self::NONE;
		result.factors[0] = Factor { unit: unit.0 + 1, exponent: 1 };
		result
	}

	/// The unit a spelling names, like `mm`.
	pub fn named(name: &str) -> Option<Self> {
		unit_named(name).map(Self::of)
	}

	/// A product of units from their exponents, merging repeated units, or `None` past four distinct units or an exponent past ±127.
	fn from_factors(factors: &[(UnitId, i8)]) -> Option<Self> {
		let mut merged: Vec<(UnitId, i8)> = Vec::with_capacity(factors.len());
		for &(unit, exponent) in factors {
			match merged.iter_mut().find(|(existing, _)| *existing == unit) {
				Some((_, total)) => *total = total.checked_add(exponent)?,
				None => merged.push((unit, exponent)),
			}
		}
		merged.retain(|(_, exponent)| *exponent != 0);
		if merged.iter().any(|(_, exponent)| *exponent == i8::MIN) {
			return None;
		}
		merged.sort_by_key(|(unit, _)| *unit);
		if merged.len() > 4 {
			return None;
		}

		let mut result = Self::NONE;
		for (slot, (unit, exponent)) in result.factors.iter_mut().zip(merged) {
			*slot = Factor { unit: unit.0 + 1, exponent };
		}
		Some(result)
	}

	fn entries(self) -> impl Iterator<Item = (UnitId, i8)> {
		self.factors.into_iter().filter(|factor| factor.unit != 0).map(|factor| (factor.id(), factor.exponent))
	}

	#[inline(always)]
	pub fn is_none(self) -> bool {
		self.factors[0].unit == 0
	}

	/// The unit, if it is one unit to the first power, like `mm` but not `mm^2` or `km/h`.
	pub fn single(self) -> Option<UnitId> {
		let mut entries = self.entries();
		match (entries.next(), entries.next()) {
			(Some((unit, 1)), None) => Some(unit),
			_ => None,
		}
	}

	pub fn dimension(self) -> Dimension {
		// Exponents stay within ±127 and a unit's dimension within ±3, so these sums can't overflow
		let mut dimension = [0_i16; 4];
		for (unit, exponent) in self.entries() {
			for (total, base) in dimension.iter_mut().zip(unit.definition().dimension.0) {
				*total += base * exponent as i16;
			}
		}
		Dimension(dimension)
	}

	/// Whether the unit is a dimensionless one like `%`, which plain numbers don't add to but which folds away in products.
	pub fn is_dimensionless(self) -> bool {
		!self.is_none() && self.dimension().is_none()
	}

	/// The size of one of this unit in coherent units.
	pub(crate) fn size(self) -> Option<Size> {
		self.entries().try_fold(Size::ONE, |size, (unit, exponent)| size.times(unit.definition().size.power(exponent)?))
	}

	pub fn inverse(self) -> Self {
		let mut result = self;
		for factor in result.factors.iter_mut().filter(|factor| factor.unit != 0) {
			factor.exponent = -factor.exponent;
		}
		result
	}

	/// The unit raised to a real power, which must leave every exponent an integer, like `0.5` on `mm^2`.
	pub fn power(self, exponent: f64) -> Result<Self, UnitError> {
		let factors = self
			.entries()
			.map(|(unit, power)| {
				// A root's reciprocal exponent rounds, so a power within a hair of an integer counts as that integer
				let raised = power as f64 * exponent;
				let whole = raised.round();
				if (raised - whole).abs() > 1e-9 {
					return Err(UnitError::FractionalPower);
				}
				if !(-(i8::MAX as f64)..=i8::MAX as f64).contains(&whole) {
					return Err(UnitError::Overflow);
				}
				Ok((unit, whole as i8))
			})
			.collect::<Result<Vec<_>, _>>()?;
		Self::from_factors(&factors).ok_or(UnitError::Overflow)
	}

	/// The product of two units with the factor its number is multiplied by: units of one dimension merge into one, so a
	/// value's units never mix (`mm in` is `mm^2`), units whose dimensions overlap expand to cancel (`rpm s` is a fraction of
	/// a turn), and a dimensionless unit like `%` folds into the number beside any other unit or past its first power.
	pub(crate) fn product(self, other: Self, pixels_per_inch: f64) -> Result<(Self, Size), UnitError> {
		// A plain operand leaves the other's unit as it is, unless that is a percentage, which folds past its first power
		let lone = match (self.is_none(), other.is_none()) {
			(true, true) => return Ok((Self::NONE, Size::ONE)),
			(true, false) => Some(other),
			(false, true) => Some(self),
			(false, false) => None,
		};
		if let Some(lone) = lone
			&& lone.entries().all(|(unit, exponent)| exponent == 1 || !unit.definition().dimension.is_none())
		{
			return Ok((lone, Size::ONE));
		}

		let mut factors: Vec<(UnitId, i8)> = Vec::with_capacity(8);
		for (unit, exponent) in self.entries().chain(other.entries()) {
			match factors.iter_mut().find(|(existing, _)| *existing == unit) {
				Some((_, total)) => *total = total.checked_add(exponent).ok_or(UnitError::Overflow)?,
				None => factors.push((unit, exponent)),
			}
		}
		factors.retain(|(_, exponent)| *exponent != 0);

		let mut scale = Size::ONE;
		loop {
			// Two units of one dimension merge into the one a sum of them would be measured in
			let same_dimension = pairs(factors.len()).find(|&(a, b)| factors[a].0.definition().dimension == factors[b].0.definition().dimension);
			if let Some((a, b)) = same_dimension {
				let (unit_a, unit_b) = (factors[a].0, factors[b].0);
				let target = common_unit(Unit::of(unit_a), Unit::of(unit_b), pixels_per_inch);
				let (from, into) = if target == Unit::of(unit_b) { (a, b) } else { (b, a) };

				// Converting `from^e` into `into^e` multiplies by the ratio of their sizes to the `e`
				let exponent = factors[from].1;
				let ratio = factors[from].0.definition().size.over(factors[into].0.definition().size).ok_or(UnitError::Overflow)?;
				scale = scale.times(ratio.power(exponent).ok_or(UnitError::Overflow)?).ok_or(UnitError::Overflow)?;
				factors[into].1 = factors[into].1.checked_add(exponent).ok_or(UnitError::Overflow)?;
				factors.remove(from);
				factors.retain(|(_, exponent)| *exponent != 0);
				continue;
			}

			// Units whose dimensions overlap without matching expand into coherent units, where the shared dimension merges
			let overlapping = pairs(factors.len()).find(|&(a, b)| factors[a].0.definition().dimension.overlaps(factors[b].0.definition().dimension));
			if let Some((a, b)) = overlapping {
				let expanded = [factors[a], factors[b]];
				factors.retain(|(unit, _)| *unit != expanded[0].0 && *unit != expanded[1].0);
				for (unit, exponent) in expanded {
					scale = scale.times(unit.definition().size.power(exponent).ok_or(UnitError::Overflow)?).ok_or(UnitError::Overflow)?;
					push_coherent(&mut factors, unit.definition().dimension, exponent)?;
				}
				continue;
			}

			break;
		}

		// A dimensionless unit is a plain factor of the number beside any unit with a dimension, and a percentage of a percentage is one percentage
		let has_dimension = factors.iter().any(|(unit, _)| !unit.definition().dimension.is_none());
		for (unit, exponent) in factors.iter_mut().filter(|(unit, _)| unit.definition().dimension.is_none()) {
			let folded = if has_dimension || *exponent < 0 { *exponent } else { *exponent - 1 };
			scale = scale.times(unit.definition().size.power(folded).ok_or(UnitError::Overflow)?).ok_or(UnitError::Overflow)?;
			*exponent -= folded;
		}
		factors.retain(|(_, exponent)| *exponent != 0);

		// Past four distinct units, everything is measured in coherent units, which take at most one per base dimension
		if factors.len() > 4 {
			let mut coherent = Vec::new();
			for (unit, exponent) in factors {
				scale = scale.times(unit.definition().size.power(exponent).ok_or(UnitError::Overflow)?).ok_or(UnitError::Overflow)?;
				push_coherent(&mut coherent, unit.definition().dimension, exponent)?;
			}
			factors = coherent;
		}

		Ok((Self::from_factors(&factors).ok_or(UnitError::Overflow)?, scale))
	}

	/// The factor that converts a number in this unit into `target`, which must measure the same dimension.
	pub(crate) fn conversion(self, target: Self) -> Result<Size, UnitError> {
		if self.dimension() != target.dimension() {
			return Err(UnitError::Incompatible);
		}
		if self == target {
			return Ok(Size::ONE);
		}
		self.size().zip(target.size()).and_then(|(from, into)| from.over(into)).ok_or(UnitError::Overflow)
	}
}

/// Every pair of distinct indices below `count`.
fn pairs(count: usize) -> impl Iterator<Item = (usize, usize)> {
	(0..count).flat_map(move |a| (a + 1..count).map(move |b| (a, b)))
}

/// Adds the coherent units of a dimension raised to `exponent` to a list of factors.
fn push_coherent(factors: &mut Vec<(UnitId, i8)>, dimension: Dimension, exponent: i8) -> Result<(), UnitError> {
	for (base, power) in dimension.0.into_iter().enumerate().filter(|(_, power)| *power != 0) {
		let raised = i8::try_from(power * exponent as i16).ok().filter(|raised| *raised != i8::MIN).ok_or(UnitError::Overflow)?;
		let unit = coherent_unit(base);
		match factors.iter_mut().find(|(existing, _)| *existing == unit) {
			Some((_, total)) => *total = total.checked_add(raised).ok_or(UnitError::Overflow)?,
			None => factors.push((unit, raised)),
		}
	}
	factors.retain(|(_, exponent)| *exponent != 0);
	Ok(())
}

/// Whether a ratio's denominator divides a power of ten, so the ratio is a terminating decimal.
fn terminates(mut denominator: i128) -> bool {
	for prime in [2, 5] {
		while denominator % prime == 0 {
			denominator /= prime;
		}
	}
	denominator == 1
}

/// Which of two units of one dimension a sum of them is measured in: the one the other converts into by an integer, like
/// millimeters beside meters, or else by a terminating decimal, like meters beside feet, or else the one whose size has no
/// factor of π, like radians beside degrees, or else the left one.
pub fn common_unit(left: Unit, right: Unit, pixels_per_inch: f64) -> Unit {
	if left == right {
		return left;
	}
	let (Some(left_size), Some(right_size)) = (left.size(), right.size()) else { return left };
	let Some(ratio) = left_size.over(right_size) else { return left };

	// The ratio is how many of the right unit one of the left makes
	match ratio.rational(pixels_per_inch) {
		Some((_, 1)) => right,
		Some((1, _)) => left,
		Some((_, denominator)) if terminates(denominator) => right,
		Some((numerator, _)) if terminates(numerator) => left,
		Some(_) => left,
		None if left_size.pi != 0 && right_size.pi == 0 => right,
		None => left,
	}
}

impl fmt::Display for Unit {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let write_factor = |f: &mut fmt::Formatter<'_>, unit: UnitId, exponent: i8| {
			let symbol = unit.definition().symbol;
			if exponent.unsigned_abs() == 1 {
				f.write_str(symbol)
			} else {
				write!(f, "{symbol}^{}", exponent.unsigned_abs())
			}
		};

		let numerator: Vec<_> = self.entries().filter(|(_, exponent)| *exponent > 0).collect();
		let denominator: Vec<_> = self.entries().filter(|(_, exponent)| *exponent < 0).collect();

		if numerator.is_empty() && !denominator.is_empty() {
			f.write_str("1")?;
		}
		for (index, (unit, exponent)) in numerator.iter().enumerate() {
			if index > 0 {
				f.write_str(" ")?;
			}
			write_factor(f, *unit, *exponent)?;
		}

		if !denominator.is_empty() {
			f.write_str("/")?;
			let grouped = denominator.len() > 1;
			if grouped {
				f.write_str("(")?;
			}
			for (index, (unit, exponent)) in denominator.iter().enumerate() {
				if index > 0 {
					f.write_str(" ")?;
				}
				write_factor(f, *unit, *exponent)?;
			}
			if grouped {
				f.write_str(")")?;
			}
		}
		Ok(())
	}
}

impl Unit {
	/// Whether a number is written right against this unit, like `45°` or `50%`, rather than with a space, like `5 mm`.
	pub fn attaches_without_space(self) -> bool {
		let first = self.entries().find(|(_, exponent)| *exponent > 0);
		first.is_some_and(|(unit, _)| !unit.definition().symbol.starts_with(char::is_alphabetic))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn unit(name: &str) -> Unit {
		Unit::named(name).unwrap_or_else(|| panic!("`{name}` should be a unit"))
	}

	#[test]
	fn spellings_are_unique_and_symbols_lead() {
		let mut seen = std::collections::HashSet::new();
		for definition in &UNITS {
			assert_eq!(definition.spellings[0], definition.symbol, "`{}` lists its symbol first", definition.name);
			for spelling in definition.spellings {
				assert!(seen.insert(*spelling), "`{spelling}` names two units");
			}
		}
	}

	#[test]
	fn sums_take_the_unit_that_converts_exactly() {
		let ppi = DEFAULT_PIXELS_PER_INCH;
		assert_eq!(common_unit(unit("m"), unit("mm"), ppi), unit("mm"));
		assert_eq!(common_unit(unit("mm"), unit("m"), ppi), unit("mm"));
		assert_eq!(common_unit(unit("m"), unit("ft"), ppi), unit("m"));
		assert_eq!(common_unit(unit("ft"), unit("m"), ppi), unit("m"));
		assert_eq!(common_unit(unit("in"), unit("mm"), ppi), unit("mm"));
		assert_eq!(common_unit(unit("in"), unit("px"), ppi), unit("px"));
		assert_eq!(common_unit(unit("deg"), unit("rad"), ppi), unit("rad"));
		assert_eq!(common_unit(unit("rad"), unit("deg"), ppi), unit("rad"));
		assert_eq!(common_unit(unit("turn"), unit("deg"), ppi), unit("deg"));
		assert_eq!(common_unit(unit("px"), unit("mm"), ppi), unit("px"));
		assert_eq!(common_unit(unit("mm"), unit("px"), ppi), unit("mm"));
	}

	#[test]
	fn products_merge_units_of_one_dimension() {
		let ppi = DEFAULT_PIXELS_PER_INCH;
		let (product, scale) = unit("mm").product(unit("in"), ppi).unwrap();
		assert_eq!(product, unit("mm").power(2.).unwrap());
		assert_eq!(scale.apply(Number::Integer(1), ppi), Some(Number::Real(25.4)));

		let (quotient, scale) = unit("m").product(unit("mm").inverse(), ppi).unwrap();
		assert_eq!(quotient, Unit::NONE);
		assert_eq!(scale.apply(Number::Integer(1), ppi), Some(Number::Integer(1000)));

		// An angular speed over a duration expands so the time cancels
		let (angle, _) = unit("rpm").product(unit("s"), ppi).unwrap();
		assert_eq!(angle.dimension(), Dimension::ANGLE);

		// A percentage folds into the number beside a length, and stays once beside a plain number
		let (length, scale) = unit("%").product(unit("mm"), ppi).unwrap();
		assert_eq!(length, unit("mm"));
		assert_eq!(scale.apply(Number::Integer(50), ppi).and_then(Number::as_real), Some(0.5));
		let (percent, scale) = unit("%").product(unit("%"), ppi).unwrap();
		assert_eq!(percent, unit("%"));
		assert_eq!(scale.apply(Number::Integer(2500), ppi).and_then(Number::as_real), Some(25.));
	}

	#[test]
	fn conversions_are_exact_ratios() {
		let ppi = DEFAULT_PIXELS_PER_INCH;
		let convert = |value: i64, from: &str, into: &str| unit(from).conversion(unit(into)).unwrap().apply(Number::Integer(value), ppi).unwrap().canonical();
		assert_eq!(convert(3, "ft", "in"), Number::Integer(36));
		assert_eq!(convert(1, "in", "px"), Number::Integer(96));
		assert_eq!(convert(72, "pt", "in"), Number::Integer(1));
		assert_eq!(convert(1, "turn", "deg"), Number::Integer(360));
		assert_eq!(convert(1, "h", "s"), Number::Integer(3600));
		assert_eq!(convert(254, "mm", "in").as_real(), Some(10.));
		assert!(unit("mm").conversion(unit("s")).is_err());
	}

	#[test]
	fn units_display_as_written() {
		assert_eq!(unit("km").product(unit("h").inverse(), 96.).unwrap().0.to_string(), "km/h");
		assert_eq!(unit("m").product(unit("s").power(-2.).unwrap(), 96.).unwrap().0.to_string(), "m/s^2");
		assert_eq!(unit("s").inverse().to_string(), "1/s");
		assert_eq!(unit("deg").to_string(), "°");
		assert!(unit("deg").attaches_without_space());
		assert!(!unit("mm").attaches_without_space());
	}
}
