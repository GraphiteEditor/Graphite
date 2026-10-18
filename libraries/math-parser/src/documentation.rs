//! What an editor shows on hover for the language's builtins, constants, operators, and literals: a name as the label beside the form
//! it's written in and the range of its results, then what it does, the domain of each argument, and examples, which a test evaluates.

use crate::ast::{BinaryOp, Node};
use crate::constants::{SUFFIXED_FORMS, suffixed_function};
use crate::context::{EvalContext, NothingMap};
use crate::lexer::{Constant, Token};
use crate::matrix::Matrix;
use crate::quaternion::Quaternion;
use crate::reducer::{Reducer, classify_reducer};
use crate::value::Value;

/// Hover text, as the tooltip Markdown subset: code spans, fenced code blocks, bold, and italic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tooltip {
	pub label: String,
	/// How it's written, like `sin(θ)`, `a + b`, or `π`, which an editor shows beside the label.
	pub form: Option<String>,
	pub description: String,
	/// The signature of a call or matrix literal, which its form shows, for an editor to mark the argument or entry being typed in it.
	pub signature: Option<Signature>,
}

/// A call's or matrix literal's parameters between its delimiters, like `sin(` and `)` or `[` and `]`, where `?` marks an optional
/// one and `…` any count of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
	pub opening: String,
	pub parameters: Vec<&'static str>,
	pub separator: &'static str,
	pub closing: &'static str,
	/// For a call, the range of its results, like `[0, ∞]`, written after an arrow.
	pub range: &'static str,
	/// For a drawn matrix literal, the arrow pointing at each entry's row or column, for an editor to draw at the one being typed.
	pub pointers: Vec<Pointer>,
	/// For a call, the description line explaining each parameter, for an editor to mark the one being typed.
	pub lines: Vec<usize>,
}

/// An arrow to draw into a tooltip's code block, at a character of one of its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointer {
	pub line: usize,
	pub column: usize,
	pub arrow: char,
}

impl Signature {
	/// The signature as it's written, like `log(x, b?) → [−∞, ∞]` or `[x; y]`.
	pub fn text(&self) -> String {
		let range = if self.range.is_empty() { String::new() } else { format!(" → {}", self.range) };
		format!("{}{}{}{range}", self.opening, self.parameters.join(self.separator), self.closing)
	}
}

/// The documentation of a builtin, constant, or operator.
pub struct Doc {
	pub description: &'static str,
	/// Expressions with their results, each checked by evaluating both.
	pub examples: &'static [(&'static str, &'static str)],
}

/// How a function or operator takes a value with a vector part, which its description states where its domains don't show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Treatment {
	/// As a whole, which its domains of vectors or values show.
	Whole,
	/// On each part apart from the others.
	Componentwise,
	/// As a complex number whose imaginary axis points along the vector part.
	InPlane,
	/// Not at all, since it takes only real numbers.
	RealOnly,
}

/// A parameter by the name its signature shows, where `?` marks an optional one and `…` any count of them, with the domain of the
/// arguments it takes and what it's for.
pub struct Parameter {
	pub name: &'static str,
	/// Where a real argument gives a real result, like `[0, ∞]`, where a bound is closed only where it's reached, or else a kind of
	/// value, like `vectors`.
	pub domain: &'static str,
	pub description: &'static str,
}

/// A built-in function by name, with its title in words, its treatment of a value with a vector part, its parameters, the range of its
/// results written like its parameters' domains, and its documentation.
pub struct DocumentedFunction {
	pub name: &'static str,
	pub title: &'static str,
	pub treatment: Treatment,
	pub parameters: &'static [Parameter],
	pub range: &'static str,
	pub doc: Doc,
}

impl DocumentedFunction {
	/// Its parameters as its signature lists them, like `x, b?`.
	pub fn parameter_list(&self) -> String {
		self.parameters.iter().map(|parameter| parameter.name).collect::<Vec<_>>().join(", ")
	}
}

const fn doc(description: &'static str, examples: &'static [(&'static str, &'static str)]) -> Doc {
	Doc { description, examples }
}

const fn parameter(name: &'static str, domain: &'static str, description: &'static str) -> Parameter {
	Parameter { name, domain, description }
}

const fn function(name: &'static str, title: &'static str, treatment: Treatment, parameters: &'static [Parameter], range: &'static str, doc: Doc) -> DocumentedFunction {
	DocumentedFunction {
		name,
		title,
		treatment,
		parameters,
		range,
		doc,
	}
}

