use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface};
use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::subpath::Subpath;
use graphene_std::vector::PointId;

/// Convert an affine transform into the tuple `(scale, angle, translation, shear)` assuming `shear.y = 0`.
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
pub fn update_transform(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, transform: DAffine2) {
	let (scale, rotation, translation, shear) = compute_scale_angle_translation_shear(transform);

	let rotation = rotation.to_degrees();
	let shear = DVec2::new(shear.x.atan().to_degrees(), shear.y.atan().to_degrees());

	network_interface.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::DVec2(translation), false), &[]);
	network_interface.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::F64(rotation), false), &[]);
	network_interface.set_input(&InputConnector::node(*node_id, 3), NodeInput::value(TaggedValue::DVec2(scale), false), &[]);
	network_interface.set_input(&InputConnector::node(*node_id, 4), NodeInput::value(TaggedValue::DVec2(shear), false), &[]);
}

// TODO: This should be extracted from the graph at the location of the transform node.
pub struct LayerBounds {
	pub bounds: [DVec2; 2],
	pub bounds_transform: DAffine2,
	pub layer_transform: DAffine2,
}

impl LayerBounds {
	/// Extract the layer bounds and their transform for a layer.
	pub fn new(
		metadata: &crate::messages::portfolio::document::utility_types::document_metadata::DocumentMetadata,
		layer: crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier,
	) -> Self {
		Self {
			bounds: metadata.nonzero_bounding_box(layer),
			bounds_transform: DAffine2::IDENTITY,
			layer_transform: metadata.transform_to_document(layer),
		}
	}

	pub fn layerspace_pivot(&self, normalized_pivot: DVec2) -> DVec2 {
		self.bounds[0] + (self.bounds[1] - self.bounds[0]) * normalized_pivot
	}

	pub fn local_pivot(&self, normalized_pivot: DVec2) -> DVec2 {
		self.bounds_transform.transform_point2(self.layerspace_pivot(normalized_pivot))
	}
}

/// Get the current affine transform from the transform node's inputs
pub fn get_current_transform(inputs: &[NodeInput]) -> DAffine2 {
	let translation = if let Some(&TaggedValue::DVec2(translation)) = inputs[1].as_value() {
		translation
	} else {
		DVec2::ZERO
	};
	let rotation = if let Some(&TaggedValue::F64(rotation)) = inputs[2].as_value() { rotation } else { 0. };
	let scale = if let Some(&TaggedValue::DVec2(scale)) = inputs[3].as_value() { scale } else { DVec2::ONE };
	let shear = if let Some(&TaggedValue::DVec2(shear)) = inputs[4].as_value() { shear } else { DVec2::ZERO };

	let rotation = rotation.to_radians();
	let shear = DVec2::new(shear.x.to_radians().tan(), shear.y.to_radians().tan());

	DAffine2::from_scale_angle_translation(scale, rotation, translation) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.])
}

/// Extract the current normalized pivot from the layer
pub fn get_current_normalized_pivot(inputs: &[NodeInput]) -> DVec2 {
	if let Some(&TaggedValue::DVec2(pivot)) = inputs[5].as_value() { pivot } else { DVec2::splat(0.5) }
}

/// Expand a bounds to avoid div zero errors
fn clamp_bounds(bounds_min: DVec2, mut bounds_max: DVec2) -> [DVec2; 2] {
	let bounds_size = bounds_max - bounds_min;
	if bounds_size.x < 1e-10 {
		bounds_max.x = bounds_min.x + 1.;
	}
	if bounds_size.y < 1e-10 {
		bounds_max.y = bounds_min.y + 1.;
	}
	[bounds_min, bounds_max]
}
/// Returns corners of all subpaths
fn subpath_bounds(subpaths: &[Subpath<PointId>]) -> [DVec2; 2] {
	subpaths
		.iter()
		.filter_map(|subpath| subpath.bounding_box())
		.reduce(|b1, b2| [b1[0].min(b2[0]), b1[1].max(b2[1])])
		.unwrap_or_default()
}

/// Returns corners of all subpaths (but expanded to avoid division-by-zero errors)
pub fn nonzero_subpath_bounds(subpaths: &[Subpath<PointId>]) -> [DVec2; 2] {
	let [bounds_min, bounds_max] = subpath_bounds(subpaths);
	clamp_bounds(bounds_min, bounds_max)
}

#[cfg(test)]
mod tests {
	use super::*;
	/// ![](https://files.keavon.com/-/OptimisticSpotlessTinamou/capture.png)
	///
	/// Source:
	/// ```tex
	/// \begin{bmatrix}
	/// S_{x}\cos(\theta)-S_{y}\sin(\theta)H_{y} & S_{x}\cos(\theta)H_{x}-S_{y}\sin(\theta) & T_{x}\\
	/// S_{x}\sin(\theta)+S_{y}\cos(\theta)H_{y} & S_{x}\sin(\theta)H_{x}+S_{y}\cos(\theta) & T_{y}\\
	/// 0 & 0 & 1
	/// \end{bmatrix}
	/// ```
	#[test]
	fn derive_transform() {
		for shear_x in -10..=10 {
			let shear_x = (shear_x as f64) / 2.;
			for angle in (0..=360).step_by(15) {
				let angle = (angle as f64).to_radians();
				for scale_x in 1..10 {
					let scale_x = (scale_x as f64) / 5.;
					for scale_y in 1..10 {
						let scale_y = (scale_y as f64) / 5.;

						let shear = DVec2::new(shear_x, 0.);
						let scale = DVec2::new(scale_x, scale_y);
						let translate = DVec2::new(5666., 644.);

						let original_transform = DAffine2::from_cols(
							DVec2::new(scale.x * angle.cos() - scale.y * angle.sin() * shear.y, scale.x * angle.sin() + scale.y * angle.cos() * shear.y),
							DVec2::new(scale.x * angle.cos() * shear.x - scale.y * angle.sin(), scale.x * angle.sin() * shear.x + scale.y * angle.cos()),
							translate,
						);

						let (new_scale, new_angle, new_translation, new_shear) = compute_scale_angle_translation_shear(original_transform);
						let new_transform = DAffine2::from_scale_angle_translation(new_scale, new_angle, new_translation) * DAffine2::from_cols_array(&[1., new_shear.y, new_shear.x, 1., 0., 0.]);

						assert!(
							new_transform.abs_diff_eq(original_transform, 1e-10),
							"original_transform {original_transform} new_transform {new_transform} / scale {scale} new_scale {new_scale} / angle {angle} new_angle {new_angle} / shear {shear} / new_shear {new_shear}",
						);
					}
				}
			}
		}
	}
}
