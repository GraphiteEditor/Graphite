use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface};
use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::transform::Transform;

/// Update the inputs of the transform node to match a new transform, keeping each in the unit it was typed in.
pub fn update_transform(network_interface: &mut NodeNetworkInterface, node_id: &NodeId, transform: DAffine2) {
	use graphene_std::transform_nodes::transform::*;

	let (rotation, scale, skew) = transform.decompose_rotation_scale_skew();
	let translation = transform.translation;

	let rotation = rotation.to_degrees();
	let skew = DVec2::new(skew.atan().to_degrees(), 0.);

	let pixels_per_inch = network_interface.pixels_per_inch();
	let [translation_input, rotation_input, scale_input, skew_input] = [
		InputConnector::node(*node_id, TranslationInput),
		InputConnector::node(*node_id, RotationInput),
		InputConnector::node(*node_id, ScaleInput),
		InputConnector::node(*node_id, SkewInput),
	];
	let existing = |input: &InputConnector| network_interface.input_from_connector(input, &[]).and_then(NodeInput::as_value).cloned().unwrap_or(TaggedValue::None);
	let (existing_translation, existing_rotation, existing_skew) = (existing(&translation_input), existing(&rotation_input), existing(&skew_input));

	// An input already holding the value is left as typed, so rotating a layer doesn't rewrite a translation typed in millimeters with float noise
	let unchanged = |existing: Option<DVec2>, value: DVec2| existing.is_some_and(|existing| existing.abs_diff_eq(value, 1e-9));

	if !unchanged(existing_translation.as_length_vec2_pixels(pixels_per_inch), translation) {
		let translation = existing_translation.length_vec2_in_same_unit(translation, pixels_per_inch);
		network_interface.set_input(&translation_input, NodeInput::value(translation, false), &[]);
	}
	if !unchanged(existing_rotation.as_angle_degrees().map(DVec2::splat), DVec2::splat(rotation)) {
		network_interface.set_input(&rotation_input, NodeInput::value(existing_rotation.angle_in_same_unit(rotation), false), &[]);
	}
	network_interface.set_input(&scale_input, NodeInput::value(TaggedValue::DVec2(scale), false), &[]);
	if !unchanged(existing_skew.as_angle_vec2_degrees(), skew) {
		network_interface.set_input(&skew_input, NodeInput::value(existing_skew.angle_vec2_in_same_unit(skew), false), &[]);
	}
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

/// Get the current affine transform from the transform node's inputs, sizing a translation in a physical unit by the document's resolution.
pub fn get_current_transform(inputs: &[NodeInput], pixels_per_inch: f64) -> DAffine2 {
	let value = |index: usize| inputs.get(index).and_then(NodeInput::as_value);

	let translation = value(1).and_then(|value| value.as_length_vec2_pixels(pixels_per_inch)).unwrap_or(DVec2::ZERO);
	let rotation = value(2).and_then(TaggedValue::as_angle_degrees).unwrap_or(0.);
	let scale = if let Some(&TaggedValue::DVec2(scale)) = value(3) { scale } else { DVec2::ONE };
	let skew = value(4).and_then(TaggedValue::as_angle_vec2_degrees).unwrap_or(DVec2::ZERO);

	let rotation = rotation.to_radians();
	let skew = DVec2::new(skew.x.to_radians().tan(), skew.y.to_radians().tan());

	DAffine2::from_scale_angle_translation(scale, rotation, translation) * DAffine2::from_cols_array(&[1., skew.y, skew.x, 1., 0., 0.])
}

/// Extract the current normalized pivot from the layer
pub fn get_current_normalized_pivot(inputs: &[NodeInput]) -> DVec2 {
	if let Some(&TaggedValue::DVec2(pivot)) = inputs[5].as_value() { pivot } else { DVec2::splat(0.5) }
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
		for skew_x in -10..=10 {
			let skew_x = (skew_x as f64) / 2.;
			for angle in (0..=360).step_by(15) {
				let angle = (angle as f64).to_radians();
				for scale_x in 1..10 {
					let scale_x = (scale_x as f64) / 5.;
					for scale_y in 1..10 {
						let scale_y = (scale_y as f64) / 5.;

						let skew = DVec2::new(skew_x, 0.);
						let scale = DVec2::new(scale_x, scale_y);
						let translate = DVec2::new(5666., 644.);

						let original_transform = DAffine2::from_cols(
							DVec2::new(scale.x * angle.cos() - scale.y * angle.sin() * skew.y, scale.x * angle.sin() + scale.y * angle.cos() * skew.y),
							DVec2::new(scale.x * angle.cos() * skew.x - scale.y * angle.sin(), scale.x * angle.sin() * skew.x + scale.y * angle.cos()),
							translate,
						);

						let (new_angle, new_scale, new_skew) = original_transform.decompose_rotation_scale_skew();
						let new_translation = original_transform.translation;
						let new_transform = DAffine2::from_scale_angle_translation(new_scale, new_angle, new_translation) * DAffine2::from_cols_array(&[1., 0., new_skew, 1., 0., 0.]);

						assert!(
							new_transform.abs_diff_eq(original_transform, 1e-10),
							"original_transform {original_transform} new_transform {new_transform} / scale {scale} new_scale {new_scale} / angle {angle} new_angle {new_angle} / skew {skew} / new_skew {new_skew}",
						);
					}
				}
			}
		}
	}
}