/// Every built-in function, which an editor offers to finish a name being typed and documents on hover.
pub static BUILTIN_FUNCTIONS: &[DocumentedFunction] = &[
	// Trigonometric functions
	function(
		"sin",
		"Sine",
		Treatment::InPlane,
		&[parameter("θ", "(−∞, ∞)", "the angle in radians")],
		"[−1, 1]",
		doc(
			"The `y` coordinate of the point at the angle `θ` in radians around the unit circle.",
			&[("sin(π/2)", "1"), ("sin(π/6)", "0.5"), ("sin(i)", "1.175i")],
		),
	),
	function(
		"cos",
		"Cosine",
		Treatment::InPlane,
		&[parameter("θ", "(−∞, ∞)", "the angle in radians")],
		"[−1, 1]",
		doc(
			"The `x` coordinate of the point at the angle `θ` in radians around the unit circle.",
			&[("cos(0)", "1"), ("cos(π)", "-1")],
		),
	),
	function(
		"tan",
		"Tangent",
		Treatment::InPlane,
		&[parameter("θ", "(−∞, ∞)", "the angle in radians")],
		"(−∞, ∞)",
		doc(
			"The slope of the line at the angle `θ` in radians, `sin(θ) / cos(θ)`. It grows without bound toward odd multiples of `π/2`.",
			&[("tan(π/4)", "1")],
		),
	),
	function(
		"csc",
		"Cosecant",
		Treatment::InPlane,
		&[parameter("θ", "(−∞, ∞)", "the angle in radians")],
		"[−∞, −1] or [1, ∞]",
		doc("The reciprocal of the sine, `1 / sin(θ)`.", &[("csc(π/2)", "1"), ("csc(0)", "∞")]),
	),
	function(
		"sec",
		"Secant",
		Treatment::InPlane,
		&[parameter("θ", "(−∞, ∞)", "the angle in radians")],
		"(−∞, −1] or [1, ∞)",
		doc("The reciprocal of the cosine, `1 / cos(θ)`.", &[("sec(0)", "1")]),
	),
	function(
		"cot",
		"Cotangent",
		Treatment::InPlane,
		&[parameter("θ", "(−∞, ∞)", "the angle in radians")],
		"[−∞, ∞]",
		doc("The reciprocal of the tangent, `1 / tan(θ)`.", &[("cot(π/4)", "1")]),
	),
	// Inverse trigonometric functions
	function(
		"asin",
		"Inverse Sine",
		Treatment::InPlane,
		&[parameter("x", "[−1, 1]", "the sine")],
		"[−π/2, π/2]",
		doc(
			"The angle whose sine is `x`. Outside its domain, the result climbs into the complex plane.",
			&[("asin(1)", "π/2"), ("asin(2)", "π/2 - 1.317i")],
		),
	),
	function(
		"acos",
		"Inverse Cosine",
		Treatment::InPlane,
		&[parameter("x", "[−1, 1]", "the cosine")],
		"[0, π]",
		doc(
			"The angle whose cosine is `x`. Outside its domain, the result climbs into the complex plane.",
			&[("acos(-1)", "π"), ("acos(2)", "1.317i")],
		),
	),
	function(
		"atan",
		"Inverse Tangent",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the tangent")],
		"[−π/2, π/2]",
		doc("The angle whose tangent is `x`.", &[("atan(1)", "π/4"), ("atan(∞)", "π/2")]),
	),
	function(
		"atan2",
		"Angle of Point",
		Treatment::RealOnly,
		&[parameter("y", "[−∞, ∞]", "the point's `y` coordinate"), parameter("x", "[−∞, ∞]", "the point's `x` coordinate")],
		"[−π, π]",
		doc("The angle from the positive `x` axis to the point `(x, y)`.", &[("atan2(1, 1)", "π/4"), ("atan2(0, -1)", "π")]),
	),
	function(
		"acsc",
		"Inverse Cosecant",
		Treatment::InPlane,
		&[parameter("x", "[−∞, −1] or [1, ∞]", "the cosecant")],
		"[−π/2, π/2]",
		doc(
			"The angle whose cosecant is `x`, `asin(1/x)`. Outside its domain, the result climbs into the complex plane, except that 0 is an error.",
			&[("acsc(2)", "π/6")],
		),
	),
	function(
		"asec",
		"Inverse Secant",
		Treatment::InPlane,
		&[parameter("x", "[−∞, −1] or [1, ∞]", "the secant")],
		"[0, π]",
		doc(
			"The angle whose secant is `x`, `acos(1/x)`. Outside its domain, the result climbs into the complex plane, except that 0 is an error.",
			&[("asec(2)", "π/3")],
		),
	),
	function(
		"acot",
		"Inverse Cotangent",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the cotangent")],
		"[−π/2, π/2]",
		doc("The angle whose cotangent is `x`, `atan(1/x)`.", &[("acot(1)", "π/4")]),
	),
	// Hyperbolic functions
	function(
		"sinh",
		"Hyperbolic Sine",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[−∞, ∞]",
		doc(
			"Like the sine, but tracing a hyperbola instead of a circle, `(e^x - e^-x) / 2`.",
			&[("sinh(0)", "0"), ("sinh(1)", "1.175")],
		),
	),
	function(
		"cosh",
		"Hyperbolic Cosine",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[1, ∞]",
		doc(
			"Like the cosine, but tracing a hyperbola instead of a circle, `(e^x + e^-x) / 2`. Its curve is the catenary, the shape of a suspended chain.",
			&[("cosh(0)", "1"), ("cosh(1)", "1.543")],
		),
	),
	function(
		"tanh",
		"Hyperbolic Tangent",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[−1, 1]",
		doc("The ratio `sinh(x) / cosh(x)`, an S-shaped curve rising from `-1` to `1`.", &[("tanh(1)", "0.7616"), ("tanh(∞)", "1")]),
	),
	function(
		"csch",
		"Hyperbolic Cosecant",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[−∞, ∞]",
		doc("The reciprocal of the hyperbolic sine, `1 / sinh(x)`.", &[("csch(1)", "0.8509")]),
	),
	function(
		"sech",
		"Hyperbolic Secant",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[0, 1]",
		doc("The reciprocal of the hyperbolic cosine, `1 / cosh(x)`.", &[("sech(0)", "1")]),
	),
	function(
		"coth",
		"Hyperbolic Cotangent",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[−∞, −1] or [1, ∞]",
		doc("The reciprocal of the hyperbolic tangent, `1 / tanh(x)`.", &[("coth(1)", "1.313")]),
	),
	// Inverse hyperbolic functions
	function(
		"asinh",
		"Inverse Hyperbolic Sine",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the hyperbolic sine")],
		"[−∞, ∞]",
		doc("The value whose hyperbolic sine is `x`.", &[("asinh(1)", "0.8814")]),
	),
	function(
		"acosh",
		"Inverse Hyperbolic Cosine",
		Treatment::InPlane,
		&[parameter("x", "[1, ∞]", "the hyperbolic cosine")],
		"[0, ∞]",
		doc(
			"The value whose hyperbolic cosine is `x`. Outside its domain, the result climbs into the complex plane.",
			&[("acosh(1)", "0"), ("acosh(0.5)", "1.047i")],
		),
	),
	function(
		"atanh",
		"Inverse Hyperbolic Tangent",
		Treatment::InPlane,
		&[parameter("x", "[−1, 1]", "the hyperbolic tangent")],
		"[−∞, ∞]",
		doc(
			"The value whose hyperbolic tangent is `x`. Outside its domain, the result climbs into the complex plane.",
			&[("atanh(0.5)", "0.5493"), ("atanh(1)", "∞")],
		),
	),
	function(
		"acsch",
		"Inverse Hyperbolic Cosecant",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the hyperbolic cosecant")],
		"[−∞, ∞]",
		doc("The value whose hyperbolic cosecant is `x`, `asinh(1/x)`.", &[("acsch(1)", "0.8814")]),
	),
	function(
		"asech",
		"Inverse Hyperbolic Secant",
		Treatment::InPlane,
		&[parameter("x", "[0, 1]", "the hyperbolic secant")],
		"[0, ∞]",
		doc(
			"The value whose hyperbolic secant is `x`, `acosh(1/x)`. Outside its domain, the result climbs into the complex plane.",
			&[("asech(1)", "0")],
		),
	),
	function(
		"acoth",
		"Inverse Hyperbolic Cotangent",
		Treatment::InPlane,
		&[parameter("x", "[−∞, −1] or [1, ∞]", "the hyperbolic cotangent")],
		"[−∞, ∞]",
		doc(
			"The value whose hyperbolic cotangent is `x`, `atanh(1/x)`. Outside its domain, the result climbs into the complex plane, except that 0 is an error.",
			&[("acoth(2)", "0.5493")],
		),
	),
	// Logarithms, exponentials, and roots
	function(
		"ln",
		"Natural Logarithm",
		Treatment::InPlane,
		&[parameter("x", "[0, ∞]", "the value")],
		"[−∞, ∞]",
		doc(
			"The exponent that `e` must be raised to in order to give `x`. A negative `x` climbs into the complex plane.",
			&[("ln(e)", "1"), ("ln(0)", "-∞"), ("ln(-1)", "π i")],
		),
	),
	function(
		"exp",
		"Exponential",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the exponent")],
		"[0, ∞]",
		doc(
			"The constant `e` raised to the power `x`. An imaginary `x` turns around the unit circle.",
			&[("exp(1)", "e"), ("exp(π i)", "-1")],
		),
	),
	function(
		"sqrt",
		"Square Root",
		Treatment::InPlane,
		&[parameter("x", "[0, ∞]", "the value")],
		"[0, ∞]",
		doc(
			"The nonnegative number whose square is `x`. A negative `x` climbs to the imaginary axis.",
			&[("sqrt(9)", "3"), ("sqrt(-4)", "2i")],
		),
	),
	function(
		"cbrt",
		"Cube Root",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[−∞, ∞]",
		doc("The number whose cube is `x`, real for every real `x`.", &[("cbrt(27)", "3"), ("cbrt(-8)", "-2")]),
	),
	function(
		"log",
		"Logarithm",
		Treatment::InPlane,
		&[parameter("x", "[0, ∞]", "the value"), parameter("b?", "[0, ∞]", "the base, 10 if omitted")],
		"[−∞, ∞]",
		doc(
			"The exponent that `b` must be raised to in order to give `x`. A negative `x` or `b` climbs into the complex plane. The base can also be written into the name, like `log2(x)` or `log_1.5(x)`.",
			&[("log(1000)", "3"), ("log(8, 2)", "3"), ("log(-100)", "2 + 1.364i")],
		),
	),
	function(
		"root",
		"Root",
		Treatment::InPlane,
		&[parameter("x", "[0, ∞]", "the value"), parameter("n", "[−∞, ∞]", "the degree")],
		"[0, ∞]",
		doc(
			"The number whose `n`th power is `x`. A root of a negative `x` is real where `n` is an odd integer, and otherwise climbs into the complex plane. The degree can also be written into the name, like `root3(x)` or `root_2.5(x)`.",
			&[("root(16, 4)", "2"), ("root(-8, 3)", "-2")],
		),
	),
	// Geometry functions
	function(
		"hypot",
		"Hypotenuse",
		Treatment::Whole,
		&[parameter("…", "values", "the lengths")],
		"[0, ∞]",
		doc("The square root of the sum of its arguments' squared magnitudes, `sqrt(|a|^2 + |b|^2 + …)`.", &[("hypot(3, 4)", "5")]),
	),
	// Rounding and parts
	function(
		"abs",
		"Absolute Value",
		Treatment::Componentwise,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"[0, ∞]",
		doc(
			"The value `x` with the sign of each part dropped, which folds a point into the first quadrant or octant. The bars `|x|` take the magnitude instead, the length of a vector.",
			&[("abs(-3)", "3"), ("abs(-3i + 4j)", "3i + 4j")],
		),
	),
	function(
		"floor",
		"Floor",
		Treatment::Componentwise,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"integers or ±∞",
		doc("The greatest integer that is at most `x`.", &[("floor(2.7)", "2"), ("floor(-2.5)", "-3")]),
	),
	function(
		"ceil",
		"Ceiling",
		Treatment::Componentwise,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"integers or ±∞",
		doc("The least integer that is at least `x`.", &[("ceil(2.1)", "3"), ("ceil(-0.5)", "0")]),
	),
	function(
		"round",
		"Round",
		Treatment::Componentwise,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"integers or ±∞",
		doc("The integer nearest to `x`, with halves rounded away from 0.", &[("round(2.5)", "3"), ("round(-2.5)", "-3")]),
	),
	function(
		"trunc",
		"Truncate",
		Treatment::Componentwise,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"integers or ±∞",
		doc("The integer part of `x`, rounding toward 0 by dropping its fraction.", &[("trunc(-2.7)", "-2")]),
	),
	function(
		"fract",
		"Fractional Part",
		Treatment::Componentwise,
		&[parameter("x", "(−∞, ∞)", "the value")],
		"(−1, 1)",
		doc(
			"What's left of `x` after dropping its integer part, `x - trunc(x)`, keeping its sign.",
			&[("fract(2.75)", "0.75"), ("fract(-1.25)", "-0.25")],
		),
	),
	function(
		"sign",
		"Sign",
		Treatment::Componentwise,
		&[parameter("x", "[−∞, ∞]", "the value")],
		"{−1, 0, 1}",
		doc("Whether `x` is positive, zero, or negative, as `1`, `0`, or `-1`.", &[("sign(-5)", "-1")]),
	),
	function(
		"snap",
		"Snap",
		Treatment::Componentwise,
		&[parameter("x", "[−∞, ∞]", "the value"), parameter("step", "(−∞, 0) or (0, ∞)", "the spacing")],
		"[−∞, ∞]",
		doc(
			"The multiple of `step` nearest to `x`, `round(x / step) step`, which snaps a point to a square grid.",
			&[("snap(7, 5)", "5"), ("snap(0.37, 0.1)", "0.4")],
		),
	),
	function(
		"mod",
		"Modulo",
		Treatment::Componentwise,
		&[parameter("x", "(−∞, ∞)", "the value"), parameter("m", "[−∞, 0) or (0, ∞]", "the modulus")],
		"[−∞, ∞]",
		doc(
			"The remainder of `x` after dividing by `m`, `x - floor(x / m) m`, which has the sign of `m`. It wraps an angle into one turn with `mod(θ, τ)`.",
			&[("mod(7, 3)", "1"), ("mod(-3.2, 2)", "0.8")],
		),
	),
	// Extremes and statistics
	function(
		"min",
		"Minimum",
		Treatment::Componentwise,
		&[parameter("…", "[−∞, ∞]", "the values")],
		"[−∞, ∞]",
		doc("The least of its arguments.", &[("min(3, 1, 2)", "1")]),
	),
	function(
		"max",
		"Maximum",
		Treatment::Componentwise,
		&[parameter("…", "[−∞, ∞]", "the values")],
		"[−∞, ∞]",
		doc("The greatest of its arguments.", &[("max(3, 1, 2)", "3")]),
	),
	function(
		"mean",
		"Mean",
		Treatment::Whole,
		&[parameter("…", "values", "the values")],
		"values",
		doc("The sum of its arguments divided by their count.", &[("mean(1, 2, 3, 4)", "2.5")]),
	),
	function(
		"median",
		"Median",
		Treatment::RealOnly,
		&[parameter("…", "[−∞, ∞]", "the values")],
		"[−∞, ∞]",
		doc(
			"The middle of its arguments once sorted, or the mean of the two middle ones for an even count.",
			&[("median(3, 1, 2)", "2"), ("median(1, 2, 3, 4)", "2.5")],
		),
	),
	function(
		"variance",
		"Sample Variance",
		Treatment::Whole,
		&[parameter("…", "values", "the values")],
		"[0, ∞]",
		doc(
			"The sum of its arguments' squared distances from their mean, divided by one less than their count. It needs at least two arguments, where `variancepop` divides by the count itself.",
			&[("variance(1, 2, 3, 4)", "1.667")],
		),
	),
	function(
		"variancepop",
		"Population Variance",
		Treatment::Whole,
		&[parameter("…", "values", "the values")],
		"[0, ∞]",
		doc(
			"The sum of its arguments' squared distances from their mean, divided by their count.",
			&[("variancepop(1, 2, 3, 4)", "1.25")],
		),
	),
	function(
		"stdev",
		"Sample Standard Deviation",
		Treatment::Whole,
		&[parameter("…", "values", "the values")],
		"[0, ∞]",
		doc(
			"The typical distance of its arguments from their mean, the square root of `variance`.",
			&[("stdev(2, 4, 4, 4, 5, 5, 7, 9)", "2.138")],
		),
	),
	function(
		"stdevpop",
		"Population Standard Deviation",
		Treatment::Whole,
		&[parameter("…", "values", "the values")],
		"[0, ∞]",
		doc(
			"The typical distance of its arguments from their mean, the square root of `variancepop`.",
			&[("stdevpop(2, 4, 4, 4, 5, 5, 7, 9)", "2")],
		),
	),
	function(
		"geomean",
		"Geometric Mean",
		Treatment::Whole,
		&[parameter("…", "complex numbers", "the values")],
		"complex numbers",
		doc(
			"The principal `n`th root of the product of its `n` arguments. A negative product climbs into the complex plane, as `sqrt` does.",
			&[("geomean(2, 8)", "4")],
		),
	),
	function(
		"harmmean",
		"Harmonic Mean",
		Treatment::Whole,
		&[parameter("…", "complex numbers", "the values")],
		"complex numbers",
		doc("The count of its arguments divided by the sum of their reciprocals.", &[("harmmean(1, 4, 4)", "2")]),
	),
	function(
		"rms",
		"Root Mean Square",
		Treatment::Whole,
		&[parameter("…", "values", "the values")],
		"[0, ∞]",
		doc("The square root of the mean of its arguments' squared magnitudes.", &[("rms(3, 4)", "3.536")]),
	),
	function(
		"mode",
		"Mode",
		Treatment::RealOnly,
		&[parameter("…", "[−∞, ∞]", "the values")],
		"[−∞, ∞]",
		doc(
			"The most frequent of its arguments, the least of any tied. It's an error when no value repeats.",
			&[("mode(1, 2, 2, 3)", "2")],
		),
	),
	function(
		"count",
		"Count",
		Treatment::Whole,
		&[parameter("…", "values", "the values")],
		"nonnegative integers",
		doc("The number of its arguments.", &[("count(5, 6, 7)", "3")]),
	),
	// Logic
	function(
		"xor",
		"Exclusive Or",
		Treatment::Whole,
		&[parameter("…", "bools", "the bools")],
		"bools",
		doc(
			"Whether an odd number of its arguments are true.",
			&[("xor(true, false, true)", "false"), ("xor(true, true, true)", "true")],
		),
	),
	// Interpolation
	function(
		"lerp",
		"Linear Interpolation",
		Treatment::Whole,
		&[
			parameter("a", "values", "the start"),
			parameter("b", "values", "the end"),
			parameter("t", "[−∞, ∞]", "the fraction of the way"),
		],
		"values",
		doc(
			"The value a fraction `t` of the way from `a` to `b`, `a + (b - a) t`. A `t` outside `0..1` carries on past `a` or `b`.",
			&[("lerp(10, 20, 0.25)", "12.5"), ("lerp(0, 3i + 4j, 0.5)", "1.5i + 2j")],
		),
	),
	function(
		"slerp",
		"Spherical Linear Interpolation",
		Treatment::Whole,
		&[
			parameter("a", "values ≠ 0", "the starting rotor"),
			parameter("b", "values ≠ 0", "the ending rotor"),
			parameter("t", "(−∞, ∞)", "the fraction of the way"),
		],
		"values",
		doc(
			"The rotation a fraction `t` of the way from the rotor `a` to `b`, along the shorter arc.",
			&[("slerp(1, i, 0.5)", "0.7071 + 0.7071i")],
		),
	),
	// Vectors
	function(
		"dot",
		"Dot Product",
		Treatment::Whole,
		&[parameter("a", "values", "the first value"), parameter("b", "values", "the second value")],
		"[−∞, ∞]",
		doc("The sum of the products of the matching parts of `a` and `b`, over all four parts.", &[("dot(3i + 4j, 2i)", "6")]),
	),
	function(
		"cross",
		"Cross Product",
		Treatment::Whole,
		&[parameter("a", "vectors", "the first vector"), parameter("b", "vectors", "the second vector")],
		"vectors",
		doc(
			"The vector perpendicular to the vector parts of `a` and `b`, whose length is the area of the parallelogram they span.",
			&[("cross(i, j)", "k")],
		),
	),
	function(
		"normalize",
		"Normalize",
		Treatment::Whole,
		&[parameter("v", "values ≠ 0", "the value to scale")],
		"values",
		doc("The value `v` scaled to length 1.", &[("normalize(3i + 4j)", "0.6i + 0.8j")]),
	),
	function(
		"distance",
		"Distance",
		Treatment::Whole,
		&[parameter("a", "values", "the first point"), parameter("b", "values", "the second point")],
		"[0, ∞]",
		doc("How far apart the points `a` and `b` are, `|a - b|`.", &[("distance(i, 4i + 4j)", "5")]),
	),
	function(
		"angle",
		"Angle Between",
		Treatment::Whole,
		&[parameter("a", "values ≠ 0", "the direction turned from"), parameter("b", "values ≠ 0", "the direction turned to")],
		"[−π, π]",
		doc(
			"How far `a` must turn to face `b`, signed by the turn's direction seen from `+k`. `angle(i, v)` is the heading of `v`.",
			&[("angle(i, j)", "π/2"), ("angle(j, i)", "-π/2")],
		),
	),
	function(
		"rotate",
		"Rotate",
		Treatment::Whole,
		&[
			parameter("v", "values", "the value to rotate"),
			parameter("θ", "(−∞, ∞)", "the angle in radians"),
			parameter("axis?", "vectors ≠ 0", "the axis of rotation, `k` if omitted"),
		],
		"values",
		doc("The value `v` rotated by the angle `θ` about `axis`.", &[("rotate(i, π/2)", "j")]),
	),
	function(
		"rotor",
		"Rotor",
		Treatment::Whole,
		&[
			parameter("θ", "(−∞, ∞)", "the angle in radians"),
			parameter("axis?", "vectors ≠ 0", "the axis of rotation, `k` if omitted"),
		],
		"values",
		doc("The value of length 1 rotating by the angle `θ` about `axis`. It's applied as `q v conj(q)`.", &[("rotor(π)", "k")]),
	),
	function(
		"axis",
		"Axis of Rotation",
		Treatment::Whole,
		&[parameter("q", "values with a vector part", "the rotor")],
		"vectors",
		doc("The unit vector that the rotor `q` turns about.", &[("axis(rotor(1, j))", "j")]),
	),
	function(
		"conj",
		"Conjugate",
		Treatment::Whole,
		&[parameter("q", "values", "the value")],
		"values",
		doc("The value `q` with its vector part negated.", &[("conj(1 + 2i)", "1 - 2i")]),
	),
	function(
		"project",
		"Project",
		Treatment::Whole,
		&[parameter("a", "values", "the value to project"), parameter("b", "values ≠ 0", "the direction projected onto")],
		"values",
		doc("The part of `a` along `b`.", &[("project(3i + 4j, i)", "3i")]),
	),
	function(
		"reject",
		"Reject",
		Treatment::Whole,
		&[parameter("a", "values", "the value to reject"), parameter("b", "values ≠ 0", "the direction rejected from")],
		"values",
		doc("The part of `a` perpendicular to `b`, `a - project(a, b)`.", &[("reject(3i + 4j, i)", "4j")]),
	),
	function(
		"reflect",
		"Reflect",
		Treatment::Whole,
		&[
			parameter("v", "values", "the value to reflect"),
			parameter("n", "values ≠ 0", "the direction the plane is perpendicular to"),
		],
		"values",
		doc("The value `v` reflected across the plane perpendicular to `n`.", &[("reflect(3i + 4j, j)", "3i - 4j")]),
	),
	function(
		"perp",
		"Perpendicular",
		Treatment::Whole,
		&[parameter("v", "vectors", "the vector to turn")],
		"vectors",
		doc("The vector `v` turned a quarter turn counterclockwise in the `xy` plane, `cross(k, v)`.", &[("perp(i)", "j")]),
	),
	// Integers and counting
	function(
		"gcd",
		"Greatest Common Divisor",
		Treatment::Whole,
		&[parameter("…", "integers", "the integers")],
		"nonnegative integers",
		doc(
			"The largest integer that divides all of its integer arguments, or 0 where all are 0.",
			&[("gcd(12, 18)", "6"), ("gcd(0, 0)", "0")],
		),
	),
	function(
		"lcm",
		"Least Common Multiple",
		Treatment::Whole,
		&[parameter("…", "integers", "the integers")],
		"nonnegative integers",
		doc(
			"The smallest positive integer that is a multiple of all of its integer arguments, or 0 where one is 0.",
			&[("lcm(4, 6)", "12"), ("lcm(0, 4)", "0")],
		),
	),
	function(
		"choose",
		"Binomial Coefficient",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the count of things"), parameter("r", "nonnegative integers", "the count chosen")],
		"[−∞, ∞]",
		doc("The number of ways to choose `r` of `x` things, ignoring their order.", &[("choose(5, 2)", "10")]),
	),
	function(
		"pick",
		"Permutations",
		Treatment::InPlane,
		&[parameter("x", "[−∞, ∞]", "the count of things"), parameter("r", "nonnegative integers", "the count picked")],
		"[−∞, ∞]",
		doc(
			"The number of ordered picks of `r` from `x` things, the falling factorial `x (x - 1) … (x - r + 1)`.",
			&[("pick(5, 2)", "20")],
		),
	),
	// Matrices
	function(
		"det",
		"Determinant",
		Treatment::Whole,
		&[parameter("A", "matrices", "the matrix")],
		"[−∞, ∞]",
		doc(
			"The factor by which the matrix `A` scales volume across all four parts, negative where it flips orientation. A matrix literal of fewer than four entries zeroes the parts it leaves out, so its determinant is 0.",
			&[("det(scale(2))", "8"), ("det([i + 2j; 3i + 4j])", "0")],
		),
	),
	function(
		"linear",
		"Linear Part",
		Treatment::Whole,
		&[parameter("A", "matrices", "the matrix")],
		"matrices",
		doc("The matrix `A` without its translation, `A - A 0`.", &[("linear(I + 5i)", "I")]),
	),
	function(
		"translation",
		"Translation",
		Treatment::Whole,
		&[parameter("A", "matrices", "the matrix")],
		"values",
		doc("Where the matrix `A` takes the origin, `A 0`.", &[("translation(I + 5i)", "5i")]),
	),
	function(
		"matrix",
		"Multiplication Matrix",
		Treatment::Whole,
		&[parameter("q", "values", "the value multiplied by")],
		"matrices",
		doc("The matrix multiplying by `q` on the left, so `matrix(q) v` is `q v`.", &[("matrix(i) j", "k")]),
	),
	function(
		"rotation",
		"Rotation Matrix",
		Treatment::Whole,
		&[
			parameter("θ", "(−∞, ∞)", "the angle in radians"),
			parameter("axis?", "vectors ≠ 0", "the axis of rotation, `k` if omitted"),
		],
		"matrices",
		doc("The matrix rotating by the angle `θ` about `axis`.", &[("rotation(π/2) i", "j")]),
	),
	function(
		"scale",
		"Scale Matrix",
		Treatment::Whole,
		&[parameter("q", "values", "the scale")],
		"matrices",
		doc(
			"The matrix scaling each axis by the weight of `q` plus that axis's part, so a real scales space evenly and a vector scales each axis by its own part, flattening any where it's 0.",
			&[("scale(2) (3i + 4j)", "6i + 8j"), ("scale(2i + 3j) (i + j)", "2i + 3j")],
		),
	),
	function(
		"shear",
		"Shear Matrix",
		Treatment::Whole,
		&[
			parameter("along", "values", "the part that moves"),
			parameter("by", "values", "the part it moves by"),
			parameter("factor", "[−∞, ∞]", "how far it moves"),
		],
		"matrices",
		doc("The matrix moving the `along` part by `factor` times the `by` part.", &[("shear(i, j, 0.5) j", "0.5i + j")]),
	),
	// Ranges
	function(
		"within",
		"Within",
		Treatment::Whole,
		&[parameter("p", "values", "the point"), parameter("R", "ranges", "the range or region")],
		"bools",
		doc(
			"Whether `p` lies in the range or region `R`, edges included.",
			&[("within(0.5, 0..1)", "true"), ("within(2i, 0..1)", "false")],
		),
	),
	function(
		"clamp",
		"Clamp",
		Treatment::Whole,
		&[parameter("x", "values", "the value"), parameter("R", "ranges", "the range or region")],
		"values",
		doc(
			"The value `x` held within the range or region `R`. A part that `R` doesn't span goes to 0.",
			&[("clamp(5, 0..1)", "1"), ("clamp(3 + 3i, 1..2)", "2")],
		),
	),
	function(
		"remap",
		"Remap",
		Treatment::Whole,
		&[
			parameter("x", "values", "the value"),
			parameter("A", "ranges", "the range it's carried from"),
			parameter("B", "ranges", "the range it's carried to"),
		],
		"values",
		doc("The value `x` carried from the range `A` to the range `B`, `B A^-1 x`.", &[("remap(5, 0..10, 0..1)", "0.5")]),
	),
	function(
		"smoothstep",
		"Smooth Step",
		Treatment::Whole,
		&[
			parameter("x", "values", "the value"),
			parameter("R", "ranges", "the range it eases across"),
			parameter(
				"continuity?",
				"{0, 1, 2, 3}",
				"how many derivatives reach 0 at the edges (the ramp, the classic cubic, the quintic, or the septic), 1 if omitted",
			),
		],
		"values",
		doc(
			"The eased fraction of the way `x` is across the range `R`, from 0 to 1 on each axis `R` spans and 0 on the others.",
			&[("smoothstep(0.5, 0..1)", "0.5"), ("smoothstep(0.25, 0..1)", "0.15625"), ("smoothstep(0.5 + 3j, 0..1)", "0.5")],
		),
	),
];

