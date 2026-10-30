use crate::math::bbox::AxisAlignedBbox;
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2, UVec2};

/// Controls whether the Decompose Scale node returns axis-length magnitudes or pure scale factors.
#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum ScaleType {
	/// The visual length of each axis (always positive, includes any skew contribution).
	#[default]
	Magnitude,
	/// The isolated scale factors with rotation and skew stripped away (can be negative for flipped axes).
	Pure,
}

pub trait Transform {
	fn transform(&self) -> DAffine2;

	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		pivot
	}

	/// Decomposes the full transform into `(rotation, signed_scale, skew)` using a TRS+Skew factorization.
	///
	/// - `rotation`: angle in radians
	/// - `signed_scale`: the algebraic scale factors (can be negative for reflections, excludes skew)
	/// - `skew`: the horizontal shear coefficient (the raw matrix value, not an angle)
	///
	/// The original transform can be reconstructed as:
	/// ```ignore
	/// DAffine2::from_scale_angle_translation(scale, rotation, translation) * DAffine2::from_cols_array(&[1., 0., skew, 1., 0., 0.])
	/// ```
	#[inline(always)]
	fn decompose_rotation_scale_skew(&self) -> (f64, DVec2, f64) {
		let t = self.transform();
		let x_axis = t.matrix2.x_axis;
		let y_axis = t.matrix2.y_axis;

		let angle = x_axis.y.atan2(x_axis.x);
		let (sin, cos) = angle.sin_cos();

		let scale_x = if cos.abs() > 1e-10 { x_axis.x / cos } else { x_axis.y / sin };

		let mut skew = (sin * y_axis.y + cos * y_axis.x) / scale_x;
		if !skew.is_finite() {
			skew = 0.;
		}

		let scale_y = if cos.abs() > 1e-10 {
			(y_axis.y - scale_x * sin * skew) / cos
		} else {
			(scale_x * cos * skew - y_axis.x) / sin
		};

		(angle, DVec2::new(scale_x, scale_y), skew)
	}

	/// Extracts the rotation angle (in radians) from the transform.
	/// This is the angle of the x-axis and is correct regardless of skew, negative scale, or non-uniform scale.
	fn decompose_rotation(&self) -> f64 {
		let x_axis = self.transform().matrix2.x_axis;
		let rotation = x_axis.y.atan2(x_axis.x);
		if rotation == -0. { 0. } else { rotation }
	}

	/// Returns the signed scale components from the TRS+Skew decomposition.
	/// Unlike [`Self::scale_magnitudes`] which returns positive axis-length magnitudes,
	/// this returns the algebraic scale factors which can be negative for reflections and exclude skew.
	fn decompose_scale(&self) -> DVec2 {
		self.decompose_rotation_scale_skew().1
	}

	/// Returns the unsigned scale as the lengths of each axis (always positive, includes skew contribution).
	/// Use this for magnitude-based queries like stroke width scaling, zoom level, or bounding box inflation.
	fn scale_magnitudes(&self) -> DVec2 {
		DVec2::new(self.transform().transform_vector2(DVec2::X).length(), self.transform().transform_vector2(DVec2::Y).length())
	}

	/// Returns the horizontal skew (shear) coefficient from the TRS+Skew decomposition.
	/// This is the raw matrix coefficient. To convert to degrees: `skew.atan().to_degrees()`.
	fn decompose_skew(&self) -> f64 {
		self.decompose_rotation_scale_skew().2
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

#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
		resolution: UVec2::ONE,
		quality: RenderQuality::Full,
	};

	pub const MAX_UNION_RESOLUTION: u32 = 8192;

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
		let res = self.resolution.as_dvec2();
		let c0 = inverse.transform_point2(DVec2::ZERO);
		let c1 = inverse.transform_point2(DVec2::new(res.x, 0.));
		let c2 = inverse.transform_point2(res);
		let c3 = inverse.transform_point2(DVec2::new(0., res.y));
		AxisAlignedBbox {
			start: c0.min(c1).min(c2).min(c3),
			end: c0.max(c1).max(c2).max(c3),
		}
	}

	/// The same view squeezed into a single pixel, for evaluating content only to learn where it's placed. A boundless footprint stays boundless.
	pub fn probe(&self) -> Footprint {
		if !self.transform.is_finite() {
			return *self;
		}

		Footprint {
			transform: DAffine2::from_scale(1. / self.resolution.max(UVec2::ONE).as_dvec2()) * self.transform,
			resolution: UVec2::ONE,
			..*self
		}
	}

	/// The footprint for content shown once per transform in `instance_transforms`, covering what every instance shows of it. A boundless footprint stays boundless.
	pub fn for_instances(&self, instance_transforms: impl IntoIterator<Item = DAffine2>) -> Footprint {
		if !self.transform.is_finite() {
			return *self;
		}

		let views = instance_transforms.into_iter().map(|instance_transform| Footprint {
			transform: self.transform * instance_transform,
			..*self
		});
		Self::union(views).unwrap_or(*self)
	}

	/// The footprint covering everything each of `views` shows of the content, axis-aligned in the content's space at the finest pixel density any of them needs.
	/// Its density is lowered to keep each side within [`Self::MAX_UNION_RESOLUTION`]. `None` when no view has an invertible transform.
	pub fn union(views: impl IntoIterator<Item = Footprint>) -> Option<Footprint> {
		let mut bounds: Option<AxisAlignedBbox> = None;
		let mut density = 0_f64;
		let mut quality = RenderQuality::Full;
		for view in views {
			if view.transform.matrix2.determinant() == 0. {
				continue;
			}

			let view_bounds = view.viewport_bounds_in_local_space();
			bounds = Some(bounds.map_or(view_bounds.clone(), |bounds| bounds.union(&view_bounds)));
			density = density.max(view.scale().max_element());
			quality = view.quality;
		}

		let bounds = bounds?;
		let size = bounds.size();
		let density = density.min(Self::MAX_UNION_RESOLUTION as f64 / size.max_element());
		let resolution = (size * density).ceil().as_uvec2().max(UVec2::ONE);

		Some(Footprint {
			transform: DAffine2::from_scale(DVec2::splat(density)) * DAffine2::from_translation(-bounds.start),
			resolution,
			quality,
		})
	}

	pub fn scale(&self) -> DVec2 {
		self.transform.scale_magnitudes()
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

/// Consumes an item's `transform` attribute by baking it into the underlying value itself.
pub trait BakeTransform {
	fn bake_transform(&mut self, transform: &DAffine2);
}

impl BakeTransform for DAffine2 {
	fn bake_transform(&mut self, transform: &DAffine2) {
		*self = *transform * *self;
	}
}

impl BakeTransform for DVec2 {
	fn bake_transform(&mut self, transform: &DAffine2) {
		*self = transform.transform_point2(*self);
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
		*self = modification.transform_point2(*self);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn view(width: u32, height: u32) -> Footprint {
		Footprint {
			resolution: UVec2::new(width, height),
			..Footprint::DEFAULT
		}
	}

	#[test]
	fn instances_footprint_covers_every_instance() {
		let footprint = view(100, 100).for_instances([DAffine2::IDENTITY, DAffine2::from_translation(DVec2::new(200., 0.))]);

		let bounds = footprint.viewport_bounds_in_local_space();
		assert!(bounds.start.abs_diff_eq(DVec2::new(-200., 0.), 1e-9), "{bounds:?}");
		assert!(bounds.end.abs_diff_eq(DVec2::new(100., 100.), 1e-9), "{bounds:?}");
		assert_eq!(footprint.resolution, UVec2::new(300, 100));
	}

	#[test]
	fn instances_footprint_takes_the_finest_density() {
		let footprint = view(100, 100).for_instances([DAffine2::IDENTITY, DAffine2::from_scale(DVec2::splat(4.))]);

		assert!((footprint.scale().x - 4.).abs() < 1e-9);
		assert_eq!(footprint.resolution, UVec2::new(400, 400));
	}

	#[test]
	fn instances_footprint_caps_its_resolution() {
		let footprint = view(1000, 1000).for_instances([DAffine2::IDENTITY, DAffine2::from_translation(DVec2::new(1e6, 0.))]);

		assert!(footprint.resolution.max_element() <= Footprint::MAX_UNION_RESOLUTION);
	}

	#[test]
	fn boundless_footprint_stays_boundless() {
		assert_eq!(Footprint::BOUNDLESS.for_instances([DAffine2::from_translation(DVec2::ONE)]), Footprint::BOUNDLESS);
		assert_eq!(Footprint::BOUNDLESS.probe(), Footprint::BOUNDLESS);
	}

	#[test]
	fn probe_covers_the_same_view_in_one_pixel() {
		let footprint = Footprint {
			transform: DAffine2::from_scale(DVec2::splat(2.)),
			..view(100, 50)
		};
		let probe = footprint.probe();

		assert_eq!(probe.resolution, UVec2::ONE);
		let (bounds, probe_bounds) = (footprint.viewport_bounds_in_local_space(), probe.viewport_bounds_in_local_space());
		assert!(bounds.start.abs_diff_eq(probe_bounds.start, 1e-9) && bounds.end.abs_diff_eq(probe_bounds.end, 1e-9));
	}
}
