use glam::DAffine2;

use glam::DVec2;

use crate::raster::ImageFrame;
use crate::vector::VectorData;
use crate::Node;

#[derive(Debug, Clone, Copy)]
pub struct TransformNode<Translation, Rotation, Scale, Shear, Pivot> {
	pub(crate) translate: Translation,
	pub(crate) rotate: Rotation,
	pub(crate) scale: Scale,
	pub(crate) shear: Shear,
	pub(crate) pivot: Pivot,
}

#[node_macro::node_fn(TransformNode)]
pub(crate) fn transform_vector_data(mut vector_data: VectorData, translate: DVec2, rotate: f64, scale: DVec2, shear: DVec2, pivot: DVec2) -> VectorData {
	let pivot = DAffine2::from_translation(vector_data.local_pivot(pivot));

	let modification = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]) * pivot.inverse();
	vector_data.transform = modification * vector_data.transform;

	vector_data
}

impl<'input, Translation: 'input, Rotation: 'input, Scale: 'input, Shear: 'input, Pivot: 'input> Node<'input, ImageFrame> for TransformNode<Translation, Rotation, Scale, Shear, Pivot>
where
	Translation: for<'any_input> Node<'any_input, (), Output = DVec2>,
	Rotation: for<'any_input> Node<'any_input, (), Output = f64>,
	Scale: for<'any_input> Node<'any_input, (), Output = DVec2>,
	Shear: for<'any_input> Node<'any_input, (), Output = DVec2>,
	Pivot: for<'any_input> Node<'any_input, (), Output = DVec2>,
{
	type Output = ImageFrame;
	#[inline]
	fn eval<'node: 'input>(&'node self, mut image_frame: ImageFrame) -> Self::Output {
		let translate = self.translate.eval(());
		let rotate = self.rotate.eval(());
		let scale = self.scale.eval(());
		let shear = self.shear.eval(());
		let pivot = self.pivot.eval(());

		let pivot = DAffine2::from_translation(pivot);
		let modification = pivot * DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]) * pivot.inverse();
		image_frame.transform = modification * image_frame.transform;

		image_frame
	}
}
