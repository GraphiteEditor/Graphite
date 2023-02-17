use crate::messages::prelude::*;

use bezier_rs::TValue;
use document_legacy::{LayerId, Operation};
use graphene_std::vector::consts::ManipulatorType;
use graphene_std::vector::manipulator_group::ManipulatorGroup;
use graphene_std::vector::manipulator_point::ManipulatorPoint;
use graphene_std::vector::subpath::{BezierId, Subpath};

use document_legacy::document::Document;
use glam::DVec2;

/// ShapeEditor is the container for all of the layer paths that are represented as [Subpath]s and provides
/// functionality required to query and create the [Subpath] / [ManipulatorGroup]s / [ManipulatorPoint]s.
///
/// Overview:
/// ```text
///              ShapeEditor
///                   |
///            selected_layers               <- Paths to selected layers that may contain Subpaths
///              /    |    \
///          Subpath ... Subpath             <- Reference from layer paths, one Subpath per layer (for now, will eventually be a CompoundPath)
///            /      |      \
/// ManipulatorGroup ... ManipulatorGroup    <- Subpath contains many ManipulatorGroups
/// ```
#[derive(Clone, Debug, Default)]
pub struct ShapeEditor {
	// The layers we can select and edit manipulators (anchors and handles) from
	selected_layers: Vec<Vec<LayerId>>,
}

pub struct SelectedPointsInfo<'a> {
	pub points: Vec<ManipulatorPointInfo<'a>>,
	pub offset: DVec2,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ManipulatorPointInfo<'a> {
	pub shape_layer_path: &'a [LayerId],
	pub manipulator_group_id: u64,
	pub manipulator_type: ManipulatorType,
}

// TODO Consider keeping a list of selected manipulators to minimize traversals of the layers
impl ShapeEditor {
	/// Select the first point within the selection threshold.
	/// Returns a tuple of the points if found and the offset, or `None` otherwise.
	pub fn select_point(&self, document: &Document, mouse_position: DVec2, select_threshold: f64, add_to_selection: bool, responses: &mut VecDeque<Message>) -> Option<SelectedPointsInfo> {
		if self.selected_layers.is_empty() {
			return None;
		}

		if let Some((shape_layer_path, manipulator_group_id, manipulator_point_index)) = self.find_nearest_point_indices(document, mouse_position, select_threshold) {
			trace!("Selecting... manipulator group ID: {}, manipulator point index: {}", manipulator_group_id, manipulator_point_index);

			// If the point we're selecting has already been selected
			// we can assume this point exists.. since we did just click on it hence the unwrap
			let is_point_selected = self.shape(document, shape_layer_path).unwrap().manipulator_groups().by_id(manipulator_group_id).unwrap().points[manipulator_point_index]
				.as_ref()
				.unwrap()
				.editor_state
				.is_selected;

			let point_position = self.shape(document, shape_layer_path).unwrap().manipulator_groups().by_id(manipulator_group_id).unwrap().points[manipulator_point_index]
				.as_ref()
				.unwrap()
				.position;

			// The currently selected points (which are then modified to reflect the selection)
			let mut points = self
				.selected_layers()
				.iter()
				.filter_map(|path| document.layer(path).ok().map(|layer| (path, layer)))
				.filter_map(|(path, shape)| shape.as_subpath().map(|subpath| (path, subpath)))
				.flat_map(|(path, shape)| {
					shape
						.manipulator_groups()
						.enumerate()
						.filter(|(_id, manipulator_group)| manipulator_group.is_anchor_selected())
						.flat_map(|(id, manipulator_group)| manipulator_group.selected_points().map(move |point| (id, point.manipulator_type)))
						.map(|(anchor, manipulator_point)| ManipulatorPointInfo {
							shape_layer_path: path.as_slice(),
							manipulator_group_id: *anchor,
							manipulator_type: manipulator_point,
						})
				})
				.collect::<Vec<_>>();

			// Should we select or deselect the point?
			let should_select = if is_point_selected { !add_to_selection } else { true };

			// This is selecting the manipulator only for now, next to generalize to points
			if should_select {
				// If we're replacing the selection, clear all points in other selected shapes
				let add = add_to_selection || is_point_selected;
				if !add {
					points.clear();
					responses.push_back(DocumentMessage::DeselectAllManipulatorPoints.into());
				}

				// Add to the selected points
				let point_info = ManipulatorPointInfo {
					shape_layer_path,
					manipulator_group_id,
					manipulator_type: ManipulatorType::from_index(manipulator_point_index),
				};
				points.push(point_info);
				responses.push_back(
					Operation::SelectManipulatorPoints {
						layer_path: shape_layer_path.to_vec(),
						point_ids: vec![(point_info.manipulator_group_id, point_info.manipulator_type)],
						add,
					}
					.into(),
				);

				// Offset to snap the selected point to the cursor
				let offset = document
					.generate_transform_relative_to_viewport(shape_layer_path)
					.map(|viewspace| mouse_position - viewspace.transform_point2(point_position))
					.unwrap_or_default();

				return Some(SelectedPointsInfo { points, offset });
			} else {
				responses.push_back(
					Operation::DeselectManipulatorPoints {
						layer_path: shape_layer_path.to_vec(),
						point_ids: vec![(manipulator_group_id, ManipulatorType::from_index(manipulator_point_index))],
					}
					.into(),
				);
				points.retain(|x| {
					*x != ManipulatorPointInfo {
						shape_layer_path,
						manipulator_group_id,
						manipulator_type: ManipulatorType::from_index(manipulator_point_index),
					}
				});

				return None;
			}
		}

		// Deselect all points if no nearby point
		responses.push_back(DocumentMessage::DeselectAllManipulatorPoints.into());
		None
	}