/// The hover text for a call of a built-in function, by its name or a suffixed form of it like `log2`, or `log_` before its suffix
/// is written, whose suffix sets its last parameter so the form is called without it.
pub fn function_tooltip(name: &str) -> Option<Tooltip> {
	// The builtin a suffixed form belongs to, and the argument its suffix sets once it's written
	let suffixed = match suffixed_function(name) {
		Some(suffixed) => Some((suffixed.name, Some(suffixed.argument))),
		None => SUFFIXED_FORMS.contains(&name).then(|| (name.trim_end_matches('_'), None)),
	};
	let base = suffixed.map_or(name, |(base, _)| base);
	let function = BUILTIN_FUNCTIONS.iter().find(|function| function.name == base)?;

	// A suffixed form's description still speaks of the parameter its suffix sets, so it's given right after
	let mut description = function.doc.description.to_string();
	let parameters = match (function.parameters.split_last(), suffixed) {
		(Some((last, unsuffixed)), Some((_, argument))) => {
			if let Some(argument) = argument {
				description = format!("{description} Here, `{}` is {argument}.", last.name.trim_end_matches('?'));
			}
			unsuffixed
		}
		_ => function.parameters,
	};
	let (description, lines) = describe(prose(&description, function.treatment), "", parameters, function.doc.examples);
	let signature = Signature {
		opening: format!("{name}("),
		parameters: parameters.iter().map(|parameter| parameter.name).collect(),
		separator: ", ",
		closing: ")",
		range: function.range,
		pointers: Vec::new(),
		lines,
	};

	Some(Tooltip {
		label: function.title.to_string(),
		form: Some(signature.text()),
		description,
		signature: Some(signature),
	})
}

