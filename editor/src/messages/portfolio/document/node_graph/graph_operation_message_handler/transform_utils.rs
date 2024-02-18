use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};

use bezier_rs::{ManipulatorGroup, Subpath};
use graph_craft::document::{value::TaggedValue, NodeInput};
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use glam::{DAffine2, DVec2};

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
pub fn update_transform(inputs: &mut [NodeInput], transform: DAffine2) {
	let (scale, angle, translation, shear) = compute_scale_angle_translation_shear(transform);

	inputs[1] = NodeInput::value(TaggedValue::DVec2(translation), false);
	inputs[2] = NodeInput::value(TaggedValue::F64(angle), false);
	inputs[3] = NodeInput::value(TaggedValue::DVec2(scale), false);
	inputs[4] = NodeInput::value(TaggedValue::DVec2(shear), false);
}

// TODO: This should be extracted from the graph at the location of the transform node.
pub struct LayerBounds {
	pub bounds: [DVec2; 2],
	pub bounds_transform: DAffine2,
	pub layer_transform: DAffine2,
}

impl LayerBounds {
	/// Extract the layer bounds and their transform for a layer.
	pub fn new(metadata: &DocumentMetadata, layer: LayerNodeIdentifier) -> Self {
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

/// Extract the current normalized pivot from the layer
pub fn get_current_normalized_pivot(inputs: &[NodeInput]) -> DVec2 {
	if let NodeInput::Value {
		tagged_value: TaggedValue::DVec2(pivot),
		..
	} = inputs[5]
	{
		pivot
	} else {
		DVec2::splat(0.5)
	}
}

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
fn subpath_bounds(subpaths: &[Subpath<ManipulatorGroupId>]) -> [DVec2; 2] {
	subpaths
		.iter()
		.filter_map(|subpath| subpath.bounding_box())
		.reduce(|b1, b2| [b1[0].min(b2[0]), b1[1].max(b2[1])])
		.unwrap_or_default()
}

/// Returns corners of all subpaths (but expanded to avoid division-by-zero errors)
pub fn nonzero_subpath_bounds(subpaths: &[Subpath<ManipulatorGroupId>]) -> [DVec2; 2] {
	let [bounds_min, bounds_max] = subpath_bounds(subpaths);
	clamp_bounds(bounds_min, bounds_max)
}

pub struct VectorModificationState<'a> {
	pub subpaths: &'a mut Vec<Subpath<ManipulatorGroupId>>,
	pub mirror_angle_groups: &'a mut Vec<ManipulatorGroupId>,
}
impl<'a> VectorModificationState<'a> {
	fn insert_start(&mut self, subpath_index: usize, manipulator_group: ManipulatorGroup<ManipulatorGroupId>) {
		self.subpaths[subpath_index].insert_manipulator_group(0, manipulator_group)
	}

	fn insert_end(&mut self, subpath_index: usize, manipulator_group: ManipulatorGroup<ManipulatorGroupId>) {
		let subpath = &mut self.subpaths[subpath_index];
		subpath.insert_manipulator_group(subpath.len(), manipulator_group)
	}

	fn insert(&mut self, manipulator_group: ManipulatorGroup<ManipulatorGroupId>, after_id: ManipulatorGroupId) {
		for subpath in self.subpaths.iter_mut() {
			if let Some(index) = subpath.manipulator_index_from_id(after_id) {
				subpath.insert_manipulator_group(index + 1, manipulator_group);
				break;
			}
		}
	}

	fn remove_group(&mut self, id: ManipulatorGroupId) {
		for subpath in self.subpaths.iter_mut() {
			if let Some(index) = subpath.manipulator_index_from_id(id) {
				subpath.remove_manipulator_group(index);
				break;
			}
		}
	}

	fn remove_point(&mut self, point: ManipulatorPointId) {
		for subpath in self.subpaths.iter_mut() {
			if point.manipulator_type == SelectedType::Anchor {
				if let Some(index) = subpath.manipulator_index_from_id(point.group) {
					subpath.remove_manipulator_group(index);
					break;
				}
			} else if let Some(group) = subpath.manipulator_mut_from_id(point.group) {
				if point.manipulator_type == SelectedType::InHandle {
					group.in_handle = None;
				} else if point.manipulator_type == SelectedType::OutHandle {
					group.out_handle = None;
				}
			}
		}
	}

	fn set_mirror(&mut self, id: ManipulatorGroupId, mirror_angle: bool) {
		if !mirror_angle {
			self.mirror_angle_groups.retain(|&mirrored_id| mirrored_id != id);
		} else if !self.mirror_angle_groups.contains(&id) {
			self.mirror_angle_groups.push(id);
		}
	}

	fn toggle_mirror(&mut self, id: ManipulatorGroupId) {
		if self.mirror_angle_groups.contains(&id) {
			self.mirror_angle_groups.retain(|&mirrored_id| mirrored_id != id);
		} else {
			self.mirror_angle_groups.push(id);
		}
	}

	fn set_position(&mut self, point: ManipulatorPointId, position: DVec2) {
		assert!(position.is_finite(), "Point position should be finite");
		for subpath in self.subpaths.iter_mut() {
			if let Some(manipulator) = subpath.manipulator_mut_from_id(point.group) {
				match point.manipulator_type {
					SelectedType::Anchor => manipulator.anchor = position,
					SelectedType::InHandle => manipulator.in_handle = Some(position),
					SelectedType::OutHandle => manipulator.out_handle = Some(position),
				}
				if point.manipulator_type != SelectedType::Anchor && self.mirror_angle_groups.contains(&point.group) {
					let reflect = |opposite: DVec2| {
						(manipulator.anchor - position)
							.try_normalize()
							.map(|direction| direction * (opposite - manipulator.anchor).length() + manipulator.anchor)
							.unwrap_or(opposite)
					};
					match point.manipulator_type {
						SelectedType::InHandle => manipulator.out_handle = manipulator.out_handle.map(reflect),
						SelectedType::OutHandle => manipulator.in_handle = manipulator.in_handle.map(reflect),
						_ => {}
					}
				}

				break;
			}
		}
	}

	pub fn modify(&mut self, modification: VectorDataModification) {
		match modification {
			VectorDataModification::AddEndManipulatorGroup { subpath_index, manipulator_group } => self.insert_end(subpath_index, manipulator_group),
			VectorDataModification::AddStartManipulatorGroup { subpath_index, manipulator_group } => self.insert_start(subpath_index, manipulator_group),
			VectorDataModification::AddManipulatorGroup { manipulator_group, after_id } => self.insert(manipulator_group, after_id),
			VectorDataModification::RemoveManipulatorGroup { id } => self.remove_group(id),
			VectorDataModification::RemoveManipulatorPoint { point } => self.remove_point(point),
			VectorDataModification::SetClosed { index, closed } => self.subpaths[index].set_closed(closed),
			VectorDataModification::SetManipulatorHandleMirroring { id, mirror_angle } => self.set_mirror(id, mirror_angle),
			VectorDataModification::SetManipulatorPosition { point, position } => self.set_position(point, position),
			VectorDataModification::ToggleManipulatorHandleMirroring { id } => self.toggle_mirror(id),
			VectorDataModification::UpdateSubpaths { subpaths } => *self.subpaths = subpaths,
		}
	}
}