	/// A wrapper for `find_nearest_point_indices()` and returns a [ManipulatorPoint].
	pub fn find_nearest_point<'a>(&'a self, document: &'a Document, mouse_position: DVec2, select_threshold: f64) -> Option<&'a ManipulatorPoint> {
		let (shape_layer_path, manipulator_group_id, manipulator_point_index) = self.find_nearest_point_indices(document, mouse_position, select_threshold)?;
		let selected_shape = self.shape(document, shape_layer_path).unwrap();
		if let Some(manipulator_group) = selected_shape.manipulator_groups().by_id(manipulator_group_id) {
			return manipulator_group.points[manipulator_point_index].as_ref();
		}
		None
	}

	/// Set the shapes we consider for selection, we will choose draggable manipulators from these shapes.
	pub fn set_selected_layers(&mut self, target_layers: Vec<Vec<LayerId>>) {
		self.selected_layers = target_layers;
	}

	pub fn selected_layers(&self) -> &Vec<Vec<LayerId>> {
		&self.selected_layers
	}

	pub fn selected_layers_ref(&self) -> Vec<&[LayerId]> {
		self.selected_layers.iter().map(|l| l.as_slice()).collect::<Vec<_>>()
	}

	/// Clear all of the shapes we can modify.
	pub fn clear_selected_layers(&mut self) {
		self.selected_layers.clear();
	}

	pub fn has_selected_layers(&self) -> bool {
		!self.selected_layers.is_empty()
	}

	/// Provide the currently selected manipulators by reference.
	pub fn selected_manipulator_groups<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a ManipulatorGroup> {
		self.iter(document).flat_map(|shape| shape.selected_manipulator_groups())
	}

	/// A mutable iterator of all the manipulators, regardless of selection.
	pub fn manipulator_groups<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a ManipulatorGroup> {
		self.iter(document).flat_map(|shape| shape.manipulator_groups().iter())
	}

	/// Provide the currently selected points by reference.
	pub fn selected_points<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a ManipulatorPoint> {
		self.selected_manipulator_groups(document).flat_map(|manipulator_group| manipulator_group.selected_points())
	}

