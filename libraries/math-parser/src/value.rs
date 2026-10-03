use crate::ast::{BinaryOp, UnaryOp};
use crate::quaternion::Quaternion;
use std::cmp::Ordering;
use std::f64::consts::PI;
use std::ops::Mul;

pub type Complex = num_complex::Complex<f64>;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value {
	Number(Number),
}

/// Generates accessors reading the value as the target integer type: exactly from integer storage, and otherwise rounded to the nearest whole number.
macro_rules! integer_accessors {
	($($fn_name:ident: $int:ty),* $(,)?) => {
		$(
			#[doc = concat!("Reads the value as a `", stringify!($int), "`, rounded to the nearest whole number, or `None` if it isn't a real number, isn't finite, or lies outside the type's range.")]
			pub fn $fn_name(&self) -> Option<$int> {
				let Self::Number(number) = self;
				if let Number::Integer(integer) = number {
					return <$int>::try_from(*integer).ok();
				}

				let rounded = number.as_real()?.round();
				// `MAX + 1` is the power of two past the type, exact or absorbed by rounding, so the bound stays exclusive where `MAX` itself rounds up to it
				(rounded.is_finite() && rounded >= <$int>::MIN as f64 && rounded < <$int>::MAX as f64 + 1.).then_some(rounded as $int)
			}
		)*
	};
}

impl Value {
	pub fn from_f64(x: f64) -> Self {
		Self::Number(Number::Real(x))
	}

	/// Wraps an integer exactly, so integer arithmetic on it stays exact beyond the reals' 2^53 limit.
	pub fn from_i64(x: i64) -> Self {
		Self::Number(Number::Integer(x))
	}

	/// Wraps a truth value as the number 1 or 0, the language's representation of booleans.
	pub fn from_bool(x: bool) -> Self {
		Self::Number(Number::from_bool(x))
	}

	pub fn as_real(&self) -> Option<f64> {
		let Self::Number(number) = self;
		number.as_real()
	}

	/// Reads the value as a single-precision float, or `None` if it isn't a real number.
	pub fn as_f32(&self) -> Option<f32> {
		self.as_real().map(|real| real as f32)
	}

	/// Reads the value as a truth value, or `None` unless it is exactly 0 or 1.
	pub fn as_bool(&self) -> Option<bool> {
		let Self::Number(number) = self;
		number.as_bool()
	}

	/// The lowest rung of the number ladder that holds the value losslessly, so a host can choose the right query for it.
	pub fn rung(&self) -> Rung {
		let Self::Number(number) = self;
		number.rung()
	}

	/// The value's full quaternion form, which every value has.
	pub fn as_particle3(&self) -> Particle3 {
		let Self::Number(number) = self;
		let Quaternion { w, x, y, z } = number.to_quaternion();
		Weighted { w, vector: Vector3([x, y, z]) }
	}

	/// Reads the value as a weight with a displacement in the `xy` plane, or `None` if it has a `k` part.
	pub fn as_particle2(&self) -> Option<Particle2> {
		let Weighted { w, vector: Vector3([x, y, z]) } = self.as_particle3();
		(z == 0.).then_some(Weighted { w, vector: Vector2([x, y]) })
	}

	/// Reads the value as a complex number, a weight with a displacement along `x`, or `None` if it has a `j` or `k` part.
	pub fn as_particle1(&self) -> Option<Particle1> {
		let Weighted { w, vector: Vector2([x, y]) } = self.as_particle2()?;
		(y == 0.).then_some(Weighted { w, vector: Vector1(x) })
	}

	/// Reads the value as a weightless displacement in `xyz` space, or `None` if it has a weight.
	pub fn as_vector3(&self) -> Option<Vector3> {
		let Weighted { w, vector } = self.as_particle3();
		(w == 0.).then_some(vector)
	}

	/// Reads the value as a weightless displacement in the `xy` plane, or `None` if it has a weight or a `k` part.
	pub fn as_vector2(&self) -> Option<Vector2> {
		let Weighted { w, vector } = self.as_particle2()?;
		(w == 0.).then_some(vector)
	}

	/// Reads the value as a weightless displacement along `x`, or `None` if it has a weight or a `j` or `k` part.
	pub fn as_vector1(&self) -> Option<Vector1> {
		let Weighted { w, vector } = self.as_particle1()?;
		(w == 0.).then_some(vector)
	}

	integer_accessors! {
		as_u8: u8,
		as_u16: u16,
		as_u32: u32,
		as_u64: u64,
		as_u128: u128,
		as_i8: i8,
		as_i16: i16,
		as_i32: i32,
		as_i64: i64,
		as_i128: i128,
	}
}

