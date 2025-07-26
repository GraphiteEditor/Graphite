pub mod types {
	/// 0% - 100%
	pub type Percentage = f64;
	/// -100% - 100%
	pub type SignedPercentage = f64;
	/// -180° - 180°
	pub type Angle = f64;
	/// Ends in the unit of x
	pub type Multiplier = f64;
	/// Non-negative integer with px unit
	pub type PixelLength = f64;
	/// Non-negative
	pub type Length = f64;
	/// 0 to 1
	pub type Fraction = f64;
	/// Unsigned integer
	pub type IntegerCount = u32;
	/// Unsigned integer to be used for random seeds
	pub type SeedValue = u32;
	/// DVec2 with px unit
	pub type PixelSize = glam::DVec2;
	/// String with one or more than one line
	pub type TextArea = String;
}
