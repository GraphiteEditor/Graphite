use crate::application_io::{TextureFrame, TextureFrameTable};
use crate::raster::bbox::AxisAlignedBbox;
use crate::raster::image::{ImageFrame, ImageFrameTable};
use crate::raster::Pixel;
use crate::vector::{VectorData, VectorDataTable};
use crate::{Artboard, ArtboardGroup, CloneVarArgs, Color, Context, Ctx, ExtractAll, GraphicElement, GraphicGroupTable, OwnedContextImpl};

use glam::{DAffine2, DVec2};

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

// Implementations for ImageFrame<P>
impl<P: Pixel> Transform for ImageFrame<P> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.local_pivot(pivot)
	}
}
impl<P: Pixel> TransformMut for ImageFrame<P> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

// Implementations for ImageFrameTable<P>
impl<P: Pixel> Transform for ImageFrameTable<P>
where
	P: dyn_any::StaticType,
	P::Static: Pixel,
	GraphicElement: From<ImageFrame<P>>,
{
	fn transform(&self) -> DAffine2 {
		let image_frame = self.one_item();
		image_frame.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		let image_frame = self.one_item();
		image_frame.local_pivot(pivot)
	}
}
impl<P: Pixel> TransformMut for ImageFrameTable<P>
where
	P: dyn_any::StaticType,
	P::Static: Pixel,
	GraphicElement: From<ImageFrame<P>>,
{
	fn transform_mut(&mut self) -> &mut DAffine2 {
		let image_frame = self.one_item_mut();
		&mut image_frame.transform
	}
}

// Implementations for TextureTable
impl Transform for TextureFrameTable {
	fn transform(&self) -> DAffine2 {
		let image_frame = self.one_item();
		image_frame.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		let image_frame = self.one_item();
		image_frame.local_pivot(pivot)
	}
}
impl TransformMut for TextureFrameTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		let image_frame = self.one_item_mut();
		&mut image_frame.transform
	}
}

// Implementations for GraphicGroup
impl Transform for GraphicGroup {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformMut for GraphicGroup {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

// Implementations for GraphicGroupTable
impl Transform for GraphicGroupTable {
	fn transform(&self) -> DAffine2 {
		let graphic_group = self.one_item();
		graphic_group.transform
	}
}
impl TransformMut for GraphicGroupTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		let graphic_group = self.one_item_mut();
		&mut graphic_group.transform
	}
}

// Implementations for GraphicElement
impl Transform for GraphicElement {
	fn transform(&self) -> DAffine2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.transform(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.transform(),
			GraphicElement::RasterFrame(raster) => raster.transform(),
		}
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.local_pivot(pivot),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.local_pivot(pivot),
			GraphicElement::RasterFrame(raster) => raster.local_pivot(pivot),
		}
	}
}
impl TransformMut for GraphicElement {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.transform_mut(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.transform_mut(),
			GraphicElement::RasterFrame(raster) => raster.transform_mut(),
		}
	}
}

// Implementations for VectorData
impl Transform for VectorData {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.local_pivot(pivot)
	}
}
impl TransformMut for VectorData {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

// Implementations for VectorDataTable
impl Transform for VectorDataTable {
	fn transform(&self) -> DAffine2 {
		let vector_data = self.one_item();
		vector_data.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		let vector_data = self.one_item();
		vector_data.local_pivot(pivot)
	}
}
impl TransformMut for VectorDataTable {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		let vector_data = self.one_item_mut();
		&mut vector_data.transform
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
	/// When the transform is set downstream, all upstream modifications have to be ignored
	pub ignore_modifications: bool,
}

impl Default for Footprint {
	fn default() -> Self {
		Self {
			transform: DAffine2::IDENTITY,
			resolution: glam::UVec2::new(1920, 1080),
			quality: RenderQuality::Full,
			ignore_modifications: false,
		}
	}
}

impl Footprint {
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

#[node_macro::node(category("Debug"))]
fn cull<T>(_: impl Ctx, #[implementations(VectorDataTable, GraphicGroupTable, Artboard, ImageFrameTable<Color>, ArtboardGroup)] data: T) -> T {
	data
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
async fn transform<T: 'n + TransformMut + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> ImageFrameTable<Color>,
		Context -> TextureFrameTable,
	)]
	transform_target: impl Node<Context<'static>, Output = T>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	shear: DVec2,
	_pivot: DVec2,
) -> T {
	let modification = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]);
	let mut footprint = *ctx.try_footprint().unwrap();

	if !footprint.ignore_modifications {
		footprint.apply_transform(&modification);
	}
	let mut ctx = OwnedContextImpl::from(ctx);
	ctx.set_footprint(footprint);

	let mut transform_target = transform_target.eval(Some(ctx.into())).await;

	let data_transform = transform_target.transform_mut();
	*data_transform = modification * (*data_transform);

	transform_target
}

#[node_macro::node(category(""))]
fn replace_transform<Data: TransformMut, TransformInput: Transform>(
	_: impl Ctx,
	#[implementations(VectorDataTable, ImageFrameTable<Color>, GraphicGroupTable)] mut data: Data,
	#[implementations(DAffine2)] transform: TransformInput,
) -> Data {
	let data_transform = data.transform_mut();
	*data_transform = transform.transform();
	data
}