impl From<f64> for Value {
	fn from(x: f64) -> Self {
		Self::from_f64(x)
	}
}

impl From<i64> for Value {
	fn from(x: i64) -> Self {
		Self::from_i64(x)
	}
}

impl From<Complex> for Value {
	fn from(complex: Complex) -> Self {
		Self::Number(Number::Complex(complex).canonical())
	}
}

impl From<Quaternion> for Value {
	fn from(quaternion: Quaternion) -> Self {
		Self::Number(Number::Quaternion(quaternion).canonical())
	}
}

impl From<Vector1> for Value {
	fn from(Vector1(x): Vector1) -> Self {
		Self::from(Quaternion::new(0., x, 0., 0.))
	}
}

impl From<Vector2> for Value {
	fn from(Vector2([x, y]): Vector2) -> Self {
		Self::from(Quaternion::new(0., x, y, 0.))
	}
}

impl From<Vector3> for Value {
	fn from(Vector3([x, y, z]): Vector3) -> Self {
		Self::from(Quaternion::new(0., x, y, z))
	}
}

impl From<Particle1> for Value {
	fn from(Weighted { w, vector: Vector1(x) }: Particle1) -> Self {
		Self::from(Quaternion::new(w, x, 0., 0.))
	}
}

impl From<Particle2> for Value {
	fn from(Weighted { w, vector: Vector2([x, y]) }: Particle2) -> Self {
		Self::from(Quaternion::new(w, x, y, 0.))
	}
}

impl From<Particle3> for Value {
	fn from(Weighted { w, vector: Vector3([x, y, z]) }: Particle3) -> Self {
		Self::from(Quaternion::new(w, x, y, z))
	}
}

/// A weightless displacement along `x`: the `i` part alone.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector1(pub f64);

/// A weightless displacement in the `xy` plane: the `i` and `j` parts.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector2(pub [f64; 2]);

/// A weightless displacement in `xyz` space: the `i`, `j`, and `k` parts.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector3(pub [f64; 3]);

/// A displacement with a weight, the ladder's particle rungs, of which the plain vectors are the weightless refinements.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Weighted<V> {
	/// The real part, not a homogeneous coordinate.
	pub w: f64,
	pub vector: V,
}

/// The complex numbers: a weight with a displacement along `x`.
pub type Particle1 = Weighted<Vector1>;
/// A weight with a displacement in the `xy` plane.
pub type Particle2 = Weighted<Vector2>;
/// The quaternions: a weight with a displacement in `xyz` space.
pub type Particle3 = Weighted<Vector3>;

impl core::fmt::Display for Value {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Value::Number(num) => num.fmt(f),
		}
	}
}

/// A rung of the number ladder, Bool ⊂ Integer ⊂ Number ⊂ Particle1 ⊂ Particle2 ⊂ Particle3: each is the set of values
/// whose remaining parts are zero, and the vector rungs are the weightless refinements of the particle rungs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rung {
	/// Exactly 0 or 1.
	Bool,
	/// A whole number.
	Integer,
	/// A real number, which Graphite labels Number.
	Number,
	/// A weightless `i` part alone: a displacement along `x`.
	Vector1,
	/// A real part and an `i` part: the complex numbers.
	Particle1,
	/// Weightless `i` and `j` parts: a displacement in the `xy` plane.
	Vector2,
	/// A real part with `i` and `j` parts.
	Particle2,
	/// Weightless `i`, `j`, and `k` parts: a displacement in `xyz` space.
	Vector3,
	/// All four parts: the quaternions.
	Particle3,
}

/// A number's storage form, an optimization that is never observable: behavior is decided by the number's mathematical
/// content alone, so `2`, `2.0`, and `2 + 0i` are one value.
#[derive(Debug, Clone, Copy)]
pub enum Number {
	/// A whole number stored exactly, so integer arithmetic stays exact beyond the reals' 2^53 limit.
	Integer(i64),
	Real(f64),
	Complex(Complex),
	Quaternion(Quaternion),
}

impl PartialEq for Number {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Number::Integer(lhs), Number::Integer(rhs)) => lhs == rhs,
			(Number::Real(lhs), Number::Real(rhs)) => lhs == rhs,
			(Number::Integer(integer), Number::Real(real)) | (Number::Real(real), Number::Integer(integer)) => compare_integer_real(*integer, *real) == Some(Ordering::Equal),
			// A number with a vector part equals a real number only when that part is zero, and then the way its real part does
			(vector @ (Number::Complex(_) | Number::Quaternion(_)), real @ (Number::Integer(_) | Number::Real(_)))
			| (real @ (Number::Integer(_) | Number::Real(_)), vector @ (Number::Complex(_) | Number::Quaternion(_))) => {
				let quaternion = vector.to_quaternion();
				quaternion.is_real() && Number::Real(quaternion.w) == *real
			}
			_ => self.to_quaternion() == other.to_quaternion(),
		}
	}
}

