use crate::Artboard;
use crate::math::bbox::AxisAlignedBbox;
pub use crate::vector::ReferencePoint;
use core::f64;
use glam::{DAffine2, DMat2, DVec2, UVec2};

pub trait Transform {
	fn transform(&self) -> DAffine2;

	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		pivot
	}

	fn decompose_scale(&self) -> DVec2 {
		DVec2::new(self.transform().transform_vector2(DVec2::X).length(), self.transform().transform_vector2(DVec2::Y).length())
	}

	/// Requires that the transform does not contain any skew.
	fn decompose_rotation(&self) -> f64 {
		let rotation_matrix = (self.transform() * DAffine2::from_scale(self.decompose_scale().recip())).matrix2;
		let rotation = -rotation_matrix.mul_vec2(DVec2::X).angle_to(DVec2::X);
		if rotation == -0. { 0. } else { rotation }
	}

	/// Detects if the transform contains skew by checking if the transformation matrix
	/// deviates from a pure rotation + uniform scale + translation.
	///
	/// Returns true if the matrix columns are not orthogonal or have different lengths,
	/// indicating the presence of skew or non-uniform scaling.
	fn has_skew(&self) -> bool {
		let mat2 = self.transform().matrix2;
		let col0 = mat2.x_axis;
		let col1 = mat2.y_axis;

		const EPSILON: f64 = 1e-10;

		// Check if columns are orthogonal (dot product should be ~0) and equal length
		// Non-orthogonal columns or different lengths indicate skew/non-uniform scaling
		col0.dot(col1).abs() > EPSILON || (col0.length() - col1.length()).abs() > EPSILON
	}
}

pub trait TransformMut: Transform {
	fn transform_mut(&mut self) -> &mut DAffine2;
	fn translate(&mut self, offset: DVec2) {
		*self.transform_mut() = DAffine2::from_translation(offset) * self.transform();
	}
}

// Implementation for references to anything that implements Transform
impl<T: Transform> Transform for &T {
	fn transform(&self) -> DAffine2 {
		(*self).transform()
	}
}

// Implementations for Artboard
impl Transform for Artboard {
	fn transform(&self) -> DAffine2 {
		DAffine2::from_translation(self.location.as_dvec2())
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.location.as_dvec2() + self.dimensions.as_dvec2() * pivot
	}
}

// Implementations for DAffine2
impl Transform for DAffine2 {
	fn transform(&self) -> DAffine2 {
		*self
	}
}
impl TransformMut for DAffine2 {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		self
	}
}

// Implementations for Footprint
impl Transform for Footprint {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformMut for Footprint {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RenderQuality {
	/// Low quality, fast rendering
	Preview,
	/// Ensure that the render is available with at least the specified quality
	/// A value of 0.5 means that the render is available with at least 50% of the final image resolution
	Scale(f32),
	/// Flip a coin to decide if the render should be available with the current quality or done at full quality
	/// This should be used to gradually update the render quality of a cached node
	Probability(f32),
	/// Render at full quality
	Full,
}
#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Footprint {
	/// Inverse of the transform which will be applied to the node output during the rendering process
	pub transform: DAffine2,
	/// Resolution of the target output area in pixels
	pub resolution: UVec2,
	/// Quality of the render, this may be used by caching nodes to decide if the cached render is sufficient
	pub quality: RenderQuality,
}

impl Default for Footprint {
	fn default() -> Self {
		Self::DEFAULT
	}
}

impl Footprint {
	pub const DEFAULT: Self = Self {
		transform: DAffine2::IDENTITY,
		resolution: UVec2::new(1920, 1080),
		quality: RenderQuality::Full,
	};

	pub const BOUNDLESS: Self = Self {
		transform: DAffine2 {
			matrix2: DMat2::from_diagonal(DVec2::splat(f64::INFINITY)),
			translation: DVec2::ZERO,
		},
		resolution: UVec2::ZERO,
		quality: RenderQuality::Full,
	};

	pub fn viewport_bounds_in_local_space(&self) -> AxisAlignedBbox {
		let inverse = self.transform.inverse();
		let start = inverse.transform_point2((0., 0.).into());
		let end = inverse.transform_point2(self.resolution.as_dvec2());
		AxisAlignedBbox { start, end }
	}

	pub fn scale(&self) -> DVec2 {
		self.transform.decompose_scale()
	}

	pub fn offset(&self) -> DVec2 {
		self.transform.transform_point2(DVec2::ZERO)
	}
}

impl From<()> for Footprint {
	fn from(_: ()) -> Self {
		Footprint::default()
	}
}

impl std::hash::Hash for Footprint {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.transform.to_cols_array().iter().for_each(|x| x.to_le_bytes().hash(state));
		self.resolution.hash(state)
	}
}

pub trait ApplyTransform {
	fn apply_transform(&mut self, modification: &DAffine2);
	fn left_apply_transform(&mut self, modification: &DAffine2);
}
impl<T: TransformMut> ApplyTransform for T {
	fn apply_transform(&mut self, &modification: &DAffine2) {
		*self.transform_mut() = self.transform() * modification
	}
	fn left_apply_transform(&mut self, &modification: &DAffine2) {
		*self.transform_mut() = modification * self.transform()
	}
}
impl ApplyTransform for DVec2 {
	fn apply_transform(&mut self, modification: &DAffine2) {
		*self = modification.transform_point2(*self);
	}
	fn left_apply_transform(&mut self, modification: &DAffine2) {
		*self = modification.inverse().transform_point2(*self);
	}
}
