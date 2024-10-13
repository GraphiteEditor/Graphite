#[derive(Debug, PartialEq)]
pub enum Value {
	Complex(f64, f64),
}

impl Value {
	pub fn from_f64(x: f64) -> Self {
		Self::Complex(x, 0.0)
	}
	/// Attempt to convert to a real number
	pub fn as_real(&self) -> Option<f64> {
		match self {
			Self::Complex(real, imaginary) if imaginary.abs() < f64::EPSILON => Some(*real),
			_ => None,
		}
	}
}

impl From<f64> for Value {
	fn from(x: f64) -> Self {
		Self::from_f64(x)
	}
}

impl core::fmt::Display for Value {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(real) = self.as_real() {
			return real.fmt(f);
		}
		match self {
			Value::Complex(real, imaginary) => write!(f, "{real}{imaginary:+}i"),
		}
	}
}