/// Writes a real as the language spells it, with `∞` for an infinity rather than Rust's `inf`.
pub(crate) fn fmt_real(real: f64, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
	if real.is_infinite() {
		f.pad(if real > 0. { "∞" } else { "-∞" })
	} else {
		std::fmt::Display::fmt(&real, f)
	}
}

impl std::fmt::Display for Number {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Number::Integer(integer) => integer.fmt(f),
			Number::Real(real) => fmt_real(*real, f),
			Number::Complex(complex) => Quaternion::from_complex(*complex).fmt(f),
			Number::Quaternion(quaternion) => quaternion.fmt(f),
		}
	}
}

impl Number {
	pub fn from_f64(x: f64) -> Self {
		Self::Real(x)
	}

	/// The number 1 or 0, the language's representation of a truth value.
	pub fn from_bool(x: bool) -> Self {
		Self::Integer(x as i64)
	}

	/// Stores a real in integer form when it is whole and fits, since a whole number is an integer whatever computed it.
	pub(crate) fn real_or_integer(real: f64) -> Self {
		// The saturating cast round-trips exactly for a whole real in range, except 2^63, which saturates to `i64::MAX` and rounds back up
		let integer = real as i64;
		if integer as f64 == real && real < i64::MAX as f64 {
			Number::Integer(integer)
		} else {
			Number::Real(real)
		}
	}

	/// Reads the number as a real, or `None` if it has a vector part.
	pub fn as_real(self) -> Option<f64> {
		match self {
			Number::Integer(integer) => Some(integer as f64),
			Number::Real(real) => Some(real),
			// Canonical form stores a number whose other parts are zero as a real, so a canonical complex number or quaternion is never real
			Number::Complex(_) | Number::Quaternion(_) => None,
		}
	}

	/// Reads the number as a complex number, or `None` if it has a `j` or `k` part.
	pub fn as_complex(self) -> Option<Complex> {
		match self {
			Number::Complex(complex) => Some(complex),
			Number::Quaternion(quaternion) if quaternion.y == 0. && quaternion.z == 0. => Some(Complex::new(quaternion.w, quaternion.x)),
			Number::Quaternion(_) => None,
			real => Some(Complex::new(real.as_real()?, 0.)),
		}
	}

	/// Widens the number to its full quaternion form, since every value is a quaternion whose remaining parts are zero.
	pub fn to_quaternion(self) -> Quaternion {
		match self {
			Number::Integer(integer) => Quaternion::new(integer as f64, 0., 0., 0.),
			Number::Real(real) => Quaternion::new(real, 0., 0., 0.),
			Number::Complex(complex) => Quaternion::from_complex(complex),
			Number::Quaternion(quaternion) => quaternion,
		}
	}

	/// The truth value of a logical operand, which must be exactly 0 or 1: any other number is not a truth value, so logic on it is an error rather than a guess.
	pub fn as_bool(self) -> Option<bool> {
		match self {
			Number::Integer(0) | Number::Real(0.) => Some(false),
			Number::Integer(1) | Number::Real(1.) => Some(true),
			_ => None,
		}
	}

	/// The lowest rung of the number ladder that holds the number losslessly.
	pub fn rung(self) -> Rung {
		match self {
			Number::Integer(0 | 1) => return Rung::Bool,
			Number::Integer(_) => return Rung::Integer,
			_ => {}
		}

		let Quaternion { w, x, y, z } = self.to_quaternion();
		let highest_axis = [x, y, z].iter().rposition(|part| *part != 0.).map_or(0, |index| index + 1);
		match (w == 0., highest_axis) {
			(_, 0) if w == 0. || w == 1. => Rung::Bool,
			(_, 0) if w.fract() == 0. => Rung::Integer,
			(_, 0) => Rung::Number,
			(true, 1) => Rung::Vector1,
			(false, 1) => Rung::Particle1,
			(true, 2) => Rung::Vector2,
			(false, 2) => Rung::Particle2,
			(true, _) => Rung::Vector3,
			(false, _) => Rung::Particle3,
		}
	}