/// A constant's documentation, with the name labeling its tooltip and its value, like `e = 2.71828182845904…`.
struct ConstantDoc {
	name: &'static str,
	value: &'static str,
	doc: Doc,
}

const fn documented_constant(name: &'static str, value: &'static str, doc: Doc) -> ConstantDoc {
	ConstantDoc { name, value, doc }
}

fn constant_doc(constant: Constant) -> ConstantDoc {
	match constant {
		Constant::Pi => documented_constant(
			"Pi",
			"π = τ/2 = 3.14159265358979…",
			doc("The ratio of a circle's circumference to its diameter, half a turn in radians.", &[]),
		),
		Constant::Tau => documented_constant(
			"Tau",
			"τ = 2π = 6.28318530717958…",
			doc("The ratio of a circle's circumference to its radius, a whole turn in radians.", &[]),
		),
		Constant::Phi => documented_constant(
			"Golden Ratio",
			"φ = (1 + √5)/2 = 1.61803398874989…",
			doc("The aspect ratio of the rectangle that splits into a square and a smaller rectangle of the same aspect ratio.", &[]),
		),
		Constant::E => documented_constant(
			"Euler's Number",
			"e = 2.71828182845904…",
			doc("The base of natural exponential growth, `exp(x)`, and of the natural logarithm, `ln(x)`.", &[]),
		),
		Constant::Inf => documented_constant(
			"Infinity",
			"",
			doc(
				"The value greater than every real number. `0 * ∞` and `∞ - ∞` have no single answer, so they're errors.",
				&[("1 / 0", "∞")],
			),
		),
		Constant::I => documented_constant(
			"X Direction",
			"i = √−1",
			doc(
				"The unit basis vector along the `x` axis, which is also the imaginary unit of complex numbers.",
				&[("i^2", "-1"), ("i j", "k")],
			),
		),
		Constant::J => documented_constant("Y Direction", "", doc("The unit basis vector along the `y` axis.", &[("j^2", "-1"), ("j k", "i")])),
		Constant::K => documented_constant("Z Direction", "", doc("The unit basis vector along the `z` axis.", &[("k^2", "-1"), ("i j k", "-1")])),
		Constant::True => documented_constant("True", "true = 1", doc("The bool representing truth.", &[])),
		Constant::False => documented_constant("False", "false = 0", doc("The bool representing falsehood.", &[])),
	}
}

/// The hover text for a constant.
pub fn constant_tooltip(constant: Constant) -> Tooltip {
	let ConstantDoc { name, value, doc } = constant_doc(constant);
	let form = constant.symbol().map_or_else(|| constant.to_string(), str::to_string);
	documented(name, form, &doc, Treatment::Whole, value)
}

const IDENTITY: Doc = doc(
	"The matrix that leaves every value unchanged. A value added to it makes a translation, like `I + 5i`.",
	&[("I (3i + 4j)", "3i + 4j")],
);

/// The hover text for the identity matrix `I`.
pub fn identity_tooltip() -> Tooltip {
	documented("Identity Matrix", "I".to_string(), &IDENTITY, Treatment::Whole, "")
}

