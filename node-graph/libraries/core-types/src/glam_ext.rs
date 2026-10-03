use glam::DVec2;

pub trait FallibleVec2Operations: Sized {
	/// Returns the angle of rotation (in radians) from `self` to `rhs` in the range `[-π, +π]` if both vectors are not zero.
	///
	/// See [`angle_to()`][DVec2::angle_to]
	#[must_use]
	fn try_angle_to(self, rhs: Self) -> Option<f64>;

	/// Returns the vector projection of `self` onto `rhs` if `rhs` is not zero length.
	///
	/// See [`project_onto()`][DVec2::project_onto]
	#[must_use]
	fn try_project_onto(self, rhs: Self) -> Option<Self>;

	/// Returns `true` if the vector is not the zero vector (also rejects NaN).
	#[must_use]
	fn is_non_zero(&self) -> bool;
}

impl FallibleVec2Operations for DVec2 {
	#[inline]
	fn try_angle_to(self, rhs: Self) -> Option<f64> {
		(self.is_non_zero() && rhs.is_non_zero()).then(|| self.angle_to(rhs)).filter(|angle| angle.is_finite())
	}

	#[inline]
	fn try_project_onto(self, rhs: Self) -> Option<Self> {
		let other_len_sq_rcp = rhs.dot(rhs).recip();
		other_len_sq_rcp.is_finite().then(|| rhs * (self.dot(rhs) * other_len_sq_rcp)).filter(|result| result.is_finite())
	}

	#[inline]
	fn is_non_zero(&self) -> bool {
		self.is_finite() && self.length_squared() > 0.
	}
}