	/// The number's canonical form: zero-valued higher parts are dropped, since `n + 0i` is exactly `n`, a signed zero is
	/// plain zero, and a whole real takes integer storage, so no zero-valued part or storage form can ever change a result.
	#[inline(always)]
	pub fn canonical(self) -> Number {
		let unsigned_zero = |x: f64| if x == 0. { 0. } else { x };
		match self {
			Number::Integer(_) => self,
			// A signed zero is whole, so it becomes the integer 0
			Number::Real(real) => Number::real_or_integer(real),
			Number::Complex(complex) if complex.im == 0. => Number::real_or_integer(complex.re),
			Number::Complex(complex) => Number::Complex(Complex::new(unsigned_zero(complex.re), unsigned_zero(complex.im))),
			Number::Quaternion(Quaternion { w, x: 0., y: 0., z: 0. }) => Number::real_or_integer(w),
			Number::Quaternion(Quaternion { w, x, y: 0., z: 0. }) => Number::Complex(Complex::new(unsigned_zero(w), unsigned_zero(x))),
			Number::Quaternion(quaternion) => Number::Quaternion(quaternion.map(unsigned_zero)),
		}
	}

	/// Whether any part is NaN, which no operation may produce: an indeterminate form is an evaluation error instead.
	pub fn is_nan(self) -> bool {
		match self {
			Number::Integer(_) => false,
			Number::Real(real) => real.is_nan(),
			Number::Complex(complex) => complex.re.is_nan() || complex.im.is_nan(),
			Number::Quaternion(quaternion) => quaternion.parts().iter().any(|part| part.is_nan()),
		}
	}

	/// The Euclidean magnitude over every part: the absolute value on the reals.
	pub fn magnitude(self) -> f64 {
		match self {
			Number::Integer(integer) => integer.unsigned_abs() as f64,
			Number::Real(real) => real.abs(),
			Number::Complex(complex) => complex.norm(),
			Number::Quaternion(quaternion) => quaternion.norm(),
		}
	}

	/// Applies a function to every part.
	pub(crate) fn map_parts(self, function: impl Fn(f64) -> f64) -> Number {
		match self {
			Number::Integer(integer) => Number::Real(function(integer as f64)),
			Number::Real(real) => Number::Real(function(real)),
			Number::Complex(complex) => Number::Complex(Complex::new(function(complex.re), function(complex.im))),
			Number::Quaternion(quaternion) => Number::Quaternion(quaternion.map(function)),
		}
	}

	/// Rounds every part with `function`, passing an integer through since it is already whole.
	pub(crate) fn round_parts(self, function: impl Fn(f64) -> f64) -> Number {
		match self {
			Number::Integer(_) => self,
			_ => self.map_parts(function),
		}
	}

	pub fn binary_op(self, op: BinaryOp, other: Number) -> Option<Number> {
		// Logic and equality work uniformly across every rung
		match op {
			BinaryOp::And | BinaryOp::Or => {
				let (Some(lhs), Some(rhs)) = (self.as_bool(), other.as_bool()) else { return None };
				let result = if matches!(op, BinaryOp::And) { lhs && rhs } else { lhs || rhs };
				return Some(Number::from_bool(result));
			}
			BinaryOp::Eq | BinaryOp::Neq => return Some(Number::from_bool((self == other) != matches!(op, BinaryOp::Neq))),
			_ => {}
		}

		// Two integers compute exactly, falling back to the reals only when the result overflows integer storage, and any
		// other pair computes in the widest storage either operand needs
		let (lhs, rhs) = match (self, other) {
			(Number::Real(lhs), Number::Real(rhs)) => (lhs, rhs),
			(Number::Integer(lhs), Number::Integer(rhs)) => match integer_binary_op(lhs, op, rhs) {
				Some(result) => return Some(result),
				None => (lhs as f64, rhs as f64),
			},
			(Number::Quaternion(_), _) | (_, Number::Quaternion(_)) => return quaternion_binary_op(self.to_quaternion(), op, other.to_quaternion()),
			(Number::Complex(_), _) | (_, Number::Complex(_)) => return complex_binary_op(self.as_complex()?, op, other.as_complex()?),
			// An integer beside a real widens for arithmetic, but orders exactly since widening rounds past 2^53
			_ => {
				if let Some(accepted) = comparison(op) {
					return Some(Number::from_bool(self.real_ordering(other).is_some_and(accepted)));
				}
				(self.as_real()?, other.as_real()?)
			}
		};
		// One call site keeps the real arithmetic inlined even where optimizing for size
		real_binary_op(lhs, op, rhs)
	}

