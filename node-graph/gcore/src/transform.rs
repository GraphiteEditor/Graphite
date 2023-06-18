use glam::DAffine2;

use glam::DVec2;

use crate::raster::ImageFrame;
use crate::raster::Pixel;
use crate::vector::VectorData;
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
pub struct TransformNode<Translation, Rotation, Scale, Shear, Pivot> {
	pub(crate) translate: Translation,
	pub(crate) rotate: Rotation,
	pub(crate) scale: Scale,
	pub(crate) shear: Shear,
	pub(crate) pivot: Pivot,
}

#[node_macro::node_fn(TransformNode)]
pub(crate) fn transform_vector_data<Data: TransformMut>(mut data: Data, translate: DVec2, rotate: f32, scale: DVec2, shear: DVec2, pivot: DVec2) -> Data {
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
