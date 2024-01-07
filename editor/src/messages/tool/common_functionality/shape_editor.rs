use super::graph_modification_utils;
use super::snapping::{group_smooth, SnapCandidatePoint, SnapData, SnapManager, SnappedPoint};
use crate::consts::DRAG_THRESHOLD;
use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{NodeSnapSource, SnapSource};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{get_manipulator_from_id, get_manipulator_groups, get_mirror_handles, get_subpaths};

use bezier_rs::{Bezier, ManipulatorGroup, TValue};
use graph_craft::document::NodeNetwork;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use glam::DVec2;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ManipulatorAngle {
	Smooth,
	Sharp,
	Mixed,
}

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
	pub fn clear_points(&mut self) {
		self.selected_points.clear();
	}
	pub fn selected_points_count(&self) -> usize {
		self.selected_points.len()
	}
}

pub type SelectedShapeState = HashMap<LayerNodeIdentifier, SelectedLayerState>;
#[derive(Debug, Default)]
pub struct ShapeState {
	// The layers we can select and edit manipulators (anchors and handles) from
	pub selected_shape_state: SelectedShapeState,
}

pub struct SelectedPointsInfo {
	pub points: Vec<ManipulatorPointInfo>,
	pub offset: DVec2,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ManipulatorPointInfo {
	pub layer: LayerNodeIdentifier,
	pub point_id: ManipulatorPointId,
}

pub type OpposingHandleLengths = HashMap<LayerNodeIdentifier, HashMap<ManipulatorGroupId, Option<f64>>>;

// TODO Consider keeping a list of selected manipulators to minimize traversals of the layers
impl ShapeState {
	// Snap, returning a viewport delta
	pub fn snap(&self, snap_manager: &mut SnapManager, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, previous_mouse: DVec2) -> DVec2 {
		let mut snap_data = SnapData::new(document, input);

		for (layer, state) in &self.selected_shape_state {
			for point in &state.selected_points {
				snap_data.manipulators.push((*layer, point.group));
			}
		}

		let mouse_delta = document.metadata.document_to_viewport.inverse().transform_vector2(input.mouse.position - previous_mouse);
		let mut offset = mouse_delta;
		let mut best_snapped = SnappedPoint::infinite_snap(document.metadata.document_to_viewport.inverse().transform_point2(input.mouse.position));
		for (layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(*layer, &document.network) else { continue };

			let to_document = document.metadata.transform_to_document(*layer);

			for subpath in document.metadata.layer_outline(*layer) {
				for (index, group) in subpath.manipulator_groups().iter().enumerate() {
					for handle in [SelectedType::Anchor, SelectedType::InHandle, SelectedType::OutHandle] {
						if !state.is_selected(ManipulatorPointId::new(group.id, handle)) {
							continue;
						}
						let source = if handle.is_handle() {
							SnapSource::Node(NodeSnapSource::Handle)
						} else if group_smooth(group, to_document, subpath, index) {
							SnapSource::Node(NodeSnapSource::Smooth)
						} else {
							SnapSource::Node(NodeSnapSource::Sharp)
						};
						let Some(position) = handle.get_position(&group) else { continue };
						let point = SnapCandidatePoint::new_source(to_document.transform_point2(position) + mouse_delta, source);
						let snapped = snap_manager.free_snap(&snap_data, &point, None, false);
						if best_snapped.other_snap_better(&snapped) {
							offset = snapped.snapped_point_document - point.document_point + mouse_delta;
							best_snapped = snapped;
						}
					}
				}
			}
		}
		snap_manager.update_indicator(best_snapped);
		document.metadata.document_to_viewport.transform_vector2(offset)
	}

