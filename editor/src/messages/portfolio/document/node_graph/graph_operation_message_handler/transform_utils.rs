use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::NodeInput;

/// Convert an affine transform into scale angle translation and shear, assuming shear.y = 0.
pub fn compute_scale_angle_translation_shear(transform: DAffine2) -> (DVec2, f64, DVec2, DVec2) {
	let x_axis = transform.matrix2.x_axis;
	let y_axis = transform.matrix2.y_axis;

	// Assuming there is no vertical shear
	let angle = x_axis.y.atan2(x_axis.x);
	let (sin, cos) = angle.sin_cos();
	let scale_x = if cos.abs() > 1e-10 { x_axis.x / cos } else { x_axis.y / sin };

	let mut shear_x = (sin * y_axis.y + cos * y_axis.x) / (sin * sin * scale_x + cos * cos * scale_x);
	if !shear_x.is_finite() {
		shear_x = 0.;
	}
	let scale_y = if cos.abs() > 1e-10 {
		(y_axis.y - scale_x * sin * shear_x) / cos
	} else {
		(scale_x * cos * shear_x - y_axis.x) / sin
	};
	let translation = transform.translation;
	let scale = DVec2::new(scale_x, scale_y);
	let shear = DVec2::new(shear_x, 0.);
	(scale, angle, translation, shear)
}

/// Update the inputs of the transform node to match a new transform
pub fn update_transform(inputs: &mut Vec<NodeInput>, transform: DAffine2) {
	let (scale, angle, translation, skew) = compute_scale_angle_translation_shear(transform);

	inputs[1] = NodeInput::value(TaggedValue::DVec2(translation), false);
	inputs[2] = NodeInput::value(TaggedValue::F64(angle), false);
	inputs[3] = NodeInput::value(TaggedValue::DVec2(scale), false);
	inputs[4] = NodeInput::value(TaggedValue::DVec2(skew), false);
}

/// Get the current affine transform from the transform node's inputs
pub fn get_current_transform(inputs: &[NodeInput]) -> DAffine2 {
	let translation = if let NodeInput::Value {
		tagged_value: TaggedValue::DVec2(translation),
		..
	} = inputs[1]
	{
		translation
	} else {
		DVec2::ZERO
	};
	let angle = if let NodeInput::Value {
		tagged_value: TaggedValue::F64(angle),
		..
	} = inputs[2]
	{
		angle
	} else {
		0.
	};
	let scale = if let NodeInput::Value {
		tagged_value: TaggedValue::DVec2(scale),
		..
	} = inputs[3]
	{
		scale
	} else {
		DVec2::ONE
	};
	let shear = if let NodeInput::Value {
		tagged_value: TaggedValue::DVec2(shear),
		..
	} = inputs[4]
	{
		shear
	} else {
		DVec2::ZERO
	};
	DAffine2::from_scale_angle_translation(scale, angle, translation) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.])
}

///
//   \begin{bmatrix}
//     S_{x}\cos(\theta)-S_{y}\sin(\theta)H_{y} & S_{x}\cos(\theta)H_{x}-S_{y}\sin(\theta) & T_{x}\\
//     S_{x}\sin(\theta)+S_{y}\cos(\theta)H_{y} & S_{x}\sin(\theta)H_{x}+S_{y}\cos(\theta) & T_{y}\\
//     0 & 0 & 1
//   \end{bmatrix}
#[test]
fn derive_transform() {
	for shear_x in -10..=10 {
		let shear_x = (shear_x as f64) / 2.;
		for angle in (0..=360).step_by(15) {
			let angle = (angle as f64).to_radians();
			for scale_x in 0..10 {
				let scale_x = (scale_x as f64) / 5.;
				for scale_y in 0..10 {
					if scale_x == 0. && scale_y == 0 {
						continue;
					}

					let scale_y = (scale_y as f64) / 5.;

					let shear = DVec2::new(shear_x, 0.);
					let scale = DVec2::new(scale_x, scale_y);
					let translate = DVec2::new(5666., 644.);
					let translate = DVec2::ZERO;

					let origional_transform = DAffine2::from_cols(
						DVec2::new(scale.x * angle.cos() - scale.y * angle.sin() * shear.y, scale.x * angle.sin() + scale.y * angle.cos() * shear.y),
						DVec2::new(scale.x * angle.cos() * shear.x - scale.y * angle.sin(), scale.x * angle.sin() * shear.x + scale.y * angle.cos()),
						translate,
					);

					let (new_scale, new_angle, new_translation, new_shear) = compute_scale_angle_translation_shear(origional_transform);
					let new_transform = DAffine2::from_scale_angle_translation(new_scale, new_angle, new_translation) * DAffine2::from_cols_array(&[1., new_shear.y, new_shear.x, 1., 0., 0.]);

					assert!(
						new_transform.abs_diff_eq(origional_transform, 1e-10),
						"origional_transform {} new_transform {} / scale {} new_scale {} / angle {} new_angle {} / shear {} / new_shear {}",
						origional_transform, new_transform
						scale, new_scale,
						angle, new_angle,
						shear, new_shear
					);
				}
			}
		}
	}
}
