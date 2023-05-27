use crate::consts::DRAG_THRESHOLD;
use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::prelude::*;

use bezier_rs::{Bezier, TValue};
use document_legacy::LayerId;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::{ManipulatorPointId, SelectedType, VectorData};

use document_legacy::document::Document;
use glam::DVec2;

#[derive(Clone, Debug, Default)]
pub struct SelectedLayerState {
	selected_points: HashSet<ManipulatorPointId>,
}
impl SelectedLayerState {
	pub fn is_selected(&self, point: ManipulatorPointId) -> bool {
		self.selected_points.contains(&point)
	}
	pub fn select_point(&mut self, point: ManipulatorPointId) {
		self.selected_points.insert(point);
	}
	pub fn deselect_point(&mut self, point: ManipulatorPointId) {
		self.selected_points.remove(&point);
	}
}
pub type SelectedShapeState = HashMap<Vec<LayerId>, SelectedLayerState>;
#[derive(Debug, Default)]
pub struct ShapeState {
	// The layers we can select and edit manipulators (anchors and handles) from
	pub selected_shape_state: SelectedShapeState,
}

pub struct SelectedPointsInfo<'a> {
	pub points: Vec<ManipulatorPointInfo<'a>>,
	pub offset: DVec2,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ManipulatorPointInfo<'a> {
	pub shape_layer_path: &'a [LayerId],
	pub point_id: ManipulatorPointId,
}

pub type OpposingHandleLengths = HashMap<Vec<LayerId>, HashMap<ManipulatorGroupId, Option<f64>>>;

// TODO Consider keeping a list of selected manipulators to minimize traversals of the layers
impl ShapeState {
	/// Select the first point within the selection threshold.
	/// Returns a tuple of the points if found and the offset, or `None` otherwise.
	pub fn select_point(&mut self, document: &Document, mouse_position: DVec2, select_threshold: f64, add_to_selection: bool) -> Option<SelectedPointsInfo> {
		if self.selected_shape_state.is_empty() {
			return None;
		}

		if let Some((shape_layer_path, manipulator_point_id)) = self.find_nearest_point_indices(document, mouse_position, select_threshold) {
			trace!("Selecting... manipulator point: {:?}", manipulator_point_id);

			let vector_data = document.layer(&shape_layer_path).ok()?.as_vector_data()?;
			let manipulator_group = vector_data.manipulator_groups().find(|group| group.id == manipulator_point_id.group)?;
			let point_position = manipulator_point_id.manipulator_type.get_position(manipulator_group)?;

			let selected_shape_state = self.selected_shape_state.get(&shape_layer_path)?;
			let already_selected = selected_shape_state.is_selected(manipulator_point_id);

			// Should we select or deselect the point?
			let new_selected = if already_selected { !add_to_selection } else { true };

			// This is selecting the manipulator only for now, next to generalize to points
			if new_selected {
				let retain_existing_selection = add_to_selection || already_selected;
				if !retain_existing_selection {
					self.deselect_all();
				}

				// Add to the selected points
				let selected_shape_state = self.selected_shape_state.get_mut(&shape_layer_path)?;
				selected_shape_state.select_point(manipulator_point_id);

				// Offset to snap the selected point to the cursor
				let offset = document
					.generate_transform_relative_to_viewport(&shape_layer_path)
					.map(|viewspace| mouse_position - viewspace.transform_point2(point_position))
					.unwrap_or_default();

				let points = self
					.selected_shape_state
					.iter()
					.flat_map(|(shape_layer_path, state)| state.selected_points.iter().map(|&point_id| ManipulatorPointInfo { shape_layer_path, point_id }))
					.collect();

				return Some(SelectedPointsInfo { points, offset });
			} else {
				let selected_shape_state = self.selected_shape_state.get_mut(&shape_layer_path)?;
				selected_shape_state.deselect_point(manipulator_point_id);

				return None;
			}
		}

		// Deselect all points if no nearby point
		self.deselect_all();

		None
	}

	pub fn deselect_all(&mut self) {
		self.selected_shape_state.values_mut().for_each(|state| state.selected_points.clear());
	}