	/// Select the first point within the selection threshold.
	/// Returns a tuple of the points if found and the offset, or `None` otherwise.
	pub fn select_point(
		&mut self,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		mouse_position: DVec2,
		select_threshold: f64,
		add_to_selection: bool,
	) -> Option<SelectedPointsInfo> {
		if self.selected_shape_state.is_empty() {
			return None;
		}

		if let Some((layer, manipulator_point_id)) = self.find_nearest_point_indices(document_network, document_metadata, mouse_position, select_threshold) {
			trace!("Selecting... manipulator point: {manipulator_point_id:?}");

			let subpaths = get_subpaths(layer, document_network)?;
			let manipulator_group = get_manipulator_groups(subpaths).find(|group| group.id == manipulator_point_id.group)?;
			let point_position = manipulator_point_id.manipulator_type.get_position(manipulator_group)?;

			let selected_shape_state = self.selected_shape_state.get(&layer)?;
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
				let selected_shape_state = self.selected_shape_state.get_mut(&layer)?;
				selected_shape_state.select_point(manipulator_point_id);

				// Offset to snap the selected point to the cursor
				let offset = mouse_position - document_metadata.transform_to_viewport(layer).transform_point2(point_position);

				let points = self
					.selected_shape_state
					.iter()
					.flat_map(|(layer, state)| state.selected_points.iter().map(|&point_id| ManipulatorPointInfo { layer: *layer, point_id }))
					.collect();

				return Some(SelectedPointsInfo { points, offset });
			} else {
				let selected_shape_state = self.selected_shape_state.get_mut(&layer)?;
				selected_shape_state.deselect_point(manipulator_point_id);

				return None;
			}
		}
		None
	}

	pub fn select_all_points(&mut self, document_network: &NodeNetwork) {
		for (layer, state) in self.selected_shape_state.iter_mut() {
			let Some(subpaths) = get_subpaths(*layer, document_network) else { return };
			for manipulator in get_manipulator_groups(subpaths) {
				state.select_point(ManipulatorPointId::new(manipulator.id, SelectedType::Anchor));
				for selected_type in &[SelectedType::InHandle, SelectedType::OutHandle] {
					state.deselect_point(ManipulatorPointId::new(manipulator.id, *selected_type));
				}
			}
		}
	}

	pub fn deselect_all(&mut self) {
		self.selected_shape_state.values_mut().for_each(|state| state.selected_points.clear());
	}

	/// Set the shapes we consider for selection, we will choose draggable manipulators from these shapes.
	pub fn set_selected_layers(&mut self, target_layers: Vec<LayerNodeIdentifier>) {
		self.selected_shape_state.retain(|layer_path, _| target_layers.contains(layer_path));
		for layer in target_layers {
			self.selected_shape_state.entry(layer).or_default();
		}
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = &LayerNodeIdentifier> {
		self.selected_shape_state.keys()
	}

	pub fn has_selected_layers(&self) -> bool {
		!self.selected_shape_state.is_empty()
	}

	/// A mutable iterator of all the manipulators, regardless of selection.
	pub fn manipulator_groups<'a>(&'a self, document_network: &'a NodeNetwork) -> impl Iterator<Item = &'a ManipulatorGroup<ManipulatorGroupId>> {
		self.iter(document_network).flat_map(|subpaths| get_manipulator_groups(subpaths))
	}

	// Sets the selected points to all points for the corresponding intersection
	pub fn select_all_anchors(&mut self, document_network: &NodeNetwork, layer: LayerNodeIdentifier) {
		let Some(subpaths) = get_subpaths(layer, document_network) else { return };
		let Some(state) = self.selected_shape_state.get_mut(&layer) else { return };
		for manipulator in get_manipulator_groups(subpaths) {
			state.select_point(ManipulatorPointId::new(manipulator.id, SelectedType::Anchor))
		}
	}

	/// Provide the currently selected points by reference.
	pub fn selected_points(&self) -> impl Iterator<Item = &'_ ManipulatorPointId> {
		self.selected_shape_state.values().flat_map(|state| &state.selected_points)
	}