/// The hover text for what has no parameters to mark: its name labeling the form it's written in, then its documentation's prose,
/// any value, and its examples.
fn documented(name: &str, form: String, doc: &Doc, treatment: Treatment, value: &str) -> Tooltip {
	Tooltip {
		label: name.to_string(),
		form: Some(form),
		description: describe(prose(doc.description, treatment), value, &[], doc.examples).0,
		signature: None,
	}
}

/// An operator's or keyword's documentation, with the name labeling its tooltip, the form it's written in, like `a + b`, and the range
/// of its results where it has one.
struct NotationDoc {
	name: &'static str,
	form: &'static str,
	range: &'static str,
	doc: Doc,
}

const fn notation(name: &'static str, form: &'static str, range: &'static str, doc: Doc) -> NotationDoc {
	NotationDoc { name, form, range, doc }
}

/// An operator, told apart by where it stands where its token has two meanings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
	Add,
	Subtract,
	Negate,
	Positive,
	Multiply,
	Divide,
	Power,
	Transpose,
	Factorial,
	Not,
	Equal,
	NotEqual,
	Less,
	LessOrEqual,
	Greater,
	GreaterOrEqual,
	And,
	Or,
	Define,
	Magnitude,
}

impl Operator {
	#[cfg(test)]
	const ALL: [Self; 20] = [
		Self::Add,
		Self::Subtract,
		Self::Negate,
		Self::Positive,
		Self::Multiply,
		Self::Divide,
		Self::Power,
		Self::Transpose,
		Self::Factorial,
		Self::Not,
		Self::Equal,
		Self::NotEqual,
		Self::Less,
		Self::LessOrEqual,
		Self::Greater,
		Self::GreaterOrEqual,
		Self::And,
		Self::Or,
		Self::Define,
		Self::Magnitude,
	];

	fn doc(self) -> NotationDoc {
		match self {
			Self::Add => notation(
				"Add",
				"a + b",
				"",
				doc(
					"The sum of `a` and `b`. A value added to a matrix translates it by that value.",
					&[("2 + 3", "5"), ("(1 + 2i) + 3i", "1 + 5i")],
				),
			),
			Self::Subtract => notation("Subtract", "a - b", "", doc("The difference of `a` and `b`.", &[("5 - 3", "2")])),
			Self::Negate => notation(
				"Negate",
				"-a",
				"",
				doc("The value `a` with the sign of each of its parts flipped.", &[("-(2 - 5)", "3"), ("-(3i - 4j)", "-3i + 4j")]),
			),
			Self::Positive => notation("Unary Plus", "+a", "", doc("The value `a` unchanged, written for symmetry with `-a`.", &[("+3", "3")])),
			Self::Multiply => notation(
				"Multiply",
				"a * b",
				"",
				doc(
					"The product of `a` and `b`, where order can matter once both have vector parts. Writing factors side by side, like `2x`, multiplies too. A matrix times a value applies it, like `M p`.",
					&[("i * j", "k"), ("j * i", "-k")],
				),
			),
			Self::Divide => notation(
				"Divide",
				"a / b",
				"",
				doc(
					"The value `a` divided by `b`, which is `a * b^-1`. Dividing a nonzero value by zero gives infinity.",
					&[("7 / 2", "3.5"), ("1 / 0", "∞")],
				),
			),
			Self::Power => notation(
				"Power",
				"a^b",
				"",
				doc(
					"The base `a` raised to the exponent `b`, grouped from the right, so `a^b^c` is `a^(b^c)`. A negative `a` with a fractional `b` climbs into the complex plane. A matrix raised to an integer repeats it, and `A^-1` inverts it, which a matrix literal of fewer than four entries can't be, since it zeroes the parts it leaves out.",
					&[("2^10", "1024"), ("2^3^2", "512"), ("(-1)^0.5", "i")],
				),
			),
			Self::Transpose => notation(
				"Transpose",
				"A^T",
				"matrices",
				doc(
					"The matrix `A` with its rows and columns swapped. It's an error for a matrix with a translation.",
					&[("[i + 2j; 3i + 4j]^T", "[i + 3j; 2i + 4j]")],
				),
			),
			Self::Factorial => notation(
				"Factorial",
				"a!",
				"(−∞, ∞]",
				doc(
					"The product of every integer from 1 to `a`, extended to other numbers by the gamma function as Γ(a + 1). It's an error at the negative integers.",
					&[("5!", "120"), ("0.5!", "0.8862")],
				),
			),
			Self::Not => notation("Not", "!a", "bools", doc("The opposite of the bool `a`. Also spelled `¬a`.", &[("!false", "true")])),
			Self::Equal => notation(
				"Equal",
				"a == b",
				"bools",
				doc("Whether `a` equals `b`. A chain like `a == b == c` holds where each neighboring pair does.", &[("2 + 2 == 4", "true")]),
			),
			Self::NotEqual => notation(
				"Not Equal",
				"a != b",
				"bools",
				doc(
					"Whether `a` differs from `b`. Also spelled `a ≠ b`. A chain like `a != b != c` holds where no two of its values are equal.",
					&[("1 != 2", "true"), ("1 != 2 != 1", "false")],
				),
			),
			Self::Less => notation(
				"Less Than",
				"a < b",
				"bools",
				doc("Whether `a` is less than `b`. A chain like `0 < x < 1` holds where each neighboring pair does.", &[("1 < 2", "true")]),
			),
			Self::LessOrEqual => notation(
				"Less Than or Equal",
				"a <= b",
				"bools",
				doc(
					"Whether `a` is at most `b`. Also spelled `a ≤ b`. A chain like `0 <= x < 1` holds where each neighboring pair does.",
					&[("2 <= 2", "true")],
				),
			),
			Self::Greater => notation(
				"Greater Than",
				"a > b",
				"bools",
				doc(
					"Whether `a` is greater than `b`. A chain like `1 > x > 0` holds where each neighboring pair does.",
					&[("1 > 2", "false")],
				),
			),
			Self::GreaterOrEqual => notation(
				"Greater Than or Equal",
				"a >= b",
				"bools",
				doc(
					"Whether `a` is at least `b`. Also spelled `a ≥ b`. A chain like `1 >= x > 0` holds where each neighboring pair does.",
					&[("2 >= 3", "false")],
				),
			),
			Self::And => notation(
				"And",
				"a && b",
				"bools",
				doc("Whether both bools `a` and `b` are true. Also spelled `a ∧ b`.", &[("true && false", "false")]),
			),
			Self::Or => notation(
				"Or",
				"a || b",
				"bools",
				doc("Whether either bool `a` or `b` is true. Also spelled `a ∨ b`.", &[("true || false", "true")]),
			),
			Self::Define => notation(
				"Definition",
				"name = value",
				"",
				doc(
					"Names a value in a `where` clause, for the expression before it and the clause's other definitions. A function is defined with its parameters, like `f(t) = t^2`. Equality is written `==`.",
					&[("a + b where a = 1, b = 2", "3"), ("f(3) where f(t) = t^2", "9")],
				),
			),
			Self::Magnitude => notation(
				"Magnitude",
				"|x|",
				"[0, ∞]",
				doc(
					"How far `x` is from 0: its absolute value for a real number, and its length for any other value. `abs(x)` takes each part's absolute value instead.",
					&[("|-3|", "3"), ("|3i + 4j|", "5")],
				),
			),
		}
	}

	/// The operator's name, like `Add`.
	pub fn name(self) -> &'static str {
		self.doc().name
	}

	/// How the operator takes a value with a vector part, where the comparisons need real numbers and the factorial works in its
	/// operand's own plane.
	fn treatment(self) -> Treatment {
		match self {
			Self::Less | Self::LessOrEqual | Self::Greater | Self::GreaterOrEqual => Treatment::RealOnly,
			Self::Factorial => Treatment::InPlane,
			_ => Treatment::Whole,
		}
	}

	/// The hover text for the operator.
	pub fn tooltip(self) -> Tooltip {
		let NotationDoc { name, form, range, doc } = self.doc();
		let form = if range.is_empty() { form.to_string() } else { format!("{form} → {range}") };
		documented(name, form, &doc, self.treatment(), "")
	}
}

fn keyword_doc(keyword: &Token) -> Option<NotationDoc> {
	Some(match keyword {
		Token::Where => notation(
			"Where Clause",
			"… where x = …",
			"",
			doc(
				"Defines names for the expression before it, as `name = value` definitions separated by commas. A definition may use the others in any order, and a function is defined with its parameters, like `f(t) = t^2`. Within parentheses, the clause ends at the closing one.",
				&[("r^2 where r = d/2, d = 4", "4"), ("f(1) + f(2) where f(t) = t^2", "5")],
			),
		),
		Token::If => notation(
			"Piecewise Case",
			"{… if condition, …}",
			"",
			doc(
				"A case of a piecewise expression, whose value is taken where its condition holds. At most one case's condition may hold, and where none does, it's an error unless an `otherwise` case is given.",
				&[("{-x if x < 0, x otherwise} where x = -3", "3")],
			),
		),
		Token::Otherwise => notation(
			"Otherwise Case",
			"{…, … otherwise}",
			"",
			doc(
				"The case of a piecewise expression whose value is taken where no other case's condition holds.",
				&[("{1 if x > 0, 0 otherwise} where x = -2", "0")],
			),
		),
		_ => return None,
	})
}

/// The hover text for the keyword `where`, `if`, or `otherwise`.
pub fn keyword_tooltip(keyword: &Token) -> Option<Tooltip> {
	let NotationDoc { name, form, doc, .. } = keyword_doc(keyword)?;
	Some(documented(name, form.to_string(), &doc, Treatment::Whole, ""))
}

