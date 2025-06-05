use crate::instances::Instances;
use crate::raster::bbox::AxisAlignedBbox;
use crate::raster::image::RasterDataTable;
use crate::vector::VectorDataTable;
use crate::{Artboard, CloneVarArgs, Color, Context, Ctx, ExtractAll, GraphicGroupTable, OwnedContextImpl};
use core::f64;
use glam::{DAffine2, DMat2, DVec2};

pub trait Transform {
	fn transform(&self) -> DAffine2;
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		pivot
	}
	fn decompose_scale(&self) -> DVec2 {
		DVec2::new(
			self.transform().transform_vector2((1., 0.).into()).length(),
			self.transform().transform_vector2((0., 1.).into()).length(),
		)
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

#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq)]
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
#[derive(Debug, Clone, Copy, dyn_any::DynAny, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Footprint {
	/// Inverse of the transform which will be applied to the node output during the rendering process
	pub transform: DAffine2,
	/// Resolution of the target output area in pixels
	pub resolution: glam::UVec2,
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
		resolution: glam::UVec2::new(1920, 1080),
		quality: RenderQuality::Full,
	};

	pub const BOUNDLESS: Self = Self {
		transform: DAffine2 {
			matrix2: DMat2::from_diagonal(DVec2::splat(f64::INFINITY)),
			translation: DVec2::ZERO,
		},
		resolution: glam::UVec2::new(0, 0),
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

impl core::hash::Hash for Footprint {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.transform.to_cols_array().iter().for_each(|x| x.to_le_bytes().hash(state));
		self.resolution.hash(state)
	}
}

pub trait ApplyTransform {
	fn apply_transform(&mut self, modification: &DAffine2);
}
impl<T: TransformMut> ApplyTransform for T {
	fn apply_transform(&mut self, &modification: &DAffine2) {
		*self.transform_mut() = self.transform() * modification
	}
}
impl ApplyTransform for () {
	fn apply_transform(&mut self, &_modification: &DAffine2) {}
}

#[node_macro::node(category(""))]
async fn transform<T: 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> ImageFrameTable<Color>,
		Context -> TextureFrameTable,
	)]
	transform_target: impl Node<Context<'static>, Output = Instances<T>>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	shear: DVec2,
	_pivot: DVec2,
) -> Instances<T> {
	let matrix = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]);

	let footprint = ctx.try_footprint().copied();

	let mut ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.apply_transform(&matrix);
		ctx = ctx.with_footprint(footprint);
	}

	let mut transform_target = transform_target.eval(ctx.into_context()).await;

	for data_transform in transform_target.instance_mut_iter() {
		*data_transform.transform = matrix * *data_transform.transform;
	}

	transform_target
}

#[node_macro::node(category(""))]
fn replace_transform<Data, TransformInput: Transform>(
	_: impl Ctx,
	#[implementations(VectorDataTable, RasterDataTable<Color>, GraphicGroupTable)] mut data: Instances<Data>,
	#[implementations(DAffine2)] transform: TransformInput,
) -> Instances<Data> {
	for data_transform in data.instance_mut_iter() {
		*data_transform.transform = transform.transform();
	}
	data
}

#[node_macro::node(category("Debug"))]
async fn boundless_footprint<T: 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> ImageFrameTable<Color>,
		Context -> TextureFrameTable,
		Context -> String,
		Context -> f64,
	)]
	transform_target: impl Node<Context<'static>, Output = T>,
) -> T {
	let ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::BOUNDLESS);

	transform_target.eval(ctx.into_context()).await
}
#[node_macro::node(category("Debug"))]
async fn freeze_real_time<T: 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> ImageFrameTable<Color>,
		Context -> TextureFrameTable,
		Context -> String,
		Context -> f64,
	)]
	transform_target: impl Node<Context<'static>, Output = T>,
) -> T {
	let ctx = OwnedContextImpl::from(ctx).with_real_time(0.);

	transform_target.eval(ctx.into_context()).await
}

#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ReferencePoint {
	#[default]
	None,
	TopLeft,
	TopCenter,
	TopRight,
	CenterLeft,
	Center,
	CenterRight,
	BottomLeft,
	BottomCenter,
	BottomRight,
}

impl ReferencePoint {
	pub fn point_in_bounding_box(&self, bounding_box: AxisAlignedBbox) -> Option<DVec2> {
		let size = bounding_box.size();
		let offset = match self {
			ReferencePoint::None => return None,
			ReferencePoint::TopLeft => DVec2::ZERO,
			ReferencePoint::TopCenter => DVec2::new(size.x / 2., 0.),
			ReferencePoint::TopRight => DVec2::new(size.x, 0.),
			ReferencePoint::CenterLeft => DVec2::new(0., size.y / 2.),
			ReferencePoint::Center => DVec2::new(size.x / 2., size.y / 2.),
			ReferencePoint::CenterRight => DVec2::new(size.x, size.y / 2.),
			ReferencePoint::BottomLeft => DVec2::new(0., size.y),
			ReferencePoint::BottomCenter => DVec2::new(size.x / 2., size.y),
			ReferencePoint::BottomRight => DVec2::new(size.x, size.y),
		};
		Some(bounding_box.start + offset)
	}
}

impl From<&str> for ReferencePoint {
	fn from(input: &str) -> Self {
		match input {
			"None" => ReferencePoint::None,
			"TopLeft" => ReferencePoint::TopLeft,
			"TopCenter" => ReferencePoint::TopCenter,
			"TopRight" => ReferencePoint::TopRight,
			"CenterLeft" => ReferencePoint::CenterLeft,
			"Center" => ReferencePoint::Center,
			"CenterRight" => ReferencePoint::CenterRight,
			"BottomLeft" => ReferencePoint::BottomLeft,
			"BottomCenter" => ReferencePoint::BottomCenter,
			"BottomRight" => ReferencePoint::BottomRight,
			_ => panic!("Failed parsing unrecognized ReferencePosition enum value '{input}'"),
		}
	}
}

impl From<ReferencePoint> for Option<DVec2> {
	fn from(input: ReferencePoint) -> Self {
		match input {
			ReferencePoint::None => None,
			ReferencePoint::TopLeft => Some(DVec2::new(0., 0.)),
			ReferencePoint::TopCenter => Some(DVec2::new(0.5, 0.)),
			ReferencePoint::TopRight => Some(DVec2::new(1., 0.)),
			ReferencePoint::CenterLeft => Some(DVec2::new(0., 0.5)),
			ReferencePoint::Center => Some(DVec2::new(0.5, 0.5)),
			ReferencePoint::CenterRight => Some(DVec2::new(1., 0.5)),
			ReferencePoint::BottomLeft => Some(DVec2::new(0., 1.)),
			ReferencePoint::BottomCenter => Some(DVec2::new(0.5, 1.)),
			ReferencePoint::BottomRight => Some(DVec2::new(1., 1.)),
		}
	}
}

impl From<DVec2> for ReferencePoint {
	fn from(input: DVec2) -> Self {
		const TOLERANCE: f64 = 1e-5_f64;
		if input.y.abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return ReferencePoint::TopLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return ReferencePoint::TopCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return ReferencePoint::TopRight;
			}
		} else if (input.y - 0.5).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return ReferencePoint::CenterLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return ReferencePoint::Center;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return ReferencePoint::CenterRight;
			}
		} else if (input.y - 1.).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return ReferencePoint::BottomLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return ReferencePoint::BottomCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return ReferencePoint::BottomRight;
			}
		}
		ReferencePoint::None
	}
}
