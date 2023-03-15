use glam::DAffine2;

use glam::DVec2;

use crate::raster::ImageFrame;
use crate::vector::VectorData;
use crate::Node;

#[derive(Debug, Clone, Copy)]
pub struct TransformNode<Translation, Rotation, Scale, Shear> {
	pub(crate) translate: Translation,
	pub(crate) rotate: Rotation,
	pub(crate) scale: Scale,
	pub(crate) shear: Shear,
}

#[node_macro::node_fn(TransformNode)]
pub(crate) fn transform_vector_data(mut vector_data: VectorData, translate: DVec2, rotate: f64, scale: DVec2, shear: DVec2) -> VectorData {
	let (sin, cos) = rotate.sin_cos();
	let transform = DAffine2::from_cols_array(&[scale.x * cos, shear.y * sin, shear.x * (-sin), scale.y * cos, translate.x, translate.y]);
	vector_data.transform = transform * vector_data.transform;
	// vector_data.transform = vector_data.transform * DAffine2::from_cols_array(&[scale.x + cos, shear.y + sin, shear.x - sin, scale.y + cos, translate.x, translate.y]);
	vector_data
}

impl<'input, Translation: 'input, Rotation: 'input, Scale: 'input, Shear: 'input> Node<'input, ImageFrame> for TransformNode<Translation, Rotation, Scale, Shear>
where
	Translation: for<'any_input> Node<'any_input, (), Output = DVec2>,
	Rotation: for<'any_input> Node<'any_input, (), Output = f64>,
	Scale: for<'any_input> Node<'any_input, (), Output = DVec2>,
	Shear: for<'any_input> Node<'any_input, (), Output = DVec2>,
{
	type Output = ImageFrame;
	#[inline]
	fn eval<'node: 'input>(&'node self, mut image_frame: ImageFrame) -> Self::Output {
		let translate = self.translate.eval(());
		let rotate = self.rotate.eval(());
		let scale = self.scale.eval(());
		let shear = self.shear.eval(());

		let shear_matrix = DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]);
		let pivot = image_frame.transform.transform_point2(DVec2::splat(0.5));
		let translate_to_center = DAffine2::from_translation(-pivot);

		let transformation = translate_to_center.inverse() * DAffine2::from_scale_angle_translation(scale, rotate, translate) * shear_matrix * translate_to_center;
		log::debug!("Affine transform: {}", transformation);
		image_frame.transform = transformation * image_frame.transform; // * DAffine2::from_cols_array(&[scale.x + cos, shear.y + sin, shear.x - sin, scale.y + cos, translate.x, translate.y]);
		image_frame
	}
}