/// The hover text for a lone operator or variadic function name that applies across all of a node's items, like `+` or `max`,
/// written out over the items as `a`, `b`, `c`, and so on.
pub fn reducer_tooltip(source: &str) -> Option<Tooltip> {
	let spelling = source.trim();
	let operator_name = |op: BinaryOp| {
		let operator = match op {
			BinaryOp::Add => Operator::Add,
			BinaryOp::Sub => Operator::Subtract,
			BinaryOp::Mul => Operator::Multiply,
			BinaryOp::Div => Operator::Divide,
			BinaryOp::Pow => Operator::Power,
			BinaryOp::And => Operator::And,
			BinaryOp::Or => Operator::Or,
			BinaryOp::Lt => Operator::Less,
			BinaryOp::Leq => Operator::LessOrEqual,
			BinaryOp::Gt => Operator::Greater,
			BinaryOp::Geq => Operator::GreaterOrEqual,
			BinaryOp::Eq => Operator::Equal,
			BinaryOp::Neq => Operator::NotEqual,
		};
		operator.name()
	};

	let (name, form, summary) = match classify_reducer(source, NothingMap)? {
		Reducer::Function { .. } => {
			let name = spelling.strip_prefix('\\').unwrap_or(spelling);
			let function = BUILTIN_FUNCTIONS.iter().find(|function| function.name == name)?;
			let summary = format!("{} Here, they're all the items in order.", prose(function.doc.description, function.treatment));
			(function.title, Some(format!("{name}(a, b, c, …)")), summary)
		}
		Reducer::FoldLeft(op) => (
			operator_name(op),
			Some(format!("a {spelling} b {spelling} c {spelling} …")),
			format!("Combines all the items with `{spelling}`, in order and grouped from the left."),
		),
		Reducer::FoldRight(op) => (
			operator_name(op),
			Some(format!("a{spelling}b{spelling}c{spelling}…")),
			format!("Combines all the items with `{spelling}`, in order and grouped from the right."),
		),
		Reducer::ChainAdjacent(op) => (
			operator_name(op),
			Some(format!("a {spelling} b {spelling} c {spelling} …")),
			format!("Whether `{spelling}` holds between each item and the next."),
		),
		Reducer::ChainDistinct => (
			operator_name(BinaryOp::Neq),
			Some(format!("a {spelling} b {spelling} c {spelling} …")),
			"Whether every item differs from every other.".to_string(),
		),
	};

	Some(Tooltip {
		label: name.to_string(),
		form,
		description: summary,
		signature: None,
	})
}

/// The hover text for a matrix literal, drawing the matrix its entries make: each one a row where `;` separates them, and a column
/// where `,` does. An entry with a constant value fills in its parts, and any other stands whole across its row or down its column.
pub fn matrix_literal_tooltip(entries: &[&str], by_rows: bool) -> Tooltip {
	let summary = match (entries.len(), by_rows) {
		(1, _) => "A matrix of one row, reading a value's parts into its weight, like `[a] b` for the dot product.",
		(_, true) => "A matrix of rows, separated by `;`, each weighting a value's parts into one part of the result.",
		(_, false) => "A matrix of columns, separated by `,`, each where one basis direction goes.",
	};
	let entry_kind = match (entries.len(), by_rows) {
		(1, true) => "Row",
		(1, false) => "Column",
		(_, true) => "Rows",
		(_, false) => "Columns",
	};

	// The parts each entry lays out along: those of the rung the entry count names, and any a constant entry has
	let values = entries.iter().map(|entry| constant_value(entry)).collect::<Vec<_>>();
	let rung: &[usize] = match entries.len() {
		1 => &[0],
		2 => &[1, 2],
		3 => &[1, 2, 3],
		_ => &[0, 1, 2, 3],
	};
	let parts = (0..4)
		.filter(|&part| rung.contains(&part) || values.iter().flatten().any(|value| value.parts()[part] != 0.))
		.collect::<Vec<_>>();

	let lines = entries.iter().zip(&values).map(|(entry, value)| match value {
		Some(value) => Line::Cells(parts.iter().map(|&part| format_part(value.parts()[part])).collect()),
		None => Line::Span(span_text(entry)),
	});
	let lines = lines.collect::<Vec<_>>();
	let drawn = if by_rows { draw_rows(&lines, rung, &parts) } else { draw_columns(&lines, &parts, rung) };
	// The matrix is drawn once no entry is left blank to be typed, and only while it has no more entries than a value has parts
	let drawable = entries.len() <= PART_NAMES.len() && !entries.iter().any(|entry| entry.trim().is_empty());
	let drawing = drawable.then(|| format!("\n{}", code_block(&drawn)));

	// An arrow points at a row from the gap after its label, or at a column from the corner row below its label, after any label line
	let labels = drawn.len() - (if by_rows { entries.len() } else { parts.len() } + 2);
	let pointers = match by_rows {
		true => (0..entries.len())
			.map(|row| Pointer {
				line: labels + 1 + row,
				column: 2,
				arrow: '→',
			})
			.collect(),
		false => {
			let columns = drawn
				.first()
				.filter(|_| labels == 1)
				.map(|header| header.chars().enumerate().filter(|(_, character)| *character != ' ').map(|(column, _)| column));
			columns.into_iter().flatten().map(|column| Pointer { line: labels, column, arrow: '↓' }).collect()
		}
	};

	// Each entry is named by the part it fills, a row of the result or where a basis direction goes
	let signature = Signature {
		opening: "[".to_string(),
		parameters: (0..entries.len()).map(|index| rung.get(index).map_or("…", |&part| PART_NAMES[part])).collect(),
		separator: if by_rows { "; " } else { ", " },
		closing: "]",
		range: "",
		pointers: if drawable { pointers } else { Vec::new() },
		lines: Vec::new(),
	};

	Tooltip {
		label: format!("Matrix ({} {entry_kind})", entries.len()),
		form: Some(signature.text()),
		description: format!("{summary}{}", drawing.unwrap_or_default()),
		signature: Some(signature),
	}
}

/// The hover text for a range literal, drawing the map it makes, `p ↦ M p + c`, where both corners are constant.
pub fn range_tooltip(from: &str, to: &str) -> Tooltip {
	let summary = "The map taking `0` to `a` and `1` to `b` on each axis the corners span, a box for vector corners.";
	let (Some(a), Some(b)) = (constant_value(from), constant_value(to)) else {
		return Tooltip {
			label: "Range".to_string(),
			form: Some("a..b".to_string()),
			description: format!("{summary} It maps `p` to `a + (b - a) p`."),
			signature: None,
		};
	};

	// The spanned parts scale by the corners' difference and shift by the first corner, and the rest pass through
	let range = Matrix::range(a, b);
	let spanned = (0..4).filter(|&part| range.axes[part]).collect::<Vec<_>>();
	let linear = spanned
		.iter()
		.map(|&row| Line::Cells(spanned.iter().map(|&column| format_part(range.rows[row].parts()[column])).collect()));
	let translation = spanned.iter().map(|&row| Line::Cells(vec![format_part(range.translation.parts()[row])]));
	let linear = draw_rows(&linear.collect::<Vec<_>>(), &spanned, &spanned);
	let translation = draw_rows(&translation.collect::<Vec<_>>(), &[], &[]);

	// The translation's frame starts below the linear part's column labels
	let offset = linear.len() - translation.len();
	let middle = offset + (linear.len() - offset) / 2;
	let translation = std::iter::repeat_n(String::new(), offset).chain(translation);
	let lines = linear.iter().zip(translation).enumerate().map(|(index, (linear, translation))| {
		let joint = if index == middle { " p + " } else { "     " };
		format!("{linear}{joint}{translation}").trim_end().to_string()
	});
	let untouched = if spanned.len() < 4 { "\nThe other parts pass through unchanged." } else { "" };

	Tooltip {
		label: "Range".to_string(),
		form: Some("a..b".to_string()),
		description: format!("{summary}\n{}{untouched}", code_block(&lines.collect::<Vec<_>>())),
		signature: None,
	}
}

/// A description, led or ended by the sentence stating its treatment of a value with a vector part where that isn't whole.
fn prose(description: &str, treatment: Treatment) -> String {
	match treatment {
		Treatment::Whole => description.to_string(),
		Treatment::Componentwise => format!("Componentwise. {description}"),
		Treatment::RealOnly => format!("Accepts real-valued numbers. {description}"),
		Treatment::InPlane => format!("{description} Given a vector part, it works as on a complex number, with that part's direction as `i`."),
	}
}

/// The prose, a constant's value, the parameters, and the examples, each set apart by a blank line, leaving out those with nothing
/// to say, along with the line describing each parameter.
fn describe(prose: String, value: &str, parameters: &[Parameter], examples: &[(&str, &str)]) -> (String, Vec<usize>) {
	let leading = [Some(prose), (!value.is_empty()).then(|| format!("`{value}`"))];
	let leading = leading.into_iter().flatten().collect::<Vec<_>>();

	let arguments = parameters.iter().map(|parameter| format!("`{}` in `{}`: {}", parameter.name, parameter.domain, parameter.description));
	// A lone variadic parameter stands for every argument
	let several = parameters.len() > 1 || parameters.iter().any(|parameter| parameter.name == "…");
	let arguments = listed(if several { "Arguments:" } else { "Argument:" }, arguments);
	let listed_examples = examples.iter().map(|(expression, result)| format!("`{expression}` → `{result}`"));
	let examples = listed(if examples.len() == 1 { "Example:" } else { "Examples:" }, listed_examples);

	// The parameters follow their heading, which follows each leading section and the blank line after it
	let heading = leading.iter().map(|section| section.lines().count() + 1).sum::<usize>();
	let lines = (heading + 1..).take(parameters.len()).collect();

	(leading.into_iter().chain(arguments).chain(examples).collect::<Vec<_>>().join("\n\n"), lines)
}

/// The items as a bulleted list below their heading, or nothing where there are none.
fn listed(heading: &str, items: impl Iterator<Item = String>) -> Option<String> {
	let items = items.map(|item| format!("• {item}")).collect::<Vec<_>>();
	(!items.is_empty()).then(|| format!("{heading}\n{}", items.join("\n")))
}

/// The value an expression evaluates to on its own, where it needs no binding.
fn constant_value(source: &str) -> Option<Quaternion> {
	let object = Node::try_parse_from_str(source).ok()?.eval(&EvalContext::default()).ok()?;
	let Value::Number(number) = object.into_value()?;
	Some(number.to_quaternion())
}