	/// Set the shapes we consider for selection, we will choose draggable manipulators from these shapes.
	pub fn set_selected_layers(&mut self, target_layers: Vec<Vec<LayerId>>) {
		self.selected_shape_state.retain(|layer_path, _| target_layers.contains(layer_path));
		for layer in target_layers {
			self.selected_shape_state.entry(layer).or_insert_with(SelectedLayerState::default);
		}
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = &Vec<LayerId>> {
		self.selected_shape_state.keys()
	}

	/// Clear all of the shapes we can modify.
	pub fn clear_selected_layers(&mut self) {
		self.selected_shape_state.clear();
	}

	pub fn has_selected_layers(&self) -> bool {
		!self.selected_shape_state.is_empty()
	}

	/// A mutable iterator of all the manipulators, regardless of selection.
	pub fn manipulator_groups<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a bezier_rs::ManipulatorGroup<ManipulatorGroupId>> {
		self.iter(document).flat_map(|shape| shape.manipulator_groups())
	}

	// Sets the selected points to all points for the corresponding intersection
	pub fn select_all_anchors(&mut self, document: &Document, layer_path: &[LayerId]) {
		let Ok(layer) = document.layer(layer_path) else { return };
		let Some(vector_data) = layer.as_vector_data() else { return };
		let Some(state) = self.selected_shape_state.get_mut(layer_path) else { return };
		for manipulator in vector_data.manipulator_groups() {
			state.select_point(ManipulatorPointId::new(manipulator.id, SelectedType::Anchor))
		}
	}

	/// Provide the currently selected points by reference.
	pub fn selected_points(&self) -> impl Iterator<Item = &'_ ManipulatorPointId> {
		self.selected_shape_state.values().flat_map(|state| &state.selected_points)
	}

	/// Move the selected points by dragging the mouse.
	pub fn move_selected_points(&self, document: &Document, delta: DVec2, mirror_distance: bool, responses: &mut VecDeque<Message>) {
		for (layer_path, state) in &self.selected_shape_state {
			let Ok(layer) = document.layer(layer_path) else { continue };
			let Some(vector_data) = layer.as_vector_data() else { continue };

			let transform = document.multiply_transforms(layer_path).unwrap_or_default();
			let delta = transform.inverse().transform_vector2(delta);

			for &point in state.selected_points.iter() {
				if point.manipulator_type.is_handle() && state.is_selected(ManipulatorPointId::new(point.group, SelectedType::Anchor)) {
					continue;
				}

				let Some(group) = vector_data.manipulator_from_id(point.group) else { continue };

				let mut move_point = |point: ManipulatorPointId| {
					let Some(previous_position) = point.manipulator_type.get_position(group) else { return };
					let position = previous_position + delta;
					responses.add(GraphOperationMessage::Vector {
						layer: layer_path.clone(),
						modification: VectorDataModification::SetManipulatorPosition { point, position },
					});
				};

				move_point(point);

				if point.manipulator_type == SelectedType::Anchor {
					move_point(ManipulatorPointId::new(point.group, SelectedType::InHandle));
					move_point(ManipulatorPointId::new(point.group, SelectedType::OutHandle));
				}

				if mirror_distance && point.manipulator_type != SelectedType::Anchor {
					let mut mirror = vector_data.mirror_angle.contains(&point.group);

					// If there is no opposing handle, we mirror even if mirror_angle doesn't contain the group
					// and set angle mirroring to true.
					if !mirror && point.manipulator_type.opposite().get_position(group).is_none() {
						responses.add(GraphOperationMessage::Vector {
							layer: layer_path.clone(),
							modification: VectorDataModification::SetManipulatorHandleMirroring { id: group.id, mirror_angle: true },
						});
						mirror = true;
					}

					if mirror {
						let Some(mut original_handle_position) = point.manipulator_type.get_position(group) else { continue };
						original_handle_position += delta;

						let point = ManipulatorPointId::new(point.group, point.manipulator_type.opposite());
						if state.is_selected(point) {
							continue;
						}
						let position = group.anchor - (original_handle_position - group.anchor);
						responses.add(GraphOperationMessage::Vector {
							layer: layer_path.clone(),
							modification: VectorDataModification::SetManipulatorPosition { point, position },
						});
					}
				}
			}
		}
	}

