#[derive(Debug, PartialEq)]
pub enum Value {
	Complex(f64, f64),
}

impl Value {
	pub fn from_f64(x: f64) -> Self {
		Self::Complex(x, 0.0)
	}
}

impl From<f64> for Value {
	fn from(x: f64) -> Self {
		Self::from_f64(x)
	}
}
