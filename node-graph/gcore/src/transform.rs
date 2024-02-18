use core::future::Future;

use dyn_any::StaticType;
use glam::DAffine2;

use glam::DVec2;

use crate::raster::bbox::AxisAlignedBbox;
use crate::raster::ImageFrame;
use crate::raster::Pixel;
use crate::vector::VectorData;
use crate::Artboard;
use crate::GraphicElement;
use crate::GraphicGroup;
use crate::Node;

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

impl<P: Pixel> Transform for ImageFrame<P> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.local_pivot(pivot)
	}
}
impl<P: Pixel> Transform for &ImageFrame<P> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		(*self).local_pivot(pivot)
	}
}
impl<P: Pixel> TransformMut for ImageFrame<P> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}
impl Transform for GraphicGroup {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl Transform for &GraphicGroup {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformMut for GraphicGroup {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}
impl Transform for GraphicElement {
	fn transform(&self) -> DAffine2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.transform(),
			GraphicElement::ImageFrame(image_frame) => image_frame.transform(),
			GraphicElement::Text(_) => todo!("Transform of text"),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.transform(),
			GraphicElement::Artboard(artboard) => artboard.transform(),
		}
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.local_pivot(pivot),
			GraphicElement::ImageFrame(image_frame) => image_frame.local_pivot(pivot),
			GraphicElement::Text(_) => todo!("Transform of text"),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.local_pivot(pivot),
			GraphicElement::Artboard(artboard) => artboard.local_pivot(pivot),
		}
	}
	fn decompose_scale(&self) -> DVec2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.decompose_scale(),
			GraphicElement::ImageFrame(image_frame) => image_frame.decompose_scale(),
			GraphicElement::Text(_) => todo!("Transform of text"),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.decompose_scale(),
			GraphicElement::Artboard(artboard) => artboard.decompose_scale(),
		}
	}
}
impl TransformMut for GraphicElement {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			GraphicElement::VectorData(vector_shape) => vector_shape.transform_mut(),
			GraphicElement::ImageFrame(image_frame) => image_frame.transform_mut(),
			GraphicElement::Text(_) => todo!("Transform of text"),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.transform_mut(),
			GraphicElement::Artboard(_) => todo!("Transform of artboard"),
		}
	}
}

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

impl Transform for Artboard {
	fn transform(&self) -> DAffine2 {
		DAffine2::from_translation(self.location.as_dvec2())
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		self.location.as_dvec2() + self.dimensions.as_dvec2() * pivot
	}
}

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

#[derive(Debug, Clone, Copy)]
pub struct TransformNode<TransformTarget, Translation, Rotation, Scale, Shear, Pivot> {
	pub(crate) transform_target: TransformTarget,
	pub(crate) translate: Translation,
	pub(crate) rotate: Rotation,
	pub(crate) scale: Scale,
	pub(crate) shear: Shear,
	pub(crate) pivot: Pivot,
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
	Probabilty(f32),
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
	/// When the transform is set downstream, all upsream modifications have to be ignored
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
}

#[derive(Debug, Clone, Copy)]
pub struct CullNode<VectorData> {
	pub(crate) vector_data: VectorData,
}

#[node_macro::node_fn(CullNode)]
fn cull_vector_data<T>(footprint: Footprint, vector_data: T) -> T {
	// TODO: Implement culling
	vector_data
}

impl core::hash::Hash for Footprint {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.transform.to_cols_array().iter().for_each(|x| x.to_le_bytes().hash(state));
		self.resolution.hash(state)
	}
}

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

#[node_macro::node_fn(TransformNode)]
pub(crate) async fn transform_vector_data<Fut: Future>(
	mut footprint: Footprint,
	transform_target: impl Node<Footprint, Output = Fut>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	shear: DVec2,
	pivot: DVec2,
) -> Fut::Output
where
	Fut::Output: TransformMut,
{
	// TODO: This is hack and might break for Vector data because the pivot may be incorrect
	let transform = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]);
	if !footprint.ignore_modifications {
		let pivot_transform = DAffine2::from_translation(pivot);
		let modification = pivot_transform * transform * pivot_transform.inverse();
		*footprint.transform_mut() = footprint.transform() * modification;
	}

	let mut data = self.transform_target.eval(footprint).await;
	let pivot_transform = DAffine2::from_translation(data.local_pivot(pivot));

	let modification = pivot_transform * transform * pivot_transform.inverse();
	let data_transform = data.transform_mut();
	*data_transform = modification * (*data_transform);

	data
}
#[derive(Debug, Clone, Copy)]
pub struct SetTransformNode<TransformInput> {
	pub(crate) transform: TransformInput,
}

#[node_macro::node_fn(SetTransformNode)]
pub(crate) fn set_transform<Data: TransformMut, TransformInput: Transform>(mut data: Data, transform: TransformInput) -> Data {
	let data_transform = data.transform_mut();
	*data_transform = transform.transform();
	data
}