	/// Orders two real numbers by value, exactly across integer and real storage, or `None` when either has a vector part or is NaN, since ordering is real-only.
	#[inline]
	pub(crate) fn real_ordering(self, other: Number) -> Option<Ordering> {
		match (self, other) {
			(Number::Real(lhs), Number::Real(rhs)) => lhs.partial_cmp(&rhs),
			(Number::Integer(lhs), Number::Integer(rhs)) => Some(lhs.cmp(&rhs)),
			(Number::Integer(integer), Number::Real(real)) => compare_integer_real(integer, real),
			(Number::Real(real), Number::Integer(integer)) => compare_integer_real(integer, real).map(Ordering::reverse),
			_ => None,
		}
	}

	pub fn unary_op(self, op: UnaryOp) -> Option<Number> {
		match op {
			UnaryOp::Pos => Some(self),
			UnaryOp::Neg => Some(match self {
				Number::Integer(integer) => integer.checked_neg().map_or(Number::Real(-(integer as f64)), Number::Integer),
				Number::Real(real) => Number::Real(-real),
				Number::Complex(complex) => Number::Complex(-complex),
				Number::Quaternion(quaternion) => Number::Quaternion(-quaternion),
			}),
			UnaryOp::Not => self.as_bool().map(|boolean| Number::from_bool(!boolean)),
			UnaryOp::Transpose => None,
			UnaryOp::Magnitude => Some(match self {
				Number::Integer(integer) => i64::try_from(integer.unsigned_abs()).map_or(Number::Real(integer.unsigned_abs() as f64), Number::Integer),
				_ => Number::Real(self.magnitude()),
			}),
			UnaryOp::Fac => {
				// Exact while the product fits integer storage, which 21! overflows
				const EXACT_FACTORIAL_LIMIT: i64 = 20;
				match self.canonical() {
					Number::Integer(whole @ 0..=EXACT_FACTORIAL_LIMIT) => Some(Number::Integer((1..=whole).product())),
					Number::Integer(whole) => real_factorial(whole as f64).map(Number::Real),
					Number::Real(real) => real_factorial(real).map(Number::Real),
					Number::Complex(complex) => Some(Number::Complex(complex_gamma(complex + 1.))),
					Number::Quaternion(quaternion) => Some(Number::Quaternion(quaternion.in_plane(|z| complex_gamma(z + 1.)))),
				}
			}
		}
	}
}

/// Exact integer arithmetic, or `None` when the result overflows integer storage, leaves it (a fractional quotient or
/// a negative power), or has no answer (a zero divisor), each of which the real form settles.
fn integer_binary_op(lhs: i64, op: BinaryOp, rhs: i64) -> Option<Number> {
	let result = match op {
		BinaryOp::Add => lhs.checked_add(rhs)?,
		BinaryOp::Sub => lhs.checked_sub(rhs)?,
		// Factors within 32 bits cannot overflow, skipping the 128-bit multiply that detects overflow on wasm
		BinaryOp::Mul if i32::try_from(lhs).is_ok() && i32::try_from(rhs).is_ok() => lhs * rhs,
		BinaryOp::Mul => lhs.checked_mul(rhs)?,
		BinaryOp::Div => {
			if rhs == 0 || lhs.checked_rem(rhs)? != 0 {
				return None;
			}
			lhs.checked_div(rhs)?
		}
		BinaryOp::Pow => lhs.checked_pow(u32::try_from(rhs).ok()?)?,
		BinaryOp::Leq => return Some(Number::from_bool(lhs <= rhs)),
		BinaryOp::Lt => return Some(Number::from_bool(lhs < rhs)),
		BinaryOp::Geq => return Some(Number::from_bool(lhs >= rhs)),
		BinaryOp::Gt => return Some(Number::from_bool(lhs > rhs)),
		BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Neq => unreachable!("handled before dispatch"),
	};
	Some(Number::Integer(result))
}

fn real_binary_op(lhs: f64, op: BinaryOp, rhs: f64) -> Option<Number> {
	let result = match op {
		BinaryOp::Add => lhs + rhs,
		BinaryOp::Sub => lhs - rhs,
		BinaryOp::Mul => lhs * rhs,
		BinaryOp::Div => lhs / rhs,
		BinaryOp::Pow => {
			// A negative base under a fractional exponent has no real power, so it climbs to the principal complex one
			let power = lhs.powf(rhs);
			if power.is_nan() {
				return Some(Number::Complex(Complex::new(lhs, 0.).powf(rhs)));
			}
			power
		}
		BinaryOp::Leq => return Some(Number::from_bool(lhs <= rhs)),
		BinaryOp::Lt => return Some(Number::from_bool(lhs < rhs)),
		BinaryOp::Geq => return Some(Number::from_bool(lhs >= rhs)),
		BinaryOp::Gt => return Some(Number::from_bool(lhs > rhs)),
		BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Neq => unreachable!("handled before dispatch"),
	};
	Some(Number::Real(result))
}

