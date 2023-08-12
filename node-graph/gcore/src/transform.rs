use core::future::Future;

use dyn_any::{DynAny, StaticType};
use glam::DAffine2;

use glam::DVec2;

use crate::raster::ImageFrame;
use crate::raster::Pixel;
use crate::vector::VectorData;
use crate::GraphicElementData;
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
}
impl<P: Pixel> Transform for &ImageFrame<P> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl<P: Pixel> TransformMut for ImageFrame<P> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}
impl Transform for GraphicElementData {
	fn transform(&self) -> DAffine2 {
		match self {
			GraphicElementData::VectorShape(vector_shape) => vector_shape.transform(),
			GraphicElementData::ImageFrame(image_frame) => image_frame.transform(),
			GraphicElementData::Text(_) => todo!("Transform of text"),
			GraphicElementData::GraphicGroup(_graphic_group) => DAffine2::IDENTITY,
			GraphicElementData::Artboard(_artboard) => DAffine2::IDENTITY,
		}
	}
	fn local_pivot(&self, pivot: DVec2) -> DVec2 {
		match self {
			GraphicElementData::VectorShape(vector_shape) => vector_shape.local_pivot(pivot),
			GraphicElementData::ImageFrame(image_frame) => image_frame.local_pivot(pivot),
			GraphicElementData::Text(_) => todo!("Transform of text"),
			GraphicElementData::GraphicGroup(_graphic_group) => pivot,
			GraphicElementData::Artboard(_artboard) => pivot,
		}
	}
	fn decompose_scale(&self) -> DVec2 {
		let standard = || {
			DVec2::new(
				self.transform().transform_vector2((1., 0.).into()).length(),
				self.transform().transform_vector2((0., 1.).into()).length(),
			)
		};
		match self {
			GraphicElementData::VectorShape(vector_shape) => vector_shape.decompose_scale(),
			GraphicElementData::ImageFrame(image_frame) => image_frame.decompose_scale(),
			GraphicElementData::Text(_) => todo!("Transform of text"),
			GraphicElementData::GraphicGroup(_graphic_group) => standard(),
			GraphicElementData::Artboard(_artboard) => standard(),
		}
	}
}
impl TransformMut for GraphicElementData {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		match self {
			GraphicElementData::VectorShape(vector_shape) => vector_shape.transform_mut(),
			GraphicElementData::ImageFrame(image_frame) => image_frame.transform_mut(),
			GraphicElementData::Text(_) => todo!("Transform of text"),
			GraphicElementData::GraphicGroup(_graphic_group) => todo!("Mutable transform of graphic group"),
			GraphicElementData::Artboard(_artboard) => todo!("Mutable transform of artboard"),
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
pub struct Footprint {
	pub transform: DAffine2,
	pub resolution: glam::UVec2,
}

impl Default for Footprint {
	fn default() -> Self {
		Self {
			transform: DAffine2::IDENTITY,
			resolution: glam::UVec2::new(1920, 1080),
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct CullNode<VectorData> {
	pub(crate) vector_data: VectorData,
}

#[node_macro::node_fn(CullNode)]
fn cull_vector_data(footprint: Footprint, vector_data: VectorData) -> VectorData {
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
	rotate: f32,
	scale: DVec2,
	shear: DVec2,
	pivot: DVec2,
) -> Fut::Output
where
	Fut::Output: TransformMut,
{
	// TOOD: This is hack and might break for Vector data because the pivot may be incorrect
	let pivot_transform = DAffine2::from_translation(pivot);
	let transform =
		pivot_transform * DAffine2::from_scale_angle_translation(scale, rotate as f64, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]) * pivot_transform.inverse();
	let inverse = transform.inverse();
	*footprint.transform_mut() = inverse * footprint.transform();

	let mut data = self.transform_target.eval(footprint).await;
	let pivot = DAffine2::from_translation(data.local_pivot(pivot));

	let modification = pivot * DAffine2::from_scale_angle_translation(scale, rotate as f64, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]) * pivot.inverse();
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