	/// Delete selected and mirrored handles with zero length when the drag stops.
	pub fn delete_selected_handles_with_zero_length(&self, document: &Document, opposing_handle_lengths: &Option<OpposingHandleLengths>, responses: &mut VecDeque<Message>) {
		for (layer_path, state) in &self.selected_shape_state {
			let Ok(layer) = document.layer(layer_path) else { continue };
			let Some(vector_data) = layer.as_vector_data() else { continue };

			let opposing_handle_lengths = opposing_handle_lengths.as_ref().and_then(|lengths| lengths.get(layer_path));

			let transform = document.multiply_transforms(layer_path).unwrap_or(glam::DAffine2::IDENTITY);

			for &point in state.selected_points.iter() {
				let anchor = ManipulatorPointId::new(point.group, SelectedType::Anchor);
				if !point.manipulator_type.is_handle() || state.is_selected(anchor) {
					continue;
				}

				let Some(group) = vector_data.manipulator_from_id(point.group) else { continue };

				let anchor_position = transform.transform_point2(group.anchor);

				let point_position = if let Some(position) = point.manipulator_type.get_position(group) {
					transform.transform_point2(position)
				} else {
					continue;
				};

				if (anchor_position - point_position).length() < DRAG_THRESHOLD {
					responses.add(GraphOperationMessage::Vector {
						layer: layer_path.clone(),
						modification: VectorDataModification::RemoveManipulatorPoint { point },
					});

					// Remove opposing handle if it is not selected and is mirrored.
					let opposite_point = ManipulatorPointId::new(point.group, point.manipulator_type.opposite());
					if !state.is_selected(opposite_point) && vector_data.mirror_angle.contains(&point.group) {
						if let Some(lengths) = opposing_handle_lengths {
							if lengths.contains_key(&point.group) {
								responses.add(GraphOperationMessage::Vector {
									layer: layer_path.clone(),
									modification: VectorDataModification::RemoveManipulatorPoint { point: opposite_point },
								});
							}
						}
					}
				}
			}
		}
	}