	/// Move the selected points by dragging the mouse.
	pub fn move_selected_points(&self, delta: DVec2, mirror_distance: bool, responses: &mut VecDeque<Message>) {
		for layer_path in &self.selected_layers {
			responses.push_back(
				DocumentMessage::MoveSelectedManipulatorPoints {
					layer_path: layer_path.clone(),
					delta: (delta.x, delta.y),
					mirror_distance,
				}
				.into(),
			);
		}
	}

	/// The opposing handle lengths.
	pub fn opposing_handle_lengths(&self, document: &Document) -> HashMap<Vec<LayerId>, HashMap<u64, f64>> {
		self.selected_layers()
			.iter()
			.filter_map(|path| document.layer(path).ok().map(|layer| (path, layer)))
			.filter_map(|(path, shape)| shape.as_subpath().map(|subpath| (path, subpath)))
			.map(|(path, shape)| {
				let opposing_handle_lengths = shape
					.manipulator_groups()
					.enumerate()
					.filter_map(|(id, manipulator_group)| {
						// We will keep track of the opposing handle length when:
						// i) Both handles exist and exactly one is selected.
						// ii) The anchor is not selected.
						// iii) We have to mirror the angle between handles.

						if !manipulator_group.editor_state.mirror_angle_between_handles {
							return None;
						}

						let mut selected_handles = manipulator_group.selected_handles();
						let handle = selected_handles.next()?;

						// Check that handle is the only selected handle.
						if selected_handles.next().is_none() {
							let opposing_handle_position = manipulator_group.opposing_handle(handle)?.position;
							let anchor = manipulator_group.points[ManipulatorType::Anchor].as_ref()?;
							if !anchor.is_selected() {
								let opposing_handle_length = opposing_handle_position.distance(anchor.position);
								Some((*id, opposing_handle_length))
							} else {
								None
							}
						} else {
							None
						}
					})
					.collect::<HashMap<_, _>>();
				(path.clone(), opposing_handle_lengths)
			})
			.collect::<HashMap<_, _>>()
	}

	/// Reset the opposing handle lengths.
	pub fn reset_opposing_handle_lengths(&self, document: &Document, opposing_handle_lengths: &HashMap<Vec<LayerId>, HashMap<u64, f64>>, responses: &mut VecDeque<Message>) {
		self.selected_layers()
			.iter()
			.filter_map(|path| document.layer(path).ok().map(|layer| (path, layer)))
			.filter_map(|(path, shape)| shape.as_subpath().map(|subpath| (path, subpath)))
			.filter_map(|(path, shape)| opposing_handle_lengths.get(path).map(|layer_opposing_handle_lengths| (path, shape, layer_opposing_handle_lengths)))
			.flat_map(|(path, shape, layer_opposing_handle_lengths)| {
				shape
					.manipulator_groups()
					.enumerate()
					.map(move |(id, manipulator_group)| (path, layer_opposing_handle_lengths, id, manipulator_group))
			})
			.for_each(|(path, layer_opposing_handle_lengths, id, manipulator_group)| {
				if !manipulator_group.editor_state.mirror_angle_between_handles {
					return;
				}

				let opposing_handle_length = if let Some(length) = layer_opposing_handle_lengths.get(id) {
					length
				} else {
					return;
				};

				let mut selected_handles = manipulator_group.selected_handles();
				let handle = if let Some(handle) = selected_handles.next() {
					handle
				} else {
					return;
				};

				// Check that handle is the only selected handle.
				if selected_handles.next().is_none() {
					let opposing_handle = if let Some(opposing_handle) = manipulator_group.opposing_handle(handle) {
						opposing_handle
					} else {
						return;
					};

					let anchor = if let Some(anchor) = manipulator_group.points[ManipulatorType::Anchor].as_ref() {
						anchor
					} else {
						return;
					};
					if anchor.is_selected() {
						return;
					}

					if let Some(offset) = (opposing_handle.position - anchor.position).try_normalize() {
						let new_opposing_handle_position = anchor.position + offset * (*opposing_handle_length);
						assert!(new_opposing_handle_position.is_finite(), "Opposing handle not finite!");
						responses.push_back(
							Operation::MoveManipulatorPoint {
								layer_path: path.clone(),
								id: *id,
								manipulator_type: opposing_handle.manipulator_type,
								position: new_opposing_handle_position.into(),
							}
							.into(),
						);
					}
				}
			});
	}