	/// Moves a control point to a `new_position` in document space.
	/// Returns `Some(())` if successful and `None` otherwise.
	pub fn reposition_control_point(
		&self,
		point: &ManipulatorPointId,
		responses: &mut VecDeque<Message>,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		new_position: DVec2,
		layer: LayerNodeIdentifier,
	) -> Option<()> {
		let subpaths = get_subpaths(layer, document_network)?;
		let transform = document_metadata.transform_to_viewport(layer).inverse();
		let position = transform.transform_point2(new_position);
		let group = graph_modification_utils::get_manipulator_from_id(subpaths, point.group)?;
		let delta = position - point.manipulator_type.get_position(group)?;

		if point.manipulator_type.is_handle() {
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorHandleMirroring { id: group.id, mirror_angle: false },
			});
		}

		let mut move_point = |point: ManipulatorPointId| {
			let Some(position) = point.manipulator_type.get_position(group) else {
				return;
			};
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorPosition { point, position: (position + delta) },
			});
		};

		move_point(*point);
		if !point.manipulator_type.is_handle() {
			move_point(ManipulatorPointId::new(point.group, SelectedType::InHandle));
			move_point(ManipulatorPointId::new(point.group, SelectedType::OutHandle));
		}

		Some(())
	}

	// Iterates over the selected manipulator groups, returning whether they have mixed, sharp, or smooth angles.
	// If there are no points selected this function returns mixed.
	pub fn selected_manipulator_angles(&self, document_network: &NodeNetwork) -> ManipulatorAngle {
		// This iterator contains a bool indicating whether or not every selected point has a smooth manipulator angle.
		let mut point_smoothness_status = self
			.selected_shape_state
			.iter()
			.filter_map(|(&layer, selection_state)| Some((graph_modification_utils::get_mirror_handles(layer, document_network)?, selection_state)))
			.flat_map(|(mirror, selection_state)| selection_state.selected_points.iter().map(|selected_point| mirror.contains(&selected_point.group)));

		let Some(first_is_smooth) = point_smoothness_status.next() else { return ManipulatorAngle::Mixed };

		if point_smoothness_status.any(|point| first_is_smooth != point) {
			return ManipulatorAngle::Mixed;
		}
		match first_is_smooth {
			false => ManipulatorAngle::Sharp,
			true => ManipulatorAngle::Smooth,
		}
	}

	pub fn smooth_manipulator_group(&self, subpath: &bezier_rs::Subpath<ManipulatorGroupId>, index: usize, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier) {
		let manipulator_groups = subpath.manipulator_groups();
		let manipulator = manipulator_groups[index];

		// Grab the next and previous manipulator groups by simply looking at the next / previous index
		let mut previous_position = index.checked_sub(1).and_then(|index| manipulator_groups.get(index));
		let mut next_position = manipulator_groups.get(index + 1);

		// Wrapping around closed path
		if subpath.closed() {
			previous_position = previous_position.or_else(|| manipulator_groups.last());
			next_position = next_position.or_else(|| manipulator_groups.first());
		}

		let anchor_position = manipulator.anchor;
		// To find the length of the new tangent we just take the distance to the anchor and divide by 3 (pretty arbitrary)
		let length_previous = previous_position.map(|group| (group.anchor - anchor_position).length() / 3.);
		let length_next = next_position.map(|group| (group.anchor - anchor_position).length() / 3.);

		// Use the position relative to the anchor
		let previous_angle = previous_position.map(|group| (group.anchor - anchor_position)).map(|pos| pos.y.atan2(pos.x));
		let next_angle = next_position.map(|group| (group.anchor - anchor_position)).map(|pos| pos.y.atan2(pos.x));

		// The direction of the handles is either the perpendicular vector to the sum of the anchors' positions or just the anchor's position (if only one)
		let handle_direction = match (previous_angle, next_angle) {
			(Some(previous), Some(next)) => (previous + next) / 2. + core::f64::consts::FRAC_PI_2,
			(None, Some(val)) => core::f64::consts::PI + val,
			(Some(val), None) => val,
			(None, None) => return,
		};

		// Mirror the angle but not the distance
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification: VectorDataModification::SetManipulatorHandleMirroring {
				id: manipulator.id,
				mirror_angle: true,
			},
		});

		let (sin, cos) = handle_direction.sin_cos();
		let mut handle_vector = DVec2::new(cos, sin);

		// Flip the vector if it is not facing towards the same direction as the anchor
		if previous_position.filter(|&group| (group.anchor - anchor_position).normalize().dot(handle_vector) < 0.).is_some()
			|| next_position.filter(|&group| (group.anchor - anchor_position).normalize().dot(handle_vector) > 0.).is_some()
		{
			handle_vector = -handle_vector;
		}

		// Push both in and out handles into the correct position
		if let Some(in_handle) = length_previous.map(|length| anchor_position + handle_vector * length) {
			let point = ManipulatorPointId::new(manipulator.id, SelectedType::InHandle);
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorPosition { point, position: in_handle },
			});
		}

		if let Some(out_handle) = length_next.map(|length| anchor_position - handle_vector * length) {
			let point = ManipulatorPointId::new(manipulator.id, SelectedType::OutHandle);
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorPosition { point, position: out_handle },
			});
		}
	}

	/// Smooths the set of selected control points, assuming that the selected set is homogeneously sharp.
	pub fn smooth_selected_groups(&self, responses: &mut VecDeque<Message>, document_network: &NodeNetwork) -> Option<()> {
		let mut skip_set = HashSet::new();

		for (&layer, layer_state) in self.selected_shape_state.iter() {
			let subpaths = get_subpaths(layer, document_network)?;

			for point in layer_state.selected_points.iter() {
				if skip_set.contains(&point.group) {
					continue;
				};

				skip_set.insert(point.group);

				let anchor_selected = layer_state.selected_points.contains(&ManipulatorPointId {
					group: point.group,
					manipulator_type: SelectedType::Anchor,
				});
				let out_selected = layer_state.selected_points.contains(&ManipulatorPointId {
					group: point.group,
					manipulator_type: SelectedType::OutHandle,
				});
				let in_selected = layer_state.selected_points.contains(&ManipulatorPointId {
					group: point.group,
					manipulator_type: SelectedType::InHandle,
				});
				let group = graph_modification_utils::get_manipulator_from_id(subpaths, point.group)?;

				match (anchor_selected, out_selected, in_selected) {
					(_, true, false) => {
						let out_handle = ManipulatorPointId::new(point.group, SelectedType::OutHandle);
						if let Some(position) = group.out_handle {
							responses.add(GraphOperationMessage::Vector {
								layer,
								modification: VectorDataModification::SetManipulatorPosition { point: out_handle, position },
							});
						}
					}
					(_, false, true) => {
						let in_handle = ManipulatorPointId::new(point.group, SelectedType::InHandle);
						if let Some(position) = group.in_handle {
							responses.add(GraphOperationMessage::Vector {
								layer,
								modification: VectorDataModification::SetManipulatorPosition { point: in_handle, position },
							});
						}
					}
					(_, _, _) => {
						let found = subpaths.iter().find_map(|subpath| {
							let group_slice = subpath.manipulator_groups();
							let index = group_slice.iter().position(|manipulator| manipulator.id == group.id)?;
							// TODO: try subpath closed? wrapping
							Some((subpath, index))
						});

						if let Some((subpath, index)) = found {
							self.smooth_manipulator_group(subpath, index, responses, layer);
						}
					}
				}
			}
		}

		Some(())
	}

	/// Move the selected points by dragging the mouse.
	pub fn move_selected_points(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, delta: DVec2, mirror_distance: bool, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(layer, document_network) else { continue };
			let Some(mirror_angle) = get_mirror_handles(layer, document_network) else { continue };

			let transform = document_metadata.transform_to_viewport(layer);
			let delta = transform.inverse().transform_vector2(delta);

			for &point in state.selected_points.iter() {
				if point.manipulator_type.is_handle() && state.is_selected(ManipulatorPointId::new(point.group, SelectedType::Anchor)) {
					continue;
				}

				let Some(group) = get_manipulator_from_id(subpaths, point.group) else { continue };

				let mut move_point = |point: ManipulatorPointId| {
					let Some(previous_position) = point.manipulator_type.get_position(group) else { return };
					let position = previous_position + delta;
					responses.add(GraphOperationMessage::Vector {
						layer,
						modification: VectorDataModification::SetManipulatorPosition { point, position },
					});
				};

				move_point(point);

				if point.manipulator_type == SelectedType::Anchor {
					move_point(ManipulatorPointId::new(point.group, SelectedType::InHandle));
					move_point(ManipulatorPointId::new(point.group, SelectedType::OutHandle));
				}

				if mirror_distance && point.manipulator_type != SelectedType::Anchor {
					let mut mirror = mirror_angle.contains(&point.group);

					// If there is no opposing handle, we mirror even if mirror_angle doesn't contain the group
					// and set angle mirroring to true.
					if !mirror && point.manipulator_type.opposite().get_position(group).is_none() {
						responses.add(GraphOperationMessage::Vector {
							layer,
							modification: VectorDataModification::SetManipulatorHandleMirroring { id: group.id, mirror_angle: true },
						});
						mirror = true;
					}

					if mirror {
						let Some(mut original_handle_position) = point.manipulator_type.get_position(group) else {
							continue;
						};
						original_handle_position += delta;

						let point = ManipulatorPointId::new(point.group, point.manipulator_type.opposite());
						if state.is_selected(point) {
							continue;
						}
						let position = group.anchor - (original_handle_position - group.anchor);
						responses.add(GraphOperationMessage::Vector {
							layer,
							modification: VectorDataModification::SetManipulatorPosition { point, position },
						});
					}
				}
			}
		}
	}

	/// Delete selected and mirrored handles with zero length when the drag stops.
	pub fn delete_selected_handles_with_zero_length(
		&self,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		opposing_handle_lengths: &Option<OpposingHandleLengths>,
		responses: &mut VecDeque<Message>,
	) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(layer, document_network) else { continue };
			let Some(mirror_angle) = get_mirror_handles(layer, document_network) else { continue };

			let opposing_handle_lengths = opposing_handle_lengths.as_ref().and_then(|lengths| lengths.get(&layer));

			let transform = document_metadata.transform_to_viewport(layer);

			for &point in state.selected_points.iter() {
				let anchor = ManipulatorPointId::new(point.group, SelectedType::Anchor);
				if !point.manipulator_type.is_handle() || state.is_selected(anchor) {
					continue;
				}

				let Some(group) = get_manipulator_from_id(subpaths, point.group) else { continue };

				let anchor_position = transform.transform_point2(group.anchor);

				let point_position = if let Some(position) = point.manipulator_type.get_position(group) {
					transform.transform_point2(position)
				} else {
					continue;
				};

				if (anchor_position - point_position).length() < DRAG_THRESHOLD {
					responses.add(GraphOperationMessage::Vector {
						layer,
						modification: VectorDataModification::RemoveManipulatorPoint { point },
					});

					// Remove opposing handle if it is not selected and is mirrored.
					let opposite_point = ManipulatorPointId::new(point.group, point.manipulator_type.opposite());
					if !state.is_selected(opposite_point) && mirror_angle.contains(&point.group) {
						if let Some(lengths) = opposing_handle_lengths {
							if lengths.contains_key(&point.group) {
								responses.add(GraphOperationMessage::Vector {
									layer,
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
	pub fn opposing_handle_lengths(&self, document_network: &NodeNetwork) -> OpposingHandleLengths {
		self.selected_shape_state
			.iter()
			.filter_map(|(&layer, state)| {
				let subpaths = get_subpaths(layer, document_network)?;
				let opposing_handle_lengths = subpaths
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
				Some((layer, opposing_handle_lengths))
			})
			.collect::<HashMap<_, _>>()
	}

	/// Reset the opposing handle lengths.
	pub fn reset_opposing_handle_lengths(&self, document_network: &NodeNetwork, opposing_handle_lengths: &OpposingHandleLengths, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(layer, document_network) else { continue };
			let Some(mirror_angle) = get_mirror_handles(layer, document_network) else { continue };
			let Some(opposing_handle_lengths) = opposing_handle_lengths.get(&layer) else { continue };

			for subpath in subpaths {
				for manipulator_group in subpath.manipulator_groups() {
					if !mirror_angle.contains(&manipulator_group.id) {
						continue;
					}

					let Some(opposing_handle_length) = opposing_handle_lengths.get(&manipulator_group.id) else {
						continue;
					};

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
							layer,
							modification: VectorDataModification::RemoveManipulatorPoint {
								point: ManipulatorPointId::new(manipulator_group.id, single_selected_handle.opposite()),
							},
						});
						continue;
					};

					let Some(opposing_handle) = single_selected_handle.opposite().get_position(manipulator_group) else {
						continue;
					};

					let Some(offset) = (opposing_handle - manipulator_group.anchor).try_normalize() else { continue };

					let point = ManipulatorPointId::new(manipulator_group.id, single_selected_handle.opposite());
					let position = manipulator_group.anchor + offset * (*opposing_handle_length);
					assert!(position.is_finite(), "Opposing handle not finite!");

					responses.add(GraphOperationMessage::Vector {
						layer,
						modification: VectorDataModification::SetManipulatorPosition { point, position },
					});
				}
			}
		}
	}

	/// Dissolve the selected points.
	pub fn delete_selected_points(&self, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			for &point in &state.selected_points {
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::RemoveManipulatorPoint { point },
				})
			}
		}
	}

	/// Toggle if the handles should mirror angle across the anchor position.
	pub fn toggle_handle_mirroring_on_selected(&self, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			for point in &state.selected_points {
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::ToggleManipulatorHandleMirroring { id: point.group },
				})
			}
		}
	}

	/// Toggle if the handles should mirror angle across the anchor position.
	pub fn set_handle_mirroring_on_selected(&self, mirror_angle: bool, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			for point in &state.selected_points {
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::SetManipulatorHandleMirroring { id: point.group, mirror_angle },
				});
			}
		}
	}

	/// Iterate over the shapes.
	pub fn iter<'a>(&'a self, document_network: &'a NodeNetwork) -> impl Iterator<Item = &'a Vec<bezier_rs::Subpath<ManipulatorGroupId>>> + 'a {
		self.selected_shape_state.keys().filter_map(|&layer| get_subpaths(layer, document_network))
	}

	/// Find a [ManipulatorPoint] that is within the selection threshold and return the layer path, an index to the [ManipulatorGroup], and an enum index for [ManipulatorPoint].
	pub fn find_nearest_point_indices(
		&mut self,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		mouse_position: DVec2,
		select_threshold: f64,
	) -> Option<(LayerNodeIdentifier, ManipulatorPointId)> {
		if self.selected_shape_state.is_empty() {
			return None;
		}

		let select_threshold_squared = select_threshold * select_threshold;
		// Find the closest control point among all elements of shapes_to_modify
		for &layer in self.selected_shape_state.keys() {
			if let Some((manipulator_point_id, distance_squared)) = Self::closest_point_in_layer(document_network, document_metadata, layer, mouse_position) {
				// Choose the first point under the threshold
				if distance_squared < select_threshold_squared {
					trace!("Selecting... manipulator point: {manipulator_point_id:?}");
					return Some((layer, manipulator_point_id));
				}
			}
		}

		None
	}

	// TODO Use quadtree or some equivalent spatial acceleration structure to improve this to O(log(n))
	/// Find the closest manipulator, manipulator point, and distance so we can select path elements.
	/// Brute force comparison to determine which manipulator (handle or anchor) we want to select taking O(n) time.
	/// Return value is an `Option` of the tuple representing `(ManipulatorPointId, distance squared)`.
	fn closest_point_in_layer(document_network: &NodeNetwork, document_metadata: &DocumentMetadata, layer: LayerNodeIdentifier, pos: glam::DVec2) -> Option<(ManipulatorPointId, f64)> {
		let mut closest_distance_squared: f64 = f64::MAX;
		let mut result = None;

		let subpaths = get_subpaths(layer, document_network)?;
		let viewspace = document_metadata.transform_to_viewport(layer);
		for manipulator in get_manipulator_groups(subpaths) {
			let (selected, distance_squared) = SelectedType::closest_widget(manipulator, viewspace, pos, crate::consts::HIDE_HANDLE_DISTANCE);

			if distance_squared < closest_distance_squared {
				closest_distance_squared = distance_squared;
				result = Some((ManipulatorPointId::new(manipulator.id, selected), distance_squared));
			}
		}

		result
	}

	/// Find the `t` value along the path segment we have clicked upon, together with that segment ID.
	fn closest_segment(
		&self,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		layer: LayerNodeIdentifier,
		position: glam::DVec2,
		tolerance: f64,
	) -> Option<(ManipulatorGroupId, ManipulatorGroupId, Bezier, f64)> {
		let transform = document_metadata.transform_to_viewport(layer);
		let layer_pos = transform.inverse().transform_point2(position);
		let projection_options = bezier_rs::ProjectionOptions { lut_size: 5, ..Default::default() };

		let mut result = None;
		let mut closest_distance_squared: f64 = tolerance * tolerance;

		let subpaths = get_subpaths(layer, document_network)?;

		for subpath in subpaths {
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
	pub fn split(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) {
		for &layer in self.selected_layers() {
			if let Some((start, end, bezier, t)) = self.closest_segment(document_network, document_metadata, layer, position, tolerance) {
				let [first, second] = bezier.split(TValue::Parametric(t));

				// Adjust the first manipulator group's out handle
				let point = ManipulatorPointId::new(start, SelectedType::OutHandle);
				let position = first.handle_start().unwrap_or(first.start());
				let out_handle = GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::SetManipulatorPosition { point, position },
				};
				responses.add(out_handle);

				// Insert a new manipulator group between the existing ones
				let manipulator_group = ManipulatorGroup::new(first.end(), first.handle_end(), second.handle_start());
				let insert = GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::AddManipulatorGroup { manipulator_group, after_id: start },
				};
				responses.add(insert);

				// Adjust the last manipulator group's in handle
				let point = ManipulatorPointId::new(end, SelectedType::InHandle);
				let position = second.handle_end().unwrap_or(second.end());
				let in_handle = GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::SetManipulatorPosition { point, position },
				};
				responses.add(in_handle);

				return;
			}
		}
	}

	/// Handles the flipping between sharp corner and smooth (which can be activated by double clicking on an anchor with the Path tool).
	pub fn flip_sharp(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) -> bool {
		let mut process_layer = |layer| {
			let subpaths = get_subpaths(layer, document_network)?;

			let transform_to_screenspace = document_metadata.transform_to_viewport(layer);
			let mut result = None;
			let mut closest_distance_squared = tolerance * tolerance;

			// Find the closest anchor point on the current layer
			for (subpath_index, subpath) in subpaths.iter().enumerate() {
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

			let subpath = &subpaths[subpath_index];

			// Check by comparing the handle positions to the anchor if this manipulator group is a point
			let already_sharp = match (manipulator.in_handle, manipulator.out_handle) {
				(Some(in_handle), Some(out_handle)) => anchor_position.abs_diff_eq(in_handle, 1e-10) && anchor_position.abs_diff_eq(out_handle, 1e-10),
				(Some(handle), None) | (None, Some(handle)) => anchor_position.abs_diff_eq(handle, 1e-10),
				(None, None) => true,
			};

			if already_sharp {
				self.smooth_manipulator_group(subpath, index, responses, layer);
			} else {
				let point = ManipulatorPointId::new(manipulator.id, SelectedType::InHandle);
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::SetManipulatorPosition { point, position: anchor_position },
				});
				let point = ManipulatorPointId::new(manipulator.id, SelectedType::OutHandle);
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::SetManipulatorPosition { point, position: anchor_position },
				});
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification: VectorDataModification::SetManipulatorHandleMirroring {
						id: manipulator.id,
						mirror_angle: false,
					},
				});
			};

			Some(true)
		};
		for &layer in self.selected_shape_state.keys() {
			if let Some(result) = process_layer(layer) {
				return result;
			}
		}
		false
	}

	pub fn select_all_in_quad(&mut self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, quad: [DVec2; 2], clear_selection: bool) {
		for (&layer, state) in &mut self.selected_shape_state {
			if clear_selection {
				state.clear_points()
			}

			let Some(subpaths) = get_subpaths(layer, document_network) else { continue };

			let transform = document_metadata.transform_to_viewport(layer);

			for manipulator_group in get_manipulator_groups(subpaths) {
				for selected_type in [SelectedType::Anchor, SelectedType::InHandle, SelectedType::OutHandle] {
					let Some(position) = selected_type.get_position(manipulator_group) else { continue };
					let transformed_position = transform.transform_point2(position);

					if quad[0].min(quad[1]).cmple(transformed_position).all() && quad[0].max(quad[1]).cmpge(transformed_position).all() {
						state.select_point(ManipulatorPointId::new(manipulator_group.id, selected_type));
					}
				}
			}
		}
	}
}