/// A part of a drawn matrix, an integer where it is one and otherwise rounded to a few digits, in scientific notation where it's too
/// large or small to fit a short cell that way.
fn format_part(part: f64) -> String {
	// Trailing zeros after a decimal point say nothing at the precision shown
	let trimmed = |digits: &str| {
		if digits.contains('.') {
			digits.trim_end_matches('0').trim_end_matches('.').to_string()
		} else {
			digits.to_string()
		}
	};

	let magnitude = part.abs();
	let written = if magnitude.is_infinite() {
		"∞".to_string()
	} else if magnitude != 0. && !(1e-3..1e6).contains(&magnitude) {
		let scientific = format!("{magnitude:.3e}");
		let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
		format!("{}e{exponent}", trimmed(mantissa))
	} else if magnitude == magnitude.round() {
		format!("{magnitude}")
	} else {
		trimmed(&format!("{magnitude:.3}"))
	};
	if part < 0. { format!("−{written}") } else { written }
}

/// The longest a symbolic entry is drawn before it's cut short.
const SPAN_LENGTH_LIMIT: usize = 24;

/// A symbolic entry on one line, cut short with an ellipsis where it's too long to draw whole.
fn span_text(entry: &str) -> String {
	let text = entry.split_whitespace().collect::<Vec<_>>().join(" ");
	if text.chars().count() <= SPAN_LENGTH_LIMIT {
		return text;
	}
	format!("{}…", text.chars().take(SPAN_LENGTH_LIMIT - 1).collect::<String>().trim_end())
}

/// A row or column of a drawn matrix: its cells, or an entry spanning it whole.
enum Line {
	Cells(Vec<String>),
	Span(String),
}

/// The name of each part of a value, which labels a drawn matrix's rows and columns and a matrix literal's entries.
const PART_NAMES: [&str; 4] = ["w", "x", "y", "z"];

/// Draws rows with columns aligned and a spanning entry centered in a rule across its row, labeled by the parts they stand for.
fn draw_rows(rows: &[Line], row_parts: &[usize], column_parts: &[usize]) -> Vec<String> {
	let cells_of = |column: usize| rows.iter().filter_map(move |row| if let Line::Cells(cells) = row { cells.get(column) } else { None });
	let columns = rows.iter().map(|row| if let Line::Cells(cells) = row { cells.len() } else { 0 }).max().unwrap_or(0);
	let widths = (0..columns).map(|column| cells_of(column).map(|cell| cell.chars().count()).max().unwrap_or(1)).collect::<Vec<_>>();
	let spans = rows.iter().filter_map(|row| if let Line::Span(entry) = row { Some(entry.chars().count() + 4) } else { None });
	let width = (widths.iter().sum::<usize>() + 2 * widths.len().saturating_sub(1)).max(spans.max().unwrap_or(0));

	let inner = rows.iter().map(|row| match row {
		Line::Cells(cells) => {
			let cells = cells.iter().zip(&widths).map(|(cell, width)| format!("{cell:>width$}"));
			format!("{:<width$}", cells.collect::<Vec<_>>().join("  "))
		}
		Line::Span(entry) => {
			let rule = width - entry.chars().count() - 2;
			format!("{} {entry} {}", "─".repeat(rule / 2), "─".repeat(rule - rule / 2))
		}
	});
	let header = widths.iter().zip(column_parts).map(|(width, &part)| format!("{:>width$}", PART_NAMES[part]));
	bracket(header.collect(), inner.collect(), row_parts)
}

/// Draws columns side by side with a spanning entry midway down a rule in its column, labeled by the parts they stand for.
fn draw_columns(columns: &[Line], row_parts: &[usize], column_parts: &[usize]) -> Vec<String> {
	let rows = row_parts.len();
	let widths = columns.iter().map(|column| match column {
		Line::Cells(cells) => cells.iter().map(|cell| cell.chars().count()).max().unwrap_or(1),
		Line::Span(entry) => entry.chars().count(),
	});
	let widths = widths.collect::<Vec<_>>();

	let inner = (0..rows).map(|row| {
		let cells = columns.iter().zip(&widths).map(|(column, &width)| match column {
			Line::Cells(cells) => format!("{:>width$}", cells.get(row).map_or("", String::as_str)),
			Line::Span(entry) if row == (rows - 1) / 2 => entry.clone(),
			Line::Span(_) => format!("{:^width$}", "│"),
		});
		cells.collect::<Vec<_>>().join("  ")
	});
	let header = columns.iter().zip(&widths).zip(column_parts).map(|((column, &width), &part)| match column {
		Line::Cells(_) => format!("{:>width$}", PART_NAMES[part]),
		Line::Span(_) => format!("{:^width$}", PART_NAMES[part]),
	});
	bracket(header.collect(), inner.collect(), row_parts)
}

/// Encloses a matrix's rows in box-drawing brackets with their corners on rows of their own, labeling each row by its part down the
/// left and each column by the label in `header` above it, where either is given.
fn bracket(header: Vec<String>, rows: Vec<String>, row_parts: &[usize]) -> Vec<String> {
	let margin = if row_parts.is_empty() { "" } else { "   " };
	let blank = " ".repeat(rows.first().map_or(0, |row| row.chars().count()));
	let label = |index: usize| row_parts.get(index).map_or(margin.to_string(), |&part| format!("{}  ", PART_NAMES[part]));

	let header = (!header.is_empty()).then(|| format!("{margin}  {}", header.join("  ")).trim_end().to_string());
	let enclosed = rows.into_iter().enumerate().map(|(index, row)| format!("{}│ {row} │", label(index)));
	let frame = [format!("{margin}┌ {blank} ┐")].into_iter().chain(enclosed).chain([format!("{margin}└ {blank} ┘")]);
	header.into_iter().chain(frame).collect()
}