	/// Dissolve the selected points.
	pub fn delete_selected_points(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::DeleteSelectedManipulatorPoints.into());
	}

	/// Toggle if the handles should mirror angle across the anchor position.
	pub fn toggle_handle_mirroring_on_selected(&self, toggle_angle: bool, responses: &mut VecDeque<Message>) {
		for layer_path in &self.selected_layers {
			responses.push_back(
				DocumentMessage::ToggleSelectedHandleMirroring {
					layer_path: layer_path.clone(),
					toggle_angle,
				}
				.into(),
			);
		}
	}

	/// Deselect all manipulators from the shapes that the manipulation handler has created.
	pub fn deselect_all_points(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::DeselectAllManipulatorPoints.into());
	}

	/// Iterate over the shapes.
	pub fn iter<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a Subpath> + 'a {
		self.selected_layers.iter().flat_map(|layer_id| document.layer(layer_id)).filter_map(|shape| shape.as_subpath())
	}

	/// Find a [ManipulatorPoint] that is within the selection threshold and return the layer path, an index to the [ManipulatorGroup], and an enum index for [ManipulatorPoint].
	/// Return value is an `Option` of the tuple representing `(layer path, ManipulatorGroup ID, ManipulatorType enum index)`.
	fn find_nearest_point_indices(&self, document: &Document, mouse_position: DVec2, select_threshold: f64) -> Option<(&[LayerId], u64, usize)> {
		if self.selected_layers.is_empty() {
			return None;
		}

		let select_threshold_squared = select_threshold * select_threshold;
		// Find the closest control point among all elements of shapes_to_modify
		for layer in self.selected_layers.iter() {
			if let Some((manipulator_id, manipulator_point_index, distance_squared)) = self.closest_point_in_layer(document, layer, mouse_position) {
				// Choose the first point under the threshold
				if distance_squared < select_threshold_squared {
					trace!("Selecting... manipulator ID: {}, manipulator point index: {}", manipulator_id, manipulator_point_index);
					return Some((layer, manipulator_id, manipulator_point_index));
				}
			}
		}

		None
	}

	// TODO Use quadtree or some equivalent spatial acceleration structure to improve this to O(log(n))
	/// Find the closest manipulator, manipulator point, and distance so we can select path elements.
	/// Brute force comparison to determine which manipulator (handle or anchor) we want to select taking O(n) time.
	/// Return value is an `Option` of the tuple representing `(manipulator ID, manipulator point index, distance squared)`.
	fn closest_point_in_layer(&self, document: &Document, layer_path: &[LayerId], pos: glam::DVec2) -> Option<(u64, usize, f64)> {
		let mut closest_distance_squared: f64 = f64::MAX; // Not ideal
		let mut result: Option<(u64, usize, f64)> = None;

		if let Some(shape) = document.layer(layer_path).ok()?.as_subpath() {
			let viewspace = document.generate_transform_relative_to_viewport(layer_path).ok()?;
			for (manipulator_id, manipulator) in shape.manipulator_groups().enumerate() {
				let manipulator_point_index = manipulator.closest_point(&viewspace, pos, crate::consts::HIDE_HANDLE_DISTANCE);
				if let Some(point) = &manipulator.points[manipulator_point_index] {
					if point.editor_state.can_be_selected {
						let distance_squared = viewspace.transform_point2(point.position).distance_squared(pos);
						if distance_squared < closest_distance_squared {
							closest_distance_squared = distance_squared;
							result = Some((*manipulator_id, manipulator_point_index, distance_squared));
						}
					}
				}
			}
		}
		result
	}

	/// Find the `t` value along the path segment we have clicked upon, together with that segment ID.
	///
	/// Returns a tuple of [`BezierId`] and `t` as an f64.
	fn closest_segment(&self, document: &Document, layer_path: &[LayerId], position: glam::DVec2, tolerance: f64) -> Option<(BezierId, f64)> {
		let transform = document.generate_transform_relative_to_viewport(layer_path).ok()?;
		let layer_pos = transform.inverse().transform_point2(position);
		let projection_options = bezier_rs::ProjectionOptions { lut_size: 5, ..Default::default() };

		let mut result: Option<(BezierId, f64)> = None;
		let mut closest_distance_squared: f64 = tolerance * tolerance;

		for bezier_id in document.layer(layer_path).ok()?.as_subpath()?.bezier_iter() {
			let bezier = bezier_id.internal;
			let t = bezier.project(layer_pos, projection_options);
			let layerspace = bezier.evaluate(TValue::Parametric(t));

			let screenspace = transform.transform_point2(layerspace);
			let distance_squared = screenspace.distance_squared(position);

			if distance_squared < closest_distance_squared {
				closest_distance_squared = distance_squared;
				result = Some((bezier_id, t));
			}
		}

		result
	}

	/// Handles the splitting of a curve to insert new points (which can be activated by double clicking on a curve with the Path tool).
	pub fn split(&self, document: &Document, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) {
		for layer_path in &self.selected_layers {
			if let Some((bezier_id, t)) = self.closest_segment(document, layer_path, position, tolerance) {
				let [first, second] = bezier_id.internal.split(TValue::Parametric(t));

				// Adjust the first manipulator group's out handle
				let out_handle = Operation::SetManipulatorPoints {
					layer_path: layer_path.clone(),
					id: bezier_id.start,
					manipulator_type: ManipulatorType::OutHandle,
					position: first.handle_start().map(|p| p.into()),
				};

				// Insert a new manipulator group between the existing ones
				let insert = Operation::InsertManipulatorGroup {
					layer_path: layer_path.clone(),
					manipulator_group: ManipulatorGroup::new_with_handles(first.end(), first.handle_end(), second.handle_start()),
					after_id: bezier_id.end,
				};

				// Adjust the last manipulator group's in handle
				let in_handle = Operation::SetManipulatorPoints {
					layer_path: layer_path.clone(),
					id: bezier_id.end,
					manipulator_type: ManipulatorType::InHandle,
					position: second.handle_end().map(|p| p.into()),
				};

				responses.extend([out_handle.into(), insert.into(), in_handle.into()]);
				return;
			}
		}
	}

	/// Handles the flipping between sharp corner and smooth (which can be activated by double clicking on an anchor with the Path tool).
	pub fn flip_sharp(&self, document: &Document, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) -> bool {
		let mut process_layer = |layer_path| {
			let manipulator_groups = document.layer(layer_path).ok()?.as_subpath()?.manipulator_groups();

			let transform_to_screenspace = document.generate_transform_relative_to_viewport(layer_path).ok()?;
			let mut result = None;
			let mut closest_distance_squared = tolerance * tolerance;

			// Find the closest anchor point on the current layer
			for (index, (&bezier_id, group)) in manipulator_groups.enumerate().enumerate() {
				if let Some(anchor) = &group.points[ManipulatorType::Anchor as usize] {
					let screenspace = transform_to_screenspace.transform_point2(anchor.position);
					let distance_squared = screenspace.distance_squared(position);

					if distance_squared < closest_distance_squared {
						closest_distance_squared = distance_squared;
						result = Some((anchor.position, index, bezier_id, group));
					}
				}
			}
			let (anchor_position, index, bezier_id, group) = result?;

			// Check by comparing the handle positions to the anchor if this maniuplator group is a point
			let already_sharp = match &group.points {
				[_, Some(in_handle), Some(out_handle)] => anchor_position.abs_diff_eq(in_handle.position, f64::EPSILON * 100.) && anchor_position.abs_diff_eq(out_handle.position, f64::EPSILON * 100.),
				[_, Some(handle), None] | [_, None, Some(handle)] => anchor_position.abs_diff_eq(handle.position, f64::EPSILON * 100.),
				[_, None, None] => true,
			};

			let (in_handle, out_handle) = if already_sharp {
				let is_closed = manipulator_groups.last().filter(|group| group.is_close()).is_some();

				// Grab the next and previous manipulator groups by simply looking at the next / previous index
				let mut previous_position = index.checked_sub(1).and_then(|index| manipulator_groups.by_index(index)).and_then(|group| group.points[0].as_ref());
				let mut next_position = manipulator_groups.by_index(index + 1).and_then(|group| group.points[0].as_ref());

				// Wrapping around closed path (assuming format is point elements then a single close path)
				if is_closed {
					previous_position = previous_position.or_else(|| manipulator_groups.iter().nth_back(1).and_then(|group| group.points[0].as_ref()));
					next_position = next_position.or_else(|| manipulator_groups.first().and_then(|group| group.points[0].as_ref()));
				}

				// To find the length of the new tangent we just take the distance to the anchor and divide by 3 (pretty arbitrary)
				let length_previous = previous_position.map(|point| (point.position - anchor_position).length() / 3.);
				let length_next = next_position.map(|point| (point.position - anchor_position).length() / 3.);

				// Use the position relative to the anchor
				let previous_angle = previous_position.map(|point| (point.position - anchor_position)).map(|pos| pos.y.atan2(pos.x));
				let next_angle = next_position.map(|point| (point.position - anchor_position)).map(|pos| pos.y.atan2(pos.x));

				// The direction of the handles is either the perpendicular vector to the sum of the anchors' positions or just the anchor's position (if only one)
				let handle_direction = match (previous_angle, next_angle) {
					(Some(previous), Some(next)) => (previous + next) / 2. + core::f64::consts::FRAC_PI_2,
					(None, Some(val)) => core::f64::consts::PI + val,
					(Some(val), None) => val,
					(None, None) => return None,
				};

				// Mirror the angle but not the distance
				responses.push_back(
					Operation::SetManipulatorHandleMirroring {
						layer_path: layer_path.to_vec(),
						id: bezier_id,
						mirror_angle: true,
					}
					.into(),
				);

				let (sin, cos) = handle_direction.sin_cos();
				let mut handle_vector = DVec2::new(cos, sin);

				// Flip the vector if it is not facing towards the same direction as the anchor
				if previous_position.filter(|pos| (pos.position - anchor_position).normalize().dot(handle_vector) < 0.).is_some()
					|| next_position.filter(|pos| (pos.position - anchor_position).normalize().dot(handle_vector) > 0.).is_some()
				{
					handle_vector = -handle_vector;
				}

				(
					length_previous.map(|length| anchor_position + handle_vector * length),
					length_next.map(|length| anchor_position - handle_vector * length),
				)
			} else {
				(Some(anchor_position), Some(anchor_position))
			};

			// Push both in and out handles into the correct position
			if let Some(in_handle) = in_handle {
				let in_handle = Operation::SetManipulatorPoints {
					layer_path: layer_path.to_vec(),
					id: bezier_id,
					manipulator_type: ManipulatorType::InHandle,
					position: Some(in_handle.into()),
				};
				responses.push_back(in_handle.into());
			}
			if let Some(out_handle) = out_handle {
				let out_handle = Operation::SetManipulatorPoints {
					layer_path: layer_path.to_vec(),
					id: bezier_id,
					manipulator_type: ManipulatorType::OutHandle,
					position: Some(out_handle.into()),
				};
				responses.push_back(out_handle.into());
			}
			Some(true)
		};
		for layer_path in &self.selected_layers {
			if let Some(result) = process_layer(layer_path) {
				return result;
			}
		}
		false
	}

	fn shape<'a>(&'a self, document: &'a Document, layer_id: &[u64]) -> Option<&'a Subpath> {
		document.layer(layer_id).ok()?.as_subpath()
	}
}