	/// The opposing handle lengths.
	pub fn opposing_handle_lengths(&self, document: &Document) -> OpposingHandleLengths {
		self.selected_shape_state
			.iter()
			.filter_map(|(path, state)| {
				let layer = document.layer(path).ok()?;
				let vector_data = layer.as_vector_data()?;
				let opposing_handle_lengths = vector_data
					.subpaths
					.iter()
					.flat_map(|subpath| {
						subpath.manipulator_groups().iter().filter_map(|manipulator_group| {
							// We will keep track of the opposing handle length when:
							// i) Exactly one handle is selected.
							// ii) The anchor is not selected.

							let in_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::InHandle));
							let out_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::OutHandle));
							let anchor_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::Anchor));

							if anchor_selected {
								return None;
							}

							let single_selected_handle = match (in_handle_selected, out_handle_selected) {
								(true, false) => SelectedType::InHandle,
								(false, true) => SelectedType::OutHandle,
								_ => return None,
							};

							let Some(opposing_handle_position) = single_selected_handle.opposite().get_position(manipulator_group) else {
								return Some((manipulator_group.id, None));
							};

							let opposing_handle_length = opposing_handle_position.distance(manipulator_group.anchor);
							Some((manipulator_group.id, Some(opposing_handle_length)))
						})
					})
					.collect::<HashMap<_, _>>();
				Some((path.clone(), opposing_handle_lengths))
			})
			.collect::<HashMap<_, _>>()
	}

	/// Reset the opposing handle lengths.
	pub fn reset_opposing_handle_lengths(&self, document: &Document, opposing_handle_lengths: &OpposingHandleLengths, responses: &mut VecDeque<Message>) {
		for (path, state) in &self.selected_shape_state {
			let Ok(layer) = document.layer(path) else { continue };
			let Some(vector_data) = layer.as_vector_data() else { continue };
			let Some(opposing_handle_lengths) = opposing_handle_lengths.get(path) else { continue };

			for subpath in &vector_data.subpaths {
				for manipulator_group in subpath.manipulator_groups() {
					if !vector_data.mirror_angle.contains(&manipulator_group.id) {
						continue;
					}

					let Some(opposing_handle_length) = opposing_handle_lengths.get(&manipulator_group.id) else { continue };

					let in_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::InHandle));
					let out_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::OutHandle));
					let anchor_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::Anchor));

					if anchor_selected {
						continue;
					}

					let single_selected_handle = match (in_handle_selected, out_handle_selected) {
						(true, false) => SelectedType::InHandle,
						(false, true) => SelectedType::OutHandle,
						_ => continue,
					};

					let Some(opposing_handle_length) = opposing_handle_length else {
						responses.add(GraphOperationMessage::Vector {
							layer: path.to_vec(),
							modification: VectorDataModification::RemoveManipulatorPoint {
								point: ManipulatorPointId::new(manipulator_group.id, single_selected_handle.opposite()),
							},
						});
						continue;
					};

					let Some(opposing_handle) = single_selected_handle.opposite().get_position(manipulator_group) else { continue };

					let Some(offset) = (opposing_handle - manipulator_group.anchor).try_normalize() else { continue };

					let point = ManipulatorPointId::new(manipulator_group.id, single_selected_handle.opposite());
					let position = manipulator_group.anchor + offset * (*opposing_handle_length);
					assert!(position.is_finite(), "Opposing handle not finite!");

					responses.add(GraphOperationMessage::Vector {
						layer: path.to_vec(),
						modification: VectorDataModification::SetManipulatorPosition { point, position },
					});
				}
			}
		}
	}

	/// Dissolve the selected points.
	pub fn delete_selected_points(&self, responses: &mut VecDeque<Message>) {
		for (layer, state) in &self.selected_shape_state {
			for &point in &state.selected_points {
				responses.add(GraphOperationMessage::Vector {
					layer: layer.to_vec(),
					modification: VectorDataModification::RemoveManipulatorPoint { point },
				})
			}
		}
	}

	/// Toggle if the handles should mirror angle across the anchor position.
	pub fn toggle_handle_mirroring_on_selected(&self, responses: &mut VecDeque<Message>) {
		for (layer, state) in &self.selected_shape_state {
			for point in &state.selected_points {
				responses.add(GraphOperationMessage::Vector {
					layer: layer.to_vec(),
					modification: VectorDataModification::ToggleManipulatorHandleMirroring { id: point.group },
				})
			}
		}
	}

	/// Iterate over the shapes.
	pub fn iter<'a>(&'a self, document: &'a Document) -> impl Iterator<Item = &'a VectorData> + 'a {
		self.selected_shape_state
			.keys()
			.flat_map(|layer_id| document.layer(layer_id))
			.filter_map(|shape| shape.as_vector_data())
	}

	/// Find a [ManipulatorPoint] that is within the selection threshold and return the layer path, an index to the [ManipulatorGroup], and an enum index for [ManipulatorPoint].
	pub fn find_nearest_point_indices(&mut self, document: &Document, mouse_position: DVec2, select_threshold: f64) -> Option<(Vec<LayerId>, ManipulatorPointId)> {
		if self.selected_shape_state.is_empty() {
			return None;
		}

		let select_threshold_squared = select_threshold * select_threshold;
		// Find the closest control point among all elements of shapes_to_modify
		for layer in self.selected_shape_state.keys() {
			if let Some((manipulator_point_id, distance_squared)) = Self::closest_point_in_layer(document, layer, mouse_position) {
				// Choose the first point under the threshold
				if distance_squared < select_threshold_squared {
					trace!("Selecting... manipulator point: {:?}", manipulator_point_id);
					return Some((layer.clone(), manipulator_point_id));
				}
			}
		}

		None
	}

	// TODO Use quadtree or some equivalent spatial acceleration structure to improve this to O(log(n))
	/// Find the closest manipulator, manipulator point, and distance so we can select path elements.
	/// Brute force comparison to determine which manipulator (handle or anchor) we want to select taking O(n) time.
	/// Return value is an `Option` of the tuple representing `(ManipulatorPointId, distance squared)`.
	fn closest_point_in_layer(document: &Document, layer_path: &[LayerId], pos: glam::DVec2) -> Option<(ManipulatorPointId, f64)> {
		let mut closest_distance_squared: f64 = f64::MAX;
		let mut result = None;

		let vector_data = document.layer(layer_path).ok()?.as_vector_data()?;
		let viewspace = document.generate_transform_relative_to_viewport(layer_path).ok()?;
		for subpath in &vector_data.subpaths {
			for manipulator in subpath.manipulator_groups() {
				let (selected, distance_squared) = SelectedType::closest_widget(manipulator, viewspace, pos, crate::consts::HIDE_HANDLE_DISTANCE);

				if distance_squared < closest_distance_squared {
					closest_distance_squared = distance_squared;
					result = Some((ManipulatorPointId::new(manipulator.id, selected), distance_squared));
				}
			}
		}

		result
	}

	/// Find the `t` value along the path segment we have clicked upon, together with that segment ID.
	fn closest_segment(&self, document: &Document, layer_path: &[LayerId], position: glam::DVec2, tolerance: f64) -> Option<(ManipulatorGroupId, ManipulatorGroupId, Bezier, f64)> {
		let transform = document.generate_transform_relative_to_viewport(layer_path).ok()?;
		let layer_pos = transform.inverse().transform_point2(position);
		let projection_options = bezier_rs::ProjectionOptions { lut_size: 5, ..Default::default() };

		let mut result = None;
		let mut closest_distance_squared: f64 = tolerance * tolerance;

		let vector_data = document.layer(layer_path).ok()?.as_vector_data()?;

		for subpath in &vector_data.subpaths {
			for (manipulator_index, bezier) in subpath.iter().enumerate() {
				let t = bezier.project(layer_pos, Some(projection_options));
				let layerspace = bezier.evaluate(TValue::Parametric(t));

				let screenspace = transform.transform_point2(layerspace);
				let distance_squared = screenspace.distance_squared(position);

				if distance_squared < closest_distance_squared {
					closest_distance_squared = distance_squared;
					let start = subpath.manipulator_groups()[manipulator_index];
					let end = subpath.manipulator_groups()[(manipulator_index + 1) % subpath.len()];
					result = Some((start.id, end.id, bezier, t));
				}
			}
		}

		result
	}

	/// Handles the splitting of a curve to insert new points (which can be activated by double clicking on a curve with the Path tool).
	pub fn split(&self, document: &Document, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) {
		for layer_path in self.selected_layers() {
			if let Some((start, end, bezier, t)) = self.closest_segment(document, layer_path, position, tolerance) {
				let [first, second] = bezier.split(TValue::Parametric(t));

				// Adjust the first manipulator group's out handle
				let point = ManipulatorPointId::new(start, SelectedType::OutHandle);
				let position = first.handle_start().unwrap_or(first.start());
				let out_handle = GraphOperationMessage::Vector {
					layer: layer_path.clone(),
					modification: VectorDataModification::SetManipulatorPosition { point, position },
				};
				responses.add(out_handle);

				// Insert a new manipulator group between the existing ones
				let manipulator_group = bezier_rs::ManipulatorGroup::new(first.end(), first.handle_end(), second.handle_start());
				let insert = GraphOperationMessage::Vector {
					layer: layer_path.clone(),
					modification: VectorDataModification::AddManipulatorGroup { manipulator_group, after_id: start },
				};
				responses.add(insert);

				// Adjust the last manipulator group's in handle
				let point = ManipulatorPointId::new(end, SelectedType::InHandle);
				let position = second.handle_end().unwrap_or(second.end());
				let in_handle = GraphOperationMessage::Vector {
					layer: layer_path.clone(),
					modification: VectorDataModification::SetManipulatorPosition { point, position },
				};
				responses.add(in_handle);

				return;
			}
		}
	}

	/// Handles the flipping between sharp corner and smooth (which can be activated by double clicking on an anchor with the Path tool).
	pub fn flip_sharp(&self, document: &Document, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) -> bool {
		let mut process_layer = |layer_path| {
			let vector_data = document.layer(layer_path).ok()?.as_vector_data()?;

			let transform_to_screenspace = document.generate_transform_relative_to_viewport(layer_path).ok()?;
			let mut result = None;
			let mut closest_distance_squared = tolerance * tolerance;

			// Find the closest anchor point on the current layer
			for (subpath_index, subpath) in vector_data.subpaths.iter().enumerate() {
				for (manipulator_index, manipulator) in subpath.manipulator_groups().iter().enumerate() {
					let screenspace = transform_to_screenspace.transform_point2(manipulator.anchor);
					let distance_squared = screenspace.distance_squared(position);

					if distance_squared < closest_distance_squared {
						closest_distance_squared = distance_squared;
						result = Some((subpath_index, manipulator_index, manipulator));
					}
				}
			}
			let (subpath_index, index, manipulator) = result?;
			let anchor_position = manipulator.anchor;

			let subpath = &vector_data.subpaths[subpath_index];

			// Check by comparing the handle positions to the anchor if this maniuplator group is a point
			let already_sharp = match (manipulator.in_handle, manipulator.out_handle) {
				(Some(in_handle), Some(out_handle)) => anchor_position.abs_diff_eq(in_handle, 1e-10) && anchor_position.abs_diff_eq(out_handle, 1e-10),
				(Some(handle), None) | (None, Some(handle)) => anchor_position.abs_diff_eq(handle, 1e-10),
				(None, None) => true,
			};

			let manipulator_groups = subpath.manipulator_groups();
			let (in_handle, out_handle) = if already_sharp {
				let is_closed = subpath.closed();

				// Grab the next and previous manipulator groups by simply looking at the next / previous index
				let mut previous_position = index.checked_sub(1).and_then(|index| manipulator_groups.get(index)).map(|group| group.anchor);
				let mut next_position = manipulator_groups.get(index + 1).map(|group| group.anchor);

				// Wrapping around closed path
				if is_closed {
					previous_position = previous_position.or_else(|| manipulator_groups.last().map(|group| group.anchor));
					next_position = next_position.or_else(|| manipulator_groups.first().map(|group| group.anchor));
				}

				// To find the length of the new tangent we just take the distance to the anchor and divide by 3 (pretty arbitrary)
				let length_previous = previous_position.map(|point| (point - anchor_position).length() / 3.);
				let length_next = next_position.map(|point| (point - anchor_position).length() / 3.);

				// Use the position relative to the anchor
				let previous_angle = previous_position.map(|point| (point - anchor_position)).map(|pos| pos.y.atan2(pos.x));
				let next_angle = next_position.map(|point| (point - anchor_position)).map(|pos| pos.y.atan2(pos.x));

				// The direction of the handles is either the perpendicular vector to the sum of the anchors' positions or just the anchor's position (if only one)
				let handle_direction = match (previous_angle, next_angle) {
					(Some(previous), Some(next)) => (previous + next) / 2. + core::f64::consts::FRAC_PI_2,
					(None, Some(val)) => core::f64::consts::PI + val,
					(Some(val), None) => val,
					(None, None) => return None,
				};

				// Mirror the angle but not the distance
				responses.add(GraphOperationMessage::Vector {
					layer: layer_path.to_vec(),
					modification: VectorDataModification::SetManipulatorHandleMirroring {
						id: manipulator.id,
						mirror_angle: true,
					},
				});

				let (sin, cos) = handle_direction.sin_cos();
				let mut handle_vector = DVec2::new(cos, sin);

				// Flip the vector if it is not facing towards the same direction as the anchor
				if previous_position.filter(|&pos| (pos - anchor_position).normalize().dot(handle_vector) < 0.).is_some()
					|| next_position.filter(|&pos| (pos - anchor_position).normalize().dot(handle_vector) > 0.).is_some()
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
				let point = ManipulatorPointId::new(manipulator.id, SelectedType::InHandle);
				responses.add(GraphOperationMessage::Vector {
					layer: layer_path.to_vec(),
					modification: VectorDataModification::SetManipulatorPosition { point, position: in_handle },
				});
			}
			if let Some(out_handle) = out_handle {
				let point = ManipulatorPointId::new(manipulator.id, SelectedType::OutHandle);
				responses.add(GraphOperationMessage::Vector {
					layer: layer_path.to_vec(),
					modification: VectorDataModification::SetManipulatorPosition { point, position: out_handle },
				});
			}
			Some(true)
		};
		for layer_path in self.selected_shape_state.keys() {
			if let Some(result) = process_layer(layer_path) {
				return result;
			}
		}
		false
	}
}