/// A fenced code block holding the lines, so they line up in monospace.
fn code_block(lines: &[String]) -> String {
	format!("```\n{}\n```", lines.join("\n"))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::object::Object;

	/// Whether two results agree to the few digits the documentation rounds them to, matrices entry by entry.
	fn agree(actual: &Object, expected: &Object) -> bool {
		let parts = |object: &Object| match object {
			Object::Value(Value::Number(number)) => number.to_quaternion().parts().to_vec(),
			Object::Matrix(matrix) => matrix.rows.iter().chain([&matrix.translation]).flat_map(|row| row.parts()).collect(),
		};
		let close = |actual: f64, expected: f64| {
			if expected.is_infinite() {
				actual == expected
			} else {
				(actual - expected).abs() <= 1e-3 * expected.abs().max(1.)
			}
		};

		let (actual, expected) = (parts(actual), parts(expected));
		actual.len() == expected.len() && actual.iter().zip(&expected).all(|(actual, expected)| close(*actual, *expected))
	}

	fn check(examples: &[(&str, &str)]) {
		for (expression, result) in examples {
			let actual = crate::evaluate(expression).unwrap_or_else(|error| panic!("`{expression}` failed to parse: {error}"));
			let actual = actual.unwrap_or_else(|error| panic!("`{expression}` failed to evaluate: {error:?}"));
			let expected = crate::evaluate(result).unwrap().unwrap();
			assert!(agree(&actual, &expected), "`{expression}` is {actual}, not {result}");
		}
	}

	#[test]
	fn every_example_evaluates_to_its_stated_result() {
		BUILTIN_FUNCTIONS.iter().for_each(|function| check(function.doc.examples));
		crate::lexer::CONSTANT_SPELLINGS.into_iter().for_each(|(_, constant)| check(constant_doc(constant).doc.examples));
		Operator::ALL.into_iter().for_each(|operator| check(operator.doc().doc.examples));
		crate::lexer::KEYWORDS.iter().for_each(|(_, keyword)| check(keyword_doc(keyword).unwrap().doc.examples));
		check(IDENTITY.examples);
	}

	#[test]
	fn every_treatment_matches_how_the_function_evaluates() {
		let evaluate = |expression: &str| crate::evaluate(expression).ok()?.ok();
		let number = |expression: &str| match evaluate(expression) {
			Some(Object::Value(Value::Number(number))) => number.to_quaternion(),
			result => panic!("`{expression}` should be a number, not {result:?}"),
		};
		let close = |actual: Quaternion, expected: Quaternion| actual.parts().iter().zip(expected.parts()).all(|(actual, expected)| (actual - expected).abs() <= 1e-9);

		// Arguments following the value in each call, first as given beside a whole value, then beside each of its parts alone
		let following = |name: &str| match name {
			"snap" => (", 0.5", ", 0.5"),
			"mod" => (", 1.5", ", 1.5"),
			"min" | "max" => (", 0.5 + 0.5i + 0.5j + 0.5k", ", 0.5"),
			"root" => (", 3", ", 3"),
			"choose" | "pick" => (", 2", ", 2"),
			"atan2" | "median" => (", 0.25", ", 0.25"),
			"mode" => (", 0.3", ", 0.3"),
			_ => ("", ""),
		};

		for function in BUILTIN_FUNCTIONS {
			let name = function.name;
			let (whole, alone) = following(name);
			match function.treatment {
				Treatment::Whole => {}
				// Each part of the result is the function of the matching part alone
				Treatment::Componentwise => {
					let parts = [0.3, 1.7, -2.2, 0.6];
					let result = number(&format!("{name}(0.3 + 1.7i - 2.2j + 0.6k{whole})"));
					let expected = parts.map(|part| number(&format!("{name}({part}{alone})")).w);
					assert!(close(result, Quaternion::from_parts(expected)), "`{name}` should act on each part apart");
				}
				// Along `j`, the result is the one along `i` turned onto `j`
				Treatment::InPlane => {
					let along_i = number(&format!("{name}(0.3 + 0.7i{whole})"));
					let along_j = number(&format!("{name}(0.3 + 0.7j{whole})"));
					assert!(close(along_j, Quaternion::new(along_i.w, 0., along_i.x, 0.)), "`{name}` should work in its argument's own plane");
				}
				// A real argument is taken, and one with a vector part is refused
				Treatment::RealOnly => {
					assert!(evaluate(&format!("{name}(0.3{whole})")).is_some(), "`{name}` should take a real number");
					assert!(evaluate(&format!("{name}(0.3 + i{whole})")).is_none(), "`{name}` should refuse a vector part");
				}
			}
		}

		// The comparisons refuse a vector part just the same, and the factorial works in its operand's own plane
		let comparisons = [Operator::Less, Operator::LessOrEqual, Operator::Greater, Operator::GreaterOrEqual];
		for (operator, spelling) in comparisons.into_iter().zip(["<", "<=", ">", ">="]) {
			assert_eq!(operator.treatment(), Treatment::RealOnly);
			assert!(evaluate(&format!("0.3 {spelling} 0.5")).is_some() && evaluate(&format!("0.3 + i {spelling} 0.5")).is_none());
		}
		assert_eq!(Operator::Factorial.treatment(), Treatment::InPlane);
		let (along_i, along_j) = (number("(0.3 + 0.7i)!"), number("(0.3 + 0.7j)!"));
		assert!(close(along_j, Quaternion::new(along_i.w, 0., along_i.x, 0.)), "the factorial should work in its operand's own plane");
	}

	#[test]
	fn a_description_sets_apart_its_prose_arguments_and_examples() {
		let sqrt = function_tooltip("sqrt").unwrap();
		assert_eq!(sqrt.form.as_deref(), Some("sqrt(x) → [0, ∞]"));
		assert_eq!(
			sqrt.description,
			"The nonnegative number whose square is `x`. A negative `x` climbs to the imaginary axis. Given a vector part, it works as on a complex number, with that part's direction as `i`.\n\n\
			Argument:\n• `x` in `[0, ∞]`: the value\n\n\
			Examples:\n• `sqrt(9)` → `3`\n• `sqrt(-4)` → `2i`"
		);

		// A treatment other than a whole or in-plane one leads the prose, and a lone variadic parameter still has the plural heading
		let max = function_tooltip("max").unwrap().description;
		assert!(max.starts_with("Componentwise. The greatest of its arguments.\n\nArguments:\n• `…` in `[−∞, ∞]`: the values"));
		assert!(Operator::Less.tooltip().description.starts_with("Accepts real-valued numbers. Whether `a` is less than `b`."));
		assert_eq!(Operator::Less.tooltip().form.as_deref(), Some("a < b → bools"));
		assert_eq!(Operator::Add.tooltip().form.as_deref(), Some("a + b"));

		// A constant's value comes between its prose and its examples
		assert_eq!(constant_tooltip(Constant::I).description.split("\n\n").nth(1), Some("`i = √−1`"));
	}

	#[test]
	fn a_function_is_documented_by_every_name_reaching_it() {
		let log = function_tooltip("log").unwrap();
		assert_eq!(log.label, "Logarithm");
		assert_eq!(log.form.as_deref(), Some("log(x, b?) → [−∞, ∞]"));
		assert!(function_tooltip("nothing").is_none());

		// The signature the form shows is given apart too, with the description line of each parameter, for an editor to mark the one being typed
		let signature = |name: &str, parameters: &[&'static str], range: &'static str, lines: &[usize]| {
			Some(Signature {
				opening: format!("{name}("),
				parameters: parameters.to_vec(),
				separator: ", ",
				closing: ")",
				range,
				pointers: Vec::new(),
				lines: lines.to_vec(),
			})
		};
		assert_eq!(log.signature, signature("log", &["x", "b?"], "[−∞, ∞]", &[3, 4]));
		assert_eq!(log.description.lines().nth(4), Some("• `b?` in `[0, ∞]`: the base, 10 if omitted"));
		assert_eq!(function_tooltip("max").unwrap().signature, signature("max", &["…"], "[−∞, ∞]", &[3]));
		assert_eq!(Operator::Add.tooltip().signature, None);

		// A suffixed form is its function called without the parameter its suffix sets
		let log_2 = function_tooltip("log_2").unwrap();
		assert_eq!(log_2.label, log.label);
		assert_eq!(log_2.form.as_deref(), Some("log_2(x) → [−∞, ∞]"));
		assert_eq!(log_2.signature, signature("log_2", &["x"], "[−∞, ∞]", &[3]));
		assert!(!log_2.description.contains("`b?`"));
		assert!(
			log_2.description.contains("`log_1.5(x)`. Here, `b` is 2. Given a vector part"),
			"the note follows the description it qualifies"
		);
		assert_eq!(function_tooltip("root3").unwrap().signature, signature("root3", &["x"], "[0, ∞]", &[3]));
		assert!(function_tooltip("root_2.5").unwrap().description.contains("Here, `n` is 2.5."));
		assert!(function_tooltip("log010").unwrap().description.contains("Here, `b` is 10."), "the suffix is read as its number");

		// The bare form before its suffix is written is called without the parameter too, which it leaves unset
		let log_ = function_tooltip("log_").unwrap();
		assert_eq!(log_.form.as_deref(), Some("log_(x) → [−∞, ∞]"));
		assert!(!log_.description.contains("`b?`") && !log_.description.contains("Here,"));
	}

	#[test]
	fn a_matrix_literal_draws_its_entries_as_rows_or_columns() {
		let drawn = |entries: &[&str], by_rows: bool| {
			let description = matrix_literal_tooltip(entries, by_rows).description;
			description.lines().skip(2).take_while(|line| *line != "```").map(str::to_string).collect::<Vec<_>>()
		};

		let two_by_two = ["     x  y", "   ┌      ┐", "x  │ 1  2 │", "y  │ 3  4 │", "   └      ┘"];
		assert_eq!(drawn(&["i + 2j", "3i + 4j"], true), two_by_two);
		assert_eq!(drawn(&["i + 3j", "2i + 4j"], false), two_by_two);

		// The label counts the rows or columns the entries are given as
		assert_eq!(matrix_literal_tooltip(&["i + 2j", "3i + 4j"], true).label, "Matrix (2 Rows)");
		assert_eq!(matrix_literal_tooltip(&["u", "v", "w"], false).label, "Matrix (3 Columns)");
		assert_eq!(matrix_literal_tooltip(&["1"], true).label, "Matrix (1 Row)");

		// Its form has the literal's shape with each entry named by the part it fills, while a blank entry leaves it undrawn
		assert_eq!(matrix_literal_tooltip(&["1", "2", "3"], true).form.as_deref(), Some("[x; y; z]"));
		assert_eq!(matrix_literal_tooltip(&["1"], true).form.as_deref(), Some("[w]"));
		assert_eq!(matrix_literal_tooltip(&["1", " "], false).form.as_deref(), Some("[x, y]"));
		assert!(!matrix_literal_tooltip(&["1", " "], false).description.contains("```"));
		assert!(!matrix_literal_tooltip(&["1", "2", "3", "4", "5"], true).description.contains("```"), "a value has only four parts");

		// An arrow points at each entry's row from the gap after its label, or at its column from the corner row below its label
		let pointers = |entries: &[&str], by_rows: bool| {
			let signature = matrix_literal_tooltip(entries, by_rows).signature.unwrap();
			signature.pointers.iter().map(|pointer| (pointer.line, pointer.column, pointer.arrow)).collect::<Vec<_>>()
		};
		assert_eq!(pointers(&["i + 2j", "3i + 4j"], true), [(2, 2, '→'), (3, 2, '→')]);
		assert_eq!(pointers(&["i + 3j", "2i + 4j"], false), [(1, 5, '↓'), (1, 8, '↓')]);
		assert!(pointers(&["1", " "], false).is_empty(), "an undrawn matrix has nothing to point into");

		// An entry without a constant value spans its row or column
		assert_eq!(drawn(&["u", "v"], true), ["   ┌       ┐", "x  │ ─ u ─ │", "y  │ ─ v ─ │", "   └       ┘"]);
		assert_eq!(drawn(&["u", "v"], false), ["     x  y", "   ┌      ┐", "x  │ u  v │", "y  │ │  │ │", "   └      ┘"]);

		// A constant entry's parts off the rung its count names are drawn too
		assert_eq!(drawn(&["1", "i"], true), ["     w  x  y", "   ┌         ┐", "x  │ 1  0  0 │", "y  │ 0  1  0 │", "   └         ┘"]);
		assert_eq!(
			drawn(&["-0.5i", "2j/3"], false),
			["        x      y", "   ┌             ┐", "x  │ −0.5      0 │", "y  │    0  0.667 │", "   └             ┘"]
		);

		// A part too large or small for a short cell is written in scientific notation, and a long entry is cut short on one line
		assert_eq!(
			drawn(&["1e20", "-0.00001"], true),
			["         w  x  y", "   ┌             ┐", "x  │  1e20  0  0 │", "y  │ −1e-5  0  0 │", "   └             ┘"]
		);
		let long = drawn(&["a +\n b", "the_first_variable + the_second_variable"], true);
		assert_eq!(long[1], "x  │ ────────── a + b ─────────── │");
		assert_eq!(long[2], "y  │ ─ the_first_variable + th… ─ │");
	}

	#[test]
	fn a_range_draws_the_map_its_corners_make() {
		let drawn = |from: &str, to: &str| range_tooltip(from, to).description.lines().skip(2).collect::<Vec<_>>().join("\n");

		assert_eq!(
			drawn("0", "(3i + 4j)"),
			"     x  y\n   ┌      ┐     ┌   ┐\nx  │ 3  0 │     │ 0 │\ny  │ 0  4 │ p + │ 0 │\n   └      ┘     └   ┘\n```\nThe other parts pass through unchanged."
		);
		assert_eq!(
			drawn("2", "5"),
			"     w\n   ┌   ┐     ┌   ┐\nw  │ 3 │ p + │ 2 │\n   └   ┘     └   ┘\n```\nThe other parts pass through unchanged."
		);
		assert!(range_tooltip("a", "b").description.contains("`a + (b - a) p`"));
		assert_eq!(range_tooltip("0", "1").form.as_deref(), Some("a..b"));
	}
}
