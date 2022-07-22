use super::*;

impl SubPath {
    /// Return the sum of the approximation of the length of each bezier curve along the subpath.
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is `1000`.
	pub fn length(&self, num_subdivisions: Option<i32>) -> f64 {
		self.iter().map(|bezier| bezier.length(num_subdivisions)).sum()
	}
}
