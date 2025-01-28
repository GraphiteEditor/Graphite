use crate::application_io::{TextureFrame, TextureFrameTable};
use crate::raster::bbox::AxisAlignedBbox;
use crate::raster::image::{ImageFrame, ImageFrameTable};
use crate::raster::Pixel;
use crate::vector::{VectorData, VectorDataTable};
use crate::{Artboard, ArtboardGroup, Color, GraphicElement, GraphicGroup, GraphicGroupTable};

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
pub trait TransformSet: Transform {
	fn set_transform(&mut self, value: DAffine2);
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
impl<P: Pixel> TransformSet for ImageFrame<P> {
	fn set_transform(&mut self, value: DAffine2) {
		self.transform = value;
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
		let image_frame = self.instances().next().expect("ONE INSTANCE EXPECTED");
		image_frame.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		let image_frame = self.instances().next().expect("ONE INSTANCE EXPECTED");
		image_frame.local_pivot(pivot)
	}
}
impl<P: Pixel> TransformSet for ImageFrameTable<P>
where
	P: dyn_any::StaticType,
	P::Static: Pixel,
	GraphicElement: From<ImageFrame<P>>,
{
	fn set_transform(&mut self, value: DAffine2) {
		let mut image_frame = self.instances().next().expect("ONE INSTANCE EXPECTED");
		image_frame.transform = value;
	}
}

// Implementations for TextureTable
impl Transform for TextureFrameTable {
	fn transform(&self) -> DAffine2 {
		let image_frame = self.instances().next().expect("ONE INSTANCE EXPECTED");
		image_frame.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		let image_frame = self.instances().next().expect("ONE INSTANCE EXPECTED");
		image_frame.local_pivot(pivot)
	}
}
impl TransformSet for TextureFrameTable {
	fn set_transform(&mut self, value: DAffine2) {
		let mut image_frame = self.instances().next().expect("ONE INSTANCE EXPECTED");
		image_frame.transform = value;
	}
}

// Implementations for GraphicGroup
impl Transform for GraphicGroup {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformSet for GraphicGroup {
	fn set_transform(&mut self, value: DAffine2) {
		self.transform = value;
	}
}

// Implementations for GraphicGroupTable
impl Transform for GraphicGroupTable {
	fn transform(&self) -> DAffine2 {
		let graphic_group = self.instances().next().expect("ONE INSTANCE EXPECTED");
		graphic_group.transform
	}
}
impl TransformSet for GraphicGroupTable {
	fn set_transform(&mut self, value: DAffine2) {
		let mut graphic_group = self.instances().next().expect("ONE INSTANCE EXPECTED");
		graphic_group.transform = value;
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
impl TransformSet for GraphicElement {
	fn set_transform(&mut self, value: DAffine2) {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.set_transform(value),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.set_transform(value),
			GraphicElement::RasterFrame(raster) => raster.set_transform(value),
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
impl TransformSet for VectorData {
	fn set_transform(&mut self, value: DAffine2) {
		self.transform = value;
	}
}

// Implementations for VectorDataTable
impl Transform for VectorDataTable {
	fn transform(&self) -> DAffine2 {
		let vector_data = self.instances().next().expect("ONE INSTANCE EXPECTED");
		vector_data.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		let vector_data = self.instances().next().expect("ONE INSTANCE EXPECTED");
		vector_data.local_pivot(pivot)
	}
}
impl TransformSet for VectorDataTable {
	fn set_transform(&mut self, value: DAffine2) {
		let mut vector_data = self.instances().next().expect("ONE INSTANCE EXPECTED");
		vector_data.transform = value;
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
impl TransformSet for DAffine2 {
	fn set_transform(&mut self, value: DAffine2) {
		*self = value;
	}
}

// Implementations for Footprint
impl Transform for Footprint {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformSet for Footprint {
	fn set_transform(&mut self, value: DAffine2) {
		self.transform = value;
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
fn cull<T>(_footprint: Footprint, #[implementations(VectorDataTable, GraphicGroupTable, Artboard, ImageFrameTable<Color>, ArtboardGroup)] data: T) -> T {
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
impl<T: TransformSet> ApplyTransform for T {
	fn apply_transform(&mut self, &modification: &DAffine2) {
		self.set_transform(self.transform() * modification);
	}
}
impl ApplyTransform for () {
	fn apply_transform(&mut self, &_modification: &DAffine2) {}
}

#[node_macro::node(category(""))]
async fn transform<I: Into<Footprint> + 'n + ApplyTransform + Clone + Send + Sync, T: 'n + TransformSet>(
	#[implementations(
		(),
		(),
		(),
		(),
		Footprint,
	)]
	mut input: I,
	#[implementations(
		() -> VectorDataTable,
		() -> GraphicGroupTable,
		() -> ImageFrameTable<Color>,
		() -> TextureFrame,
		Footprint -> VectorDataTable,
		Footprint -> GraphicGroupTable,
		Footprint -> ImageFrameTable<Color>,
		Footprint -> TextureFrame,
	)]
	transform_target: impl Node<I, Output = T>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	shear: DVec2,
	_pivot: DVec2,
) -> T {
	let modification = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]);
	let footprint = input.clone().into();
	if !footprint.ignore_modifications {
		input.apply_transform(&modification);
	}

	let mut data = transform_target.eval(input).await;

	data.set_transform(modification * data.transform());

	data
}

#[node_macro::node(category(""))]
fn replace_transform<Data: TransformSet, TransformInput: Transform>(
	_: (),
	#[implementations(VectorDataTable, ImageFrameTable<Color>, GraphicGroupTable)] mut data: Data,
	#[implementations(DAffine2)] transform: TransformInput,
) -> Data {
	data.set_transform(transform.transform());
	data
}
