use glam::{DAffine2, DVec2};

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

pub trait FallibleAffine2Operations: Sized {
	/// Returns `true` if the affine transform is singular (its 2x2 linear transformation has a zero, subnormal, or non-finite determinant).
	#[must_use]
	fn is_singular(&self) -> bool;

	/// Returns `true` if the affine transform inverts to a finite matrix.
	#[must_use]
	fn is_invertible(&self) -> bool;

	/// Returns a non-singular (invertible) approximation of the affine transform,
	/// keeping its translation and orientation while restoring degenerate zero-length axes.
	#[must_use]
	fn to_invertible(&self) -> Self;
}

impl FallibleAffine2Operations for DAffine2 {
	#[inline]
	fn is_singular(&self) -> bool {
		!self.is_invertible()
	}

	#[inline]
	fn is_invertible(&self) -> bool {
		self.matrix2.determinant().recip().is_finite()
	}

	fn to_invertible(&self) -> DAffine2 {
		if self.is_invertible() {
			return *self;
		}

		let mut x_axis = self.matrix2.x_axis;
		let mut y_axis = self.matrix2.y_axis;

		let x_len_sq = x_axis.length_squared();
		let y_len_sq = y_axis.length_squared();

		let x_valid = x_axis.is_finite() && x_len_sq > 1e-12;
		let y_valid = y_axis.is_finite() && y_len_sq > 1e-12;

		match (x_valid, y_valid) {
			(true, false) => {
				let dir = x_axis.normalize();
				y_axis = DVec2::new(-dir.y, dir.x);
			}
			(false, true) => {
				let dir = y_axis.normalize();
				x_axis = DVec2::new(dir.y, -dir.x);
			}
			_ => {
				x_axis = DVec2::X;
				y_axis = DVec2::Y;
			}
		}

		let translation = if self.translation.is_finite() { self.translation } else { DVec2::ZERO };
		DAffine2::from_cols(x_axis, y_axis, translation)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn affine_singularity_checks() {
		assert!(!DAffine2::IDENTITY.is_singular());
		assert!(DAffine2::IDENTITY.is_invertible());

		let zero_x_scale = DAffine2::from_scale_angle_translation(DVec2::new(0., 100.), 0.5, DVec2::new(50., 50.));
		assert!(zero_x_scale.is_singular());
		assert!(!zero_x_scale.is_invertible());

		let zero_y_scale = DAffine2::from_scale_angle_translation(DVec2::new(100., 0.), 0., DVec2::new(10., 20.));
		assert!(zero_y_scale.is_singular());
		assert!(!zero_y_scale.is_invertible());

		let zero_scale = DAffine2::from_scale(DVec2::ZERO);
		assert!(zero_scale.is_singular());
		assert!(!zero_scale.is_invertible());

		let normal = DAffine2::from_scale_angle_translation(DVec2::new(10., 20.), std::f64::consts::PI / 4., DVec2::new(30., 40.));
		assert!(!normal.is_singular());
		assert!(normal.is_invertible());

		let inv_x = zero_x_scale.to_invertible();
		assert!(inv_x.is_invertible());
		assert_eq!(inv_x.translation, DVec2::new(50., 50.));

		let inv_y = zero_y_scale.to_invertible();
		assert!(inv_y.is_invertible());
		assert_eq!(inv_y.translation, DVec2::new(10., 20.));
	}
}