/// Orders an integer against a real exactly, or `None` against NaN. Within 2^53 the integer widens to f64 exactly, and past it
/// the integer outruns every real nearer zero while every real as far out is whole, so the real's saturating cast is exact where it matters.
fn compare_integer_real(integer: i64, real: f64) -> Option<Ordering> {
	const EXACT_REAL_LIMIT: u64 = 1 << f64::MANTISSA_DIGITS;
	if integer.unsigned_abs() <= EXACT_REAL_LIMIT {
		return (integer as f64).partial_cmp(&real);
	}

	if real.is_nan() {
		return None;
	}
	// Past integer storage the real lies beyond every integer, where `i64::MAX` rounds up to 2^63 as a real
	if real >= i64::MAX as f64 {
		return Some(Ordering::Less);
	}
	if real < i64::MIN as f64 {
		return Some(Ordering::Greater);
	}
	Some(integer.cmp(&(real as i64)))
}

/// The orderings a comparison operator accepts, or `None` for an operator that isn't a comparison.
fn comparison(op: BinaryOp) -> Option<fn(Ordering) -> bool> {
	Some(match op {
		BinaryOp::Leq => Ordering::is_le,
		BinaryOp::Lt => Ordering::is_lt,
		BinaryOp::Geq => Ordering::is_ge,
		BinaryOp::Gt => Ordering::is_gt,
		_ => return None,
	})
}

/// Complex arithmetic; ordering is real-only, so it has no answer here.
fn complex_binary_op(lhs: Complex, op: BinaryOp, rhs: Complex) -> Option<Number> {
	let result = match op {
		BinaryOp::Add => lhs + rhs,
		BinaryOp::Sub => lhs - rhs,
		BinaryOp::Mul => complex_product(lhs, rhs),
		BinaryOp::Div if rhs.im == 0. => Complex::new(part_over_real(lhs.re, rhs.re), part_over_real(lhs.im, rhs.re)),
		// An imaginary divisor divides the parts like a real one, a quarter turn apart: `(a + bi) / di` is `b / d - (a / d) i`
		BinaryOp::Div if rhs.re == 0. => Complex::new(part_over_real(lhs.im, rhs.im), -part_over_real(lhs.re, rhs.im)),
		BinaryOp::Div => complex_divide(lhs, rhs),
		BinaryOp::Pow if rhs.im == 0. => return real_power(lhs, rhs.re, Complex::from(1.), complex_product, Number::Complex, || lhs.powf(rhs.re)),
		BinaryOp::Pow => lhs.powc(rhs),
		BinaryOp::Leq | BinaryOp::Lt | BinaryOp::Geq | BinaryOp::Gt => return None,
		BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Neq => unreachable!("handled before dispatch"),
	};
	Some(Number::Complex(result))
}

/// Multiplies two parts of a product, where a zero part is an absent axis contributing nothing even beside an infinite one, so `inf i`
/// is `∞i` rather than the NaN of `∞ · 0`, unless an operand is the zero value, whose product with an infinity stays indeterminate.
#[inline(always)]
pub(crate) fn part_product(a: f64, b: f64, zero_operand: bool) -> f64 {
	if (a == 0. || b == 0.) && !zero_operand { 0. } else { a * b }
}

/// Reruns a product whose overflowing terms cancelled into NaN on operands divided by powers of two near their largest parts, keeping the
/// rerun only for the NaN parts since its scaling can underflow the rest. A NaN that remains is indeterminate, like `0 * ∞`.
pub(crate) fn rescaled_product<const N: usize>(a: [f64; N], b: [f64; N], product: impl Fn([f64; N], [f64; N]) -> [f64; N]) -> [f64; N] {
	let mut result = product(a, b);
	if !result.iter().any(|part| part.is_nan()) {
		return result;
	}

	let (a_scale, b_scale) = (power_of_two_scale(a.into_iter()), power_of_two_scale(b.into_iter()));
	let rescaled = product(a.map(|part| part / a_scale), b.map(|part| part / b_scale));
	for (part, rescaled) in result.iter_mut().zip(rescaled) {
		if part.is_nan() {
			*part = part_product(part_product(rescaled, a_scale, false), b_scale, false);
		}
	}
	result
}

/// The complex product, with its parts multiplied by [`part_product`].
fn complex_product(a: Complex, b: Complex) -> Complex {
	let [re, im] = rescaled_product([a.re, a.im], [b.re, b.im], |[a_re, a_im], [b_re, b_im]| {
		let zero_operand = (a_re == 0. && a_im == 0.) || (b_re == 0. && b_im == 0.);
		let term = |x: f64, y: f64| part_product(x, y, zero_operand);
		[term(a_re, b_re) - term(a_im, b_im), term(a_re, b_im) + term(a_im, b_re)]
	});
	Complex::new(re, im)
}

/// One part of a division by a real, keeping a zero part zero so `i / 0` is `∞i` rather than the NaN of `0 / 0` in its real part.
fn part_over_real(part: f64, divisor: f64) -> f64 {
	if part == 0. { 0. } else { part / divisor }
}

/// A power of two near the largest magnitude, dividing by which is exact and brings every value within ±2, so sums and squares
/// of the scaled values neither overflow nor underflow. It's 1 when the largest magnitude is zero or infinite.
pub(crate) fn power_of_two_scale(reals: impl Iterator<Item = f64>) -> f64 {
	let largest = reals.fold(0_f64, |largest, real| largest.max(real.abs()));
	if largest == 0. || largest.is_infinite() {
		return 1.;
	}

	// A subnormal has no exponent bits to keep, so the smallest normal power stands in, lifting it into the normal range
	if !largest.is_normal() {
		return f64::MIN_POSITIVE;
	}

	// Keeping only the exponent bits zeroes the mantissa, leaving the power of two
	f64::from_bits(largest.to_bits() & (0x7FF << 52))
}

/// Complex division by Smith's algorithm, scaling by the divisor's larger part so neither `|divisor|²` nor the quotient
/// overflows or underflows while the answer fits, and a simple quotient like `(1 + i) / (1 - i)` stays exact.
pub(crate) fn complex_divide(dividend: Complex, divisor: Complex) -> Complex {
	// Every part is halved when one passes half of f64::MAX, keeping the quotient while the sums below stay in range
	let largest_part = [dividend.re, dividend.im, divisor.re, divisor.im].into_iter().map(f64::abs).fold(0., f64::max);
	let (dividend, divisor) = if largest_part > f64::MAX / 2. { (dividend / 2., divisor / 2.) } else { (dividend, divisor) };

	let Complex { re: a, im: b } = dividend;
	let Complex { re: c, im: d } = divisor;
	if c.abs() >= d.abs() {
		let ratio = d / c;
		let denominator = c + d * ratio;
		Complex::new((a + b * ratio) / denominator, (b - a * ratio) / denominator)
	} else {
		let ratio = c / d;
		let denominator = d + c * ratio;
		Complex::new((a * ratio + b) / denominator, (b * ratio - a) / denominator)
	}
}

/// A base under a real exponent: a whole exponent multiplies out exactly by squaring, so `i^2` is `-1` where the polar form
/// leaves a `sin(π)` residue, and any other exponent takes `fractional_power`.
fn real_power<T: Copy>(base: T, exponent: f64, one: T, multiply: impl Fn(T, T) -> T, wrap: fn(T) -> Number, fractional_power: impl FnOnce() -> T) -> Option<Number> {
	if exponent.fract() != 0. || exponent.abs() >= u128::MAX as f64 {
		return Some(wrap(fractional_power()));
	}

	let power = match exponent.abs() as u128 {
		0 => one,
		count => whole_power(base, count, multiply),
	};
	// An overflowed product has NaN cross terms, so `fractional_power` takes over with the overflow's direction
	if wrap(power).is_nan() {
		return Some(wrap(fractional_power()));
	}

	if exponent < 0. {
		return Number::Real(1.).binary_op(BinaryOp::Div, wrap(power).canonical());
	}
	Some(wrap(power))
}

/// `base^n` for a whole `n` of at least 1 by repeated squaring, in O(log n) multiplications.
fn whole_power<T: Copy>(mut base: T, mut exponent: u128, multiply: impl Fn(T, T) -> T) -> T {
	while exponent & 1 == 0 {
		base = multiply(base, base);
		exponent >>= 1;
	}

	let mut result = base;
	while exponent > 1 {
		exponent >>= 1;
		base = multiply(base, base);
		if exponent & 1 == 1 {
			result = multiply(result, base);
		}
	}
	result
}

/// The factorial of a real number: the exact product for a whole number, and `x! = Γ(x + 1)` past the whole numbers, or
/// `None` at the negative integers (the gamma function's poles, where no signed limit exists) and at -∞.
fn real_factorial(x: f64) -> Option<f64> {
	// The factorial overflows f64 from 171! on, and returning early keeps a huge whole number from spinning the product loop
	if x > 171. {
		return Some(f64::INFINITY);
	}

	if x.fract() == 0. {
		(x >= 0.).then(|| (1..=x as u64).fold(1., |accumulated, k| accumulated * k as f64))
	} else {
		x.is_finite().then(|| real_gamma(x + 1.))
	}
}

/// The Lanczos approximation's shift, for which [`LANCZOS_COEFFICIENTS`] give 15 significant digits near 1, falling to 13 by the f64 overflow limit.
const LANCZOS_G: f64 = 7.;
const LANCZOS_COEFFICIENTS: [f64; 9] = [
	0.9999999999998099,
	676.5203681218851,
	-1259.1392167224028,
	771.3234287776531,
	-176.6150291621406,
	12.507343278686905,
	-0.13857109526572012,
	9.984369578019572e-6,
	1.5056327351493116e-7,
];

/// The gamma function on the reals by the Lanczos approximation. Below 1/2, where the series loses accuracy, it reflects
/// through `Γ(x) Γ(1 - x) = π / sin(πx)`.
fn real_gamma(x: f64) -> f64 {
	if x < 0.5 {
		return PI / (PI * x).sin() / real_gamma(1. - x);
	}

	let x = x - 1.;
	let series = (1..LANCZOS_COEFFICIENTS.len()).fold(LANCZOS_COEFFICIENTS[0], |sum, k| sum + LANCZOS_COEFFICIENTS[k] / (x + k as f64));
	let t = x + LANCZOS_G + 0.5;

	// One exponential for the whole `t^(x + 1/2) e^-t` factor, so past f64's range it is ∞ rather than the NaN of ∞ × 0
	(2. * PI).sqrt() * ((x + 0.5) * t.ln() - t).exp() * series
}

/// The gamma function over the complex plane, taken as one exponential of its logarithm so a magnitude past f64's range is ∞ or 0.
fn complex_gamma(z: Complex) -> Complex {
	complex_log_gamma(z).exp()
}

/// The natural logarithm of the gamma function over the complex plane, by the same approximation and reflection as [`real_gamma`].
pub(crate) fn complex_log_gamma(z: Complex) -> Complex {
	if z.re < 0.5 {
		// Shifting the real part into `[0, 1)` keeps the sine exact, with a half turn (a sign) for each odd shift
		let shift = z.re.floor();
		let log_sin = complex_log_sin(PI * (z - shift)) + Complex::new(0., if shift.rem_euclid(2.) == 0. { 0. } else { PI });
		return PI.ln() - log_sin - complex_log_gamma(1. - z);
	}

	let z = z - 1.;
	let series = (1..LANCZOS_COEFFICIENTS.len()).fold(Complex::from(LANCZOS_COEFFICIENTS[0]), |sum, k| sum + LANCZOS_COEFFICIENTS[k] / (z + k as f64));
	let t = z + LANCZOS_G + 0.5;

	0.5 * (2. * PI).ln() + (z + 0.5) * t.ln() - t + series.ln()
}

/// `ln sin(w)` without forming `sin(w)`, which overflows past an imaginary part of about 710: with `s` the sign of that part,
/// `sin(w) = e^(-siw) (e^(2siw) - 1) / (2si)`, and the growing exponential becomes a plain shift of the logarithm.
fn complex_log_sin(w: Complex) -> Complex {
	let s = if w.im < 0. { -1. } else { 1. };
	let siw = Complex::new(0., s) * w;

	-siw + (((2. * siw).exp() - 1.) / Complex::new(0., 2. * s)).ln()
}

/// Quaternion arithmetic, with `*` the Hamilton product; ordering is real-only, so it has no answer here.
// Out of line as the rare case, keeping `binary_op` compact for the real and integer arithmetic that dominates
#[cold]
#[inline(never)]
fn quaternion_binary_op(lhs: Quaternion, op: BinaryOp, rhs: Quaternion) -> Option<Number> {
	let result = match op {
		BinaryOp::Add => lhs + rhs,
		BinaryOp::Sub => lhs - rhs,
		BinaryOp::Mul => lhs * rhs,
		BinaryOp::Div if rhs.is_real() => lhs.map(|part| part_over_real(part, rhs.w)),
		BinaryOp::Div => lhs / rhs,
		BinaryOp::Pow if rhs.is_real() => return real_power(lhs, rhs.w, Quaternion::ONE, Quaternion::mul, Number::Quaternion, || lhs.pow(rhs)),
		BinaryOp::Pow => lhs.pow(rhs),
		BinaryOp::Leq | BinaryOp::Lt | BinaryOp::Geq | BinaryOp::Gt => return None,
		BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Neq => unreachable!("handled before dispatch"),
	};
	Some(Number::Quaternion(result))
}
