use super::graph_modification_utils;
use super::snapping::{SnapCache, SnapCandidatePoint, SnapData, SnapManager, SnappedPoint};
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{GeometrySnapSource, SnapSource};
use crate::messages::portfolio::document::utility_types::network_interface::{self, NodeNetworkInterface};
use crate::messages::prelude::*;

use bezier_rs::{Bezier, BezierHandles, TValue};
use graphene_core::transform::Transform;
use graphene_core::vector::{ManipulatorPointId, PointId, VectorData, VectorModificationType};

use glam::DVec2;
use graphene_std::vector::{HandleId, SegmentId};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ManipulatorAngle {
	Colinear,
	Free,
	Mixed,
}

#[derive(Clone, Debug, Default)]
pub struct SelectedLayerState {
	selected_points: HashSet<ManipulatorPointId>,
}

impl SelectedLayerState {
	pub fn selected(&self) -> impl Iterator<Item = ManipulatorPointId> + '_ {
		self.selected_points.iter().copied()
	}
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

#[derive(Debug)]
pub struct SelectedPointsInfo {
	pub points: Vec<ManipulatorPointInfo>,
	pub offset: DVec2,
	pub vector_data: VectorData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManipulatorPointInfo {
	pub layer: LayerNodeIdentifier,
	pub point_id: ManipulatorPointId,
}

pub type OpposingHandleLengths = HashMap<LayerNodeIdentifier, HashMap<HandleId, f64>>;

pub struct ClosestSegment {
	layer: LayerNodeIdentifier,
	segment: SegmentId,
	bezier: Bezier,
	points: [PointId; 2],
	colinear: [Option<HandleId>; 2],
	t: f64,
	bezier_point_to_viewport: DVec2,
	stroke_width: f64,
}

impl ClosestSegment {
	pub fn layer(&self) -> LayerNodeIdentifier {
		self.layer
	}

	pub fn closest_point_to_viewport(&self) -> DVec2 {
		self.bezier_point_to_viewport
	}

	/// Updates this [`ClosestSegment`] with the viewport-space location of the closest point on the segment to the given mouse position.
	pub fn update_closest_point(&mut self, document_metadata: &DocumentMetadata, mouse_position: DVec2) {
		let transform = document_metadata.transform_to_viewport(self.layer);
		let layer_mouse_pos = transform.inverse().transform_point2(mouse_position);

		let t = self.bezier.project(layer_mouse_pos).clamp(0., 1.);
		self.t = t;

		let bezier_point = self.bezier.evaluate(TValue::Parametric(t));
		let bezier_point = transform.transform_point2(bezier_point);
		self.bezier_point_to_viewport = bezier_point;
	}

	pub fn distance_squared(&self, mouse_position: DVec2) -> f64 {
		self.bezier_point_to_viewport.distance_squared(mouse_position)
	}

	pub fn too_far(&self, mouse_position: DVec2, tolerance: f64, document_metadata: &DocumentMetadata) -> bool {
		let dist_sq = self.distance_squared(mouse_position);
		let stroke_width = document_metadata.document_to_viewport.decompose_scale().x.max(1.) * self.stroke_width;
		let stroke_width_sq = stroke_width * stroke_width;
		let tolerance_sq = tolerance * tolerance;
		(stroke_width_sq + tolerance_sq) < dist_sq
	}

	pub fn adjusted_insert(&self, responses: &mut VecDeque<Message>) -> PointId {
		let layer = self.layer;
		let [first, second] = self.bezier.split(TValue::Parametric(self.t));

		// Point
		let midpoint = PointId::generate();
		let modification_type = VectorModificationType::InsertPoint { id: midpoint, position: first.end };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// First segment
		let segment_ids = [SegmentId::generate(), SegmentId::generate()];
		let modification_type = VectorModificationType::InsertSegment {
			id: segment_ids[0],
			points: [self.points[0], midpoint],
			handles: [first.handle_start().map(|handle| handle - first.start), first.handle_end().map(|handle| handle - first.end)],
		};
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Last segment
		let modification_type = VectorModificationType::InsertSegment {
			id: segment_ids[1],
			points: [midpoint, self.points[1]],
			handles: [second.handle_start().map(|handle| handle - second.start), second.handle_end().map(|handle| handle - second.end)],
		};
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// G1 continuous on new handles
		if self.bezier.handle_end().is_some() {
			let handles = [HandleId::end(segment_ids[0]), HandleId::primary(segment_ids[1])];
			let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: true };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}

		// Remove old segment
		let modification_type = VectorModificationType::RemoveSegment { id: self.segment };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Restore mirroring on end handles
		for (handle, other) in self.colinear.into_iter().zip([HandleId::primary(segment_ids[0]), HandleId::end(segment_ids[1])]) {
			let Some(handle) = handle else { continue };
			let handles = [handle, other];
			let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: true };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}

		midpoint
	}

	pub fn adjusted_insert_and_select(&self, shape_editor: &mut ShapeState, responses: &mut VecDeque<Message>, add_to_selection: bool) {
		let id = self.adjusted_insert(responses);
		shape_editor.select_anchor_point_by_id(self.layer, id, add_to_selection)
	}
}

// TODO Consider keeping a list of selected manipulators to minimize traversals of the layers
impl ShapeState {
	// Snap, returning a viewport delta
	pub fn snap(&self, snap_manager: &mut SnapManager, snap_cache: &SnapCache, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, previous_mouse: DVec2) -> DVec2 {
		let snap_data = SnapData::new_snap_cache(document, input, snap_cache);

		let mouse_delta = document
			.network_interface
			.document_metadata()
			.document_to_viewport
			.inverse()
			.transform_vector2(input.mouse.position - previous_mouse);
		let mut offset = mouse_delta;
		let mut best_snapped = SnappedPoint::infinite_snap(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
		for (layer, state) in &self.selected_shape_state {
			let Some(vector_data) = document.network_interface.compute_modified_vector(*layer) else {
				continue;
			};

			let to_document = document.metadata().transform_to_document(*layer);

			for &selected in &state.selected_points {
				let source = match selected {
					ManipulatorPointId::Anchor(_) if vector_data.colinear(selected) => SnapSource::Geometry(GeometrySnapSource::AnchorWithColinearHandles),
					ManipulatorPointId::Anchor(_) => SnapSource::Geometry(GeometrySnapSource::AnchorWithFreeHandles),
					_ => SnapSource::Geometry(GeometrySnapSource::Handle),
				};

				let Some(position) = selected.get_position(&vector_data) else { continue };
				let mut point = SnapCandidatePoint::new_source(to_document.transform_point2(position) + mouse_delta, source);

				if let Some(id) = selected.as_anchor() {
					for neighbor in vector_data.connected_points(id) {
						if state.is_selected(ManipulatorPointId::Anchor(neighbor)) {
							continue;
						}
						let Some(position) = vector_data.point_domain.position_from_id(neighbor) else { continue };
						point.neighbors.push(to_document.transform_point2(position));
					}
				}

				let snapped = snap_manager.free_snap(&snap_data, &point, None, false);
				if best_snapped.other_snap_better(&snapped) {
					offset = snapped.snapped_point_document - point.document_point + mouse_delta;
					best_snapped = snapped;
				}
			}
		}
		snap_manager.update_indicator(best_snapped);
		document.metadata().document_to_viewport.transform_vector2(offset)
	}

	/// Select/deselect the first point within the selection threshold.
	/// Returns a tuple of the points if found and the offset, or `None` otherwise.
	pub fn change_point_selection(&mut self, network_interface: &NodeNetworkInterface, mouse_position: DVec2, select_threshold: f64, add_to_selection: bool) -> Option<Option<SelectedPointsInfo>> {
		if self.selected_shape_state.is_empty() {
			return None;
		}

		if let Some((layer, manipulator_point_id)) = self.find_nearest_point_indices(network_interface, mouse_position, select_threshold) {
			let vector_data = network_interface.compute_modified_vector(layer)?;
			let point_position = manipulator_point_id.get_position(&vector_data)?;

			let selected_shape_state = self.selected_shape_state.get(&layer)?;
			let already_selected = selected_shape_state.is_selected(manipulator_point_id);

			// Should we select or deselect the point?
			let new_selected = if already_selected { !add_to_selection } else { true };

			// Offset to snap the selected point to the cursor
			let offset = mouse_position - network_interface.document_metadata().transform_to_viewport(layer).transform_point2(point_position);

			// This is selecting the manipulator only for now, next to generalize to points
			if new_selected {
				let retain_existing_selection = add_to_selection || already_selected;
				if !retain_existing_selection {
					self.deselect_all_points();
				}

				// Add to the selected points
				let selected_shape_state = self.selected_shape_state.get_mut(&layer)?;
				selected_shape_state.select_point(manipulator_point_id);

				let points = self
					.selected_shape_state
					.iter()
					.flat_map(|(layer, state)| state.selected_points.iter().map(|&point_id| ManipulatorPointInfo { layer: *layer, point_id }))
					.collect();

				return Some(Some(SelectedPointsInfo { points, offset, vector_data }));
			} else {
				let selected_shape_state = self.selected_shape_state.get_mut(&layer)?;
				selected_shape_state.deselect_point(manipulator_point_id);

				return Some(None);
			}
		}
		None
	}

	pub fn select_anchor_point_by_id(&mut self, layer: LayerNodeIdentifier, id: PointId, add_to_selection: bool) {
		if !add_to_selection {
			self.deselect_all_points();
		}
		let point = ManipulatorPointId::Anchor(id);
		let Some(selected_state) = self.selected_shape_state.get_mut(&layer) else { return };
		selected_state.select_point(point);
	}

	/// Selects all anchors connected to the selected subpath, and deselects all handles, for the given layer.
	pub fn select_connected_anchors(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier, mouse: DVec2) {
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			return;
		};
		let to_viewport = document.metadata().transform_to_viewport(layer);
		let layer_mouse = to_viewport.inverse().transform_point2(mouse);
		let state = self.selected_shape_state.entry(layer).or_default();

		let mut selected_stack = Vec::new();
		// Find all subpaths that have been clicked
		for stroke in vector_data.stroke_bezier_paths() {
			if stroke.contains_point(layer_mouse) {
				if let Some(first) = stroke.manipulator_groups().first() {
					selected_stack.push(first.id);
				}
			}
		}
		state.clear_points();
		if selected_stack.is_empty() {
			// Fall back on just selecting all points in the layer
			for &point in vector_data.point_domain.ids() {
				state.select_point(ManipulatorPointId::Anchor(point))
			}
		} else {
			// Select all connected points
			while let Some(point) = selected_stack.pop() {
				if !state.is_selected(ManipulatorPointId::Anchor(point)) {
					state.select_point(ManipulatorPointId::Anchor(point));
					selected_stack.extend(vector_data.connected_points(point));
				}
			}
		}
	}

	/// Selects all anchors, and deselects all handles, for the given layer.
	pub fn select_all_anchors_in_layer(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier) {
		let state = self.selected_shape_state.entry(layer).or_default();
		Self::select_all_anchors_in_layer_with_state(document, layer, state);
	}

	/// Selects all anchors, and deselects all handles, for the selected layers.
	pub fn select_all_anchors_in_selected_layers(&mut self, document: &DocumentMessageHandler) {
		for (&layer, state) in self.selected_shape_state.iter_mut() {
			Self::select_all_anchors_in_layer_with_state(document, layer, state);
		}
	}

	/// Internal helper function that selects all anchors, and deselects all handles, for a layer given its [`LayerNodeIdentifier`] and [`SelectedLayerState`].
	fn select_all_anchors_in_layer_with_state(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, state: &mut SelectedLayerState) {
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			return;
		};

		state.clear_points();

		for &point in vector_data.point_domain.ids() {
			state.select_point(ManipulatorPointId::Anchor(point))
		}
	}

	/// Deselects all points (anchors and handles) across every selected layer.
	pub fn deselect_all_points(&mut self) {
		for state in self.selected_shape_state.values_mut() {
			state.selected_points.clear()
		}
	}

	/// Set the shapes we consider for selection, we will choose draggable manipulators from these shapes.
	pub fn set_selected_layers(&mut self, target_layers: Vec<LayerNodeIdentifier>) {
		self.selected_shape_state.retain(|layer_path, _| target_layers.contains(layer_path));
		for layer in target_layers {
			self.selected_shape_state.entry(layer).or_default();
		}
	}

	/// Returns an iterator over the currently selected layers to get their [`LayerNodeIdentifier`]s.
	pub fn selected_layers(&self) -> impl Iterator<Item = &LayerNodeIdentifier> {
		self.selected_shape_state.keys()
	}

	/// iterate over all selected layers in order from top to bottom
	/// # WARN
	/// iterate over all layers of the document
	pub fn sorted_selected_layers<'a>(&'a self, document_metadata: &'a DocumentMetadata) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		document_metadata.all_layers().filter(|layer| self.selected_shape_state.contains_key(layer))
	}

	pub fn has_selected_layers(&self) -> bool {
		!self.selected_shape_state.is_empty()
	}

	/// Provide the currently selected points by reference.
	pub fn selected_points(&self) -> impl Iterator<Item = &'_ ManipulatorPointId> {
		self.selected_shape_state.values().flat_map(|state| &state.selected_points)
	}

	pub fn selected_points_in_layer(&self, layer: LayerNodeIdentifier) -> Option<&HashSet<ManipulatorPointId>> {
		self.selected_shape_state.get(&layer).map(|state| &state.selected_points)
	}

	pub fn move_primary(&self, segment: SegmentId, delta: DVec2, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: VectorModificationType::ApplyPrimaryDelta { segment, delta },
		});
	}

	pub fn move_end(&self, segment: SegmentId, delta: DVec2, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: VectorModificationType::ApplyEndDelta { segment, delta },
		});
	}

	pub fn move_anchor(&self, point: PointId, vector_data: &VectorData, delta: DVec2, layer: LayerNodeIdentifier, selected: Option<&SelectedLayerState>, responses: &mut VecDeque<Message>) {
		// Move anchor
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: VectorModificationType::ApplyPointDelta { point, delta },
		});

		// Move the other handle for a quadratic bezier
		for segment in vector_data.end_connected(point) {
			let Some((start, _end, bezier)) = vector_data.segment_points_from_id(segment) else { continue };

			if let BezierHandles::Quadratic { handle } = bezier.handles {
				if selected.is_some_and(|selected| selected.is_selected(ManipulatorPointId::Anchor(start))) {
					continue;
				}

				let relative_position = handle - bezier.start + delta;
				let modification_type = VectorModificationType::SetPrimaryHandle { segment, relative_position };

				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
		}
	}

	/// Moves a control point to a `new_position` in document space.
	/// Returns `Some(())` if successful and `None` otherwise.
	pub fn reposition_control_point(
		&self,
		point: &ManipulatorPointId,
		network_interface: &NodeNetworkInterface,
		new_position: DVec2,
		layer: LayerNodeIdentifier,
		responses: &mut VecDeque<Message>,
	) -> Option<()> {
		let vector_data = network_interface.compute_modified_vector(layer)?;
		let transform = network_interface.document_metadata().transform_to_document(layer).inverse();
		let position = transform.transform_point2(new_position);
		let current_position = point.get_position(&vector_data)?;
		let delta = position - current_position;

		match *point {
			ManipulatorPointId::Anchor(point) => self.move_anchor(point, &vector_data, delta, layer, None, responses),
			ManipulatorPointId::PrimaryHandle(segment) => {
				self.move_primary(segment, delta, layer, responses);
				if let Some(handles) = point.get_handle_pair(&vector_data) {
					let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
			ManipulatorPointId::EndHandle(segment) => {
				self.move_end(segment, delta, layer, responses);
				if let Some(handles) = point.get_handle_pair(&vector_data) {
					let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}

		Some(())
	}

	/// Iterates over the selected manipulator groups, returning whether their handles have mixed, colinear, or free angles.
	/// If there are no points selected this function returns mixed.
	pub fn selected_manipulator_angles(&self, network_interface: &NodeNetworkInterface) -> ManipulatorAngle {
		// This iterator contains a bool indicating whether or not selected points' manipulator groups have colinear handles.
		let mut points_colinear_status = self
			.selected_shape_state
			.iter()
			.map(|(&layer, selection_state)| (network_interface.compute_modified_vector(layer), selection_state))
			.flat_map(|(data, selection_state)| selection_state.selected_points.iter().map(move |&point| data.as_ref().map_or(false, |data| data.colinear(point))));

		let Some(first_is_colinear) = points_colinear_status.next() else { return ManipulatorAngle::Mixed };
		if points_colinear_status.any(|point| first_is_colinear != point) {
			return ManipulatorAngle::Mixed;
		}
		match first_is_colinear {
			false => ManipulatorAngle::Free,
			true => ManipulatorAngle::Colinear,
		}
	}

	pub fn convert_manipulator_handles_to_colinear(&self, vector_data: &VectorData, point_id: PointId, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier) {
		let Some(anchor_position) = ManipulatorPointId::Anchor(point_id).get_position(vector_data) else {
			return;
		};
		let handles = vector_data.all_connected(point_id).take(2).collect::<Vec<_>>();

		// Grab the next and previous manipulator groups by simply looking at the next / previous index
		let points = handles.iter().map(|handle| vector_data.other_point(handle.segment, point_id));
		let anchor_positions = points
			.map(|point| point.and_then(|point| ManipulatorPointId::Anchor(point).get_position(vector_data)))
			.collect::<Vec<_>>();

		// Use the position relative to the anchor
		let mut directions = anchor_positions
			.iter()
			.map(|position| position.map(|position| (position - anchor_position)).and_then(DVec2::try_normalize));

		// The direction of the handles is either the perpendicular vector to the sum of the anchors' positions or just the anchor's position (if only one)
		let mut handle_direction = match (directions.next().flatten(), directions.next().flatten()) {
			(Some(previous), Some(next)) => (previous - next).try_normalize().unwrap_or(next.perp()),
			(Some(val), None) | (None, Some(val)) => val,
			(None, None) => return,
		};

		// Set the manipulator to have colinear handles
		if let (Some(a), Some(b)) = (handles.first(), handles.get(1)) {
			let handles = [*a, *b];
			let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: true };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}

		// Flip the vector if it is not facing towards the same direction as the anchor
		let [first, second] = [anchor_positions.first().copied().flatten(), anchor_positions.get(1).copied().flatten()];
		if first.is_some_and(|group| (group - anchor_position).normalize_or_zero().dot(handle_direction) < 0.)
			|| second.is_some_and(|group| (group - anchor_position).normalize_or_zero().dot(handle_direction) > 0.)
		{
			handle_direction *= -1.;
		}

		// Push both in and out handles into the correct position
		for ((handle, sign), other_anchor) in handles.iter().zip([1., -1.]).zip(&anchor_positions) {
			// To find the length of the new tangent we just take the distance to the anchor and divide by 3 (pretty arbitrary)
			let Some(length) = other_anchor.map(|position| (position - anchor_position).length() / 3.) else {
				continue;
			};
			let new_position = handle_direction * length * sign;
			let modification_type = handle.set_relative_position(new_position);
			responses.add(GraphOperationMessage::Vector { layer, modification_type });

			// Create the opposite handle if it doesn't exist (if it is not a cubic segment)
			if handle.opposite().to_manipulator_point().get_position(vector_data).is_none() {
				let modification_type = handle.opposite().set_relative_position(DVec2::ZERO);
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
		}
	}

	/// Converts all selected points to colinear while moving the handles to ensure their 180° angle separation.
	/// If only one handle is selected, the other handle will be moved to match the angle of the selected handle.
	/// If both or neither handles are selected, the angle of both handles will be averaged from their current angles, weighted by their lengths.
	/// Assumes all selected manipulators have handles that are already not colinear.
	pub fn convert_selected_manipulators_to_colinear_handles(&self, responses: &mut VecDeque<Message>, document: &DocumentMessageHandler) {
		let mut skip_set = HashSet::new();

		for (&layer, layer_state) in self.selected_shape_state.iter() {
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				continue;
			};
			let transform = document.metadata().transform_to_document(layer);

			for &point in layer_state.selected_points.iter() {
				let Some(handles) = point.get_handle_pair(&vector_data) else { continue };
				if skip_set.contains(&handles) || skip_set.contains(&[handles[1], handles[0]]) {
					continue;
				};

				skip_set.insert(handles);

				let [selected0, selected1] = handles.map(|handle| layer_state.selected_points.contains(&handle.to_manipulator_point()));
				let handle_positions = handles.map(|handle| handle.to_manipulator_point().get_position(&vector_data));

				let Some(anchor_id) = point.get_anchor(&vector_data) else { continue };
				let Some(anchor) = vector_data.point_domain.position_from_id(anchor_id) else { continue };

				let anchor_points = handles.map(|handle| vector_data.other_point(handle.segment, anchor_id));
				let anchor_positions = anchor_points.map(|point| point.and_then(|point| vector_data.point_domain.position_from_id(point)));

				// If one handle is selected (but both exist), only move the other handle
				if let (true, [Some(pos0), Some(pos1)]) = ((selected0 ^ selected1), handle_positions) {
					let [(_selected_handle, selected_position), (unselected_handle, unselected_position)] = if selected0 {
						[(handles[0], pos0), (handles[1], pos1)]
					} else {
						[(handles[1], pos1), (handles[0], pos0)]
					};
					let direction = transform
						.transform_vector2(anchor - selected_position)
						.try_normalize()
						.unwrap_or_else(|| transform.transform_vector2(unselected_position - anchor).normalize_or_zero());

					let length = transform.transform_vector2(unselected_position - anchor).length();
					let position = transform.inverse().transform_vector2(direction * length);
					let modification_type = unselected_handle.set_relative_position(position);
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
				// If both handles are selected, average the angles of the handles
				else {
					// We could normalize these directions?
					let mut handle_directions = handle_positions.map(|handle| handle.map(|handle| handle - anchor));

					let mut normalized = handle_directions[0].and_then(|a| handle_directions[1].and_then(|b| (a - b).try_normalize()));

					if normalized.is_none() {
						handle_directions = anchor_positions.map(|relative_anchor| relative_anchor.map(|relative_anchor| (relative_anchor - anchor) / 3.));
						normalized = handle_directions[0].and_then(|a| handle_directions[1].and_then(|b| (a - b).try_normalize()))
					}
					let Some(normalized) = normalized else { continue };

					// Push both in and out handles into the correct position
					for (index, sign) in [(0, 1.), (1, -1.)] {
						let Some(direction) = handle_directions[index] else { continue };
						let new_position = direction.length() * normalized * sign;
						let modification_type = handles[index].set_relative_position(new_position);
						responses.add(GraphOperationMessage::Vector { layer, modification_type });

						// Create the opposite handle if it doesn't exist (if it is not a cubic segment)
						if handles[index].opposite().to_manipulator_point().get_position(&vector_data).is_none() {
							let modification_type = handles[index].opposite().set_relative_position(DVec2::ZERO);
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}
				}
				let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: true };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
		}
	}

	/// Move the selected points by dragging the mouse.
	pub fn move_selected_points(&self, handle_lengths: Option<OpposingHandleLengths>, document: &DocumentMessageHandler, delta: DVec2, equidistant: bool, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				continue;
			};
			let opposing_handles = handle_lengths.as_ref().and_then(|handle_lengths| handle_lengths.get(&layer));

			let transform = document.metadata().transform_to_viewport(layer);
			let delta = transform.inverse().transform_vector2(delta);

			for &point in state.selected_points.iter() {
				let handle = match point {
					ManipulatorPointId::Anchor(point) => {
						self.move_anchor(point, &vector_data, delta, layer, Some(state), responses);
						continue;
					}
					ManipulatorPointId::PrimaryHandle(segment) => HandleId::primary(segment),
					ManipulatorPointId::EndHandle(segment) => HandleId::end(segment),
				};

				let Some(anchor_id) = point.get_anchor(&vector_data) else { continue };
				if state.is_selected(ManipulatorPointId::Anchor(anchor_id)) {
					continue;
				}

				let Some(anchor_position) = vector_data.point_domain.position_from_id(anchor_id) else { continue };

				let Some(handle_position) = point.get_position(&vector_data) else { continue };
				let handle_position = handle_position + delta;

				let modification_type = handle.set_relative_position(handle_position - anchor_position);

				responses.add(GraphOperationMessage::Vector { layer, modification_type });

				let Some(other) = vector_data.other_colinear_handle(handle) else { continue };
				if state.is_selected(other.to_manipulator_point()) {
					continue;
				}

				let new_relative = if equidistant {
					-(handle_position - anchor_position)
				} else {
					let transform = document.metadata().document_to_viewport.inverse() * transform;
					let Some(other_position) = other.to_manipulator_point().get_position(&vector_data) else {
						continue;
					};
					let direction = transform.transform_vector2(handle_position - anchor_position).try_normalize();
					let opposing_handle = opposing_handles.and_then(|handles| handles.get(&other));
					let length = opposing_handle.copied().unwrap_or_else(|| transform.transform_vector2(other_position - anchor_position).length());
					direction.map_or(other_position - anchor_position, |direction| transform.inverse().transform_vector2(-direction * length))
				};
				let modification_type = other.set_relative_position(new_relative);

				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
		}
	}

	/// The opposing handle lengths.
	pub fn opposing_handle_lengths(&self, document: &DocumentMessageHandler) -> OpposingHandleLengths {
		self.selected_shape_state
			.iter()
			.filter_map(|(&layer, state)| {
				let vector_data = document.network_interface.compute_modified_vector(layer)?;
				let transform = document.metadata().transform_to_document(layer);
				let opposing_handle_lengths = vector_data
					.colinear_manipulators
					.iter()
					.filter_map(|&handles| {
						// We will keep track of the opposing handle length when:
						// i) Exactly one handle is selected.
						// ii) The anchor is not selected.

						let anchor = handles[0].to_manipulator_point().get_anchor(&vector_data)?;
						let anchor_selected = state.is_selected(ManipulatorPointId::Anchor(anchor));
						if anchor_selected {
							return None;
						}

						let handles_selected = handles.map(|handle| state.is_selected(handle.to_manipulator_point()));

						let other = match handles_selected {
							[true, false] => handles[1],
							[false, true] => handles[0],
							_ => return None,
						};

						let opposing_handle_position = other.to_manipulator_point().get_position(&vector_data)?;
						let anchor_position = vector_data.point_domain.position_from_id(anchor)?;

						let opposing_handle_length = transform.transform_vector2(opposing_handle_position - anchor_position).length();
						Some((other, opposing_handle_length))
					})
					.collect::<HashMap<_, _>>();
				Some((layer, opposing_handle_lengths))
			})
			.collect::<HashMap<_, _>>()
	}

	fn dissolve_anchor(anchor: PointId, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, vector_data: &VectorData) -> Option<[(HandleId, PointId); 2]> {
		// Delete point
		let modification_type = VectorModificationType::RemovePoint { id: anchor };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Delete connected segments
		for HandleId { segment, .. } in vector_data.all_connected(anchor) {
			let modification_type = VectorModificationType::RemoveSegment { id: segment };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
			for &handles in vector_data.colinear_manipulators.iter().filter(|handles| handles.iter().any(|handle| handle.segment == segment)) {
				let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
		}

		// Add in new segment if possible
		let mut handles = ManipulatorPointId::Anchor(anchor).get_handle_pair(vector_data)?;
		handles.reverse();
		let opposites = handles.map(|handle| handle.opposite());

		let [Some(start), Some(end)] = opposites.map(|opposite| opposite.to_manipulator_point().get_anchor(vector_data)) else {
			return None;
		};
		Some([(handles[0], start), (handles[1], end)])
	}

	/// Dissolve the selected points.
	pub fn delete_selected_points(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &mut self.selected_shape_state {
			let mut missing_anchors = HashMap::new();
			let mut deleted_anchors = HashSet::new();
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				continue;
			};

			for point in std::mem::take(&mut state.selected_points) {
				match point {
					ManipulatorPointId::Anchor(anchor) => {
						if let Some(handles) = Self::dissolve_anchor(anchor, responses, layer, &vector_data) {
							missing_anchors.insert(anchor, handles);
						}
						deleted_anchors.insert(anchor);
					}
					ManipulatorPointId::PrimaryHandle(_) | ManipulatorPointId::EndHandle(_) => {
						let Some(handle) = point.as_handle() else { continue };

						// Place the handle on top of the anchor
						let modification_type = handle.set_relative_position(DVec2::ZERO);
						responses.add(GraphOperationMessage::Vector { layer, modification_type });

						// Disable the g1 continuous
						for &handles in &vector_data.colinear_manipulators {
							if handles.contains(&handle) {
								let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
								responses.add(GraphOperationMessage::Vector { layer, modification_type });
							}
						}
					}
				}
			}

			let mut visited = Vec::new();
			while let Some((anchor, handles)) = missing_anchors.keys().next().copied().and_then(|id| missing_anchors.remove_entry(&id)) {
				visited.push(anchor);

				// If the adgacent point is just this point then skip
				let mut handles = handles.map(|handle| (handle.1 != anchor).then_some(handle));

				// If the adjacent points are themselves being deleted, then repeatedly visit the newest agacent points.
				for handle in &mut handles {
					while let Some((point, connected)) = (*handle).and_then(|(_, point)| missing_anchors.remove_entry(&point)) {
						visited.push(point);

						*handle = connected.into_iter().find(|(_, point)| !visited.contains(point));
					}
				}

				let [Some(start), Some(end)] = handles else { continue };

				// Avoid reconnecting to points that are being deleted (this can happen if a whole loop is deleted)
				if deleted_anchors.contains(&start.1) || deleted_anchors.contains(&end.1) {
					continue;
				}

				// Grab the handles from the opposite side of the segment(s) being deleted and make it relative to the anchor
				let [handle_start, handle_end] = [start, end].map(|(handle, _)| {
					let handle = handle.opposite();
					let handle_position = handle.to_manipulator_point().get_position(&vector_data);
					let relative_position = handle
						.to_manipulator_point()
						.get_anchor(&vector_data)
						.and_then(|anchor| vector_data.point_domain.position_from_id(anchor));
					handle_position.and_then(|handle| relative_position.map(|relative| handle - relative)).unwrap_or_default()
				});

				let segment = start.0.segment;

				let modification_type = VectorModificationType::InsertSegment {
					id: segment,
					points: [start.1, end.1],
					handles: [Some(handle_start), Some(handle_end)],
				};

				responses.add(GraphOperationMessage::Vector { layer, modification_type });

				for &handles in vector_data.colinear_manipulators.iter() {
					if !handles.iter().any(|&handle| handle == start.0.opposite() || handle == end.0.opposite()) {
						continue;
					}

					let Some(anchor) = handles[0].to_manipulator_point().get_anchor(&vector_data) else { continue };
					let Some(other) = handles.iter().find(|&&handle| handle != start.0.opposite() && handle != end.0.opposite()) else {
						continue;
					};

					let handle_ty = if anchor == start.1 {
						HandleId::primary(segment)
					} else if anchor == end.1 {
						HandleId::end(segment)
					} else {
						continue;
					};
					let handles = [*other, handle_ty];
					let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: true };

					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}
	}

	pub fn break_path_at_selected_point(&self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				continue;
			};

			for &delete in &state.selected_points {
				let Some(point) = delete.get_anchor(&vector_data) else { continue };
				let Some(pos) = vector_data.point_domain.position_from_id(point) else { continue };

				let mut used_initial_point = false;
				for handle in vector_data.all_connected(point) {
					// Disable the g1 continuous
					for &handles in &vector_data.colinear_manipulators {
						if handles.contains(&handle) {
							let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}

					// Keep the existing point for the first segment
					if !used_initial_point {
						used_initial_point = true;
						continue;
					}

					// Create new point
					let id = PointId::generate();
					let modification_type = VectorModificationType::InsertPoint { id, position: pos };

					responses.add(GraphOperationMessage::Vector { layer, modification_type });

					// Update segment
					let HandleId { ty, segment } = handle;
					let modification_type = match ty {
						graphene_std::vector::HandleType::Primary => VectorModificationType::SetStartPoint { segment, id },
						graphene_std::vector::HandleType::End => VectorModificationType::SetEndPoint { segment, id },
					};

					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}
	}

	/// Delete point(s) and adjacent segments.
	pub fn delete_point_and_break_path(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &mut self.selected_shape_state {
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				continue;
			};

			for delete in std::mem::take(&mut state.selected_points) {
				let Some(point) = delete.get_anchor(&vector_data) else { continue };

				// Delete point
				let modification_type = VectorModificationType::RemovePoint { id: point };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });

				// Delete connected segments
				for HandleId { segment, .. } in vector_data.all_connected(point) {
					let modification_type = VectorModificationType::RemoveSegment { id: segment };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}
	}

	/// Disable colinear handles colinear.
	pub fn disable_colinear_handles_state_on_selected(&self, network_interface: &NodeNetworkInterface, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(vector_data) = network_interface.compute_modified_vector(layer) else {
				continue;
			};

			for &point in &state.selected_points {
				if let ManipulatorPointId::Anchor(point) = point {
					for connected in vector_data.all_connected(point) {
						if let Some(&handles) = vector_data.colinear_manipulators.iter().find(|target| target.iter().any(|&target| target == connected)) {
							let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}
				} else if let Some(handles) = point.get_handle_pair(&vector_data) {
					let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}
	}

	/// Find a [ManipulatorPoint] that is within the selection threshold and return the layer path, an index to the [ManipulatorGroup], and an enum index for [ManipulatorPoint].
	pub fn find_nearest_point_indices(&mut self, network_interface: &NodeNetworkInterface, mouse_position: DVec2, select_threshold: f64) -> Option<(LayerNodeIdentifier, ManipulatorPointId)> {
		if self.selected_shape_state.is_empty() {
			return None;
		}

		let select_threshold_squared = select_threshold * select_threshold;

		// Find the closest control point among all elements of shapes_to_modify
		for &layer in self.selected_shape_state.keys() {
			if let Some((manipulator_point_id, distance_squared)) = Self::closest_point_in_layer(network_interface, layer, mouse_position) {
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
	fn closest_point_in_layer(network_interface: &NodeNetworkInterface, layer: LayerNodeIdentifier, pos: glam::DVec2) -> Option<(ManipulatorPointId, f64)> {
		let mut closest_distance_squared: f64 = f64::MAX;
		let mut manipulator_point = None;

		let vector_data = network_interface.compute_modified_vector(layer)?;
		let viewspace = network_interface.document_metadata().transform_to_viewport(layer);

		// Handles
		for (segment_id, bezier, _, _) in vector_data.segment_bezier_iter() {
			let bezier = bezier.apply_transformation(|point| viewspace.transform_point2(point));
			let valid = |handle: DVec2, control: DVec2| handle.distance_squared(control) > crate::consts::HIDE_HANDLE_DISTANCE.powi(2);

			if let Some(primary_handle) = bezier.handle_start() {
				if valid(primary_handle, bezier.start) && (bezier.handle_end().is_some() || valid(primary_handle, bezier.end)) && primary_handle.distance_squared(pos) <= closest_distance_squared {
					closest_distance_squared = primary_handle.distance_squared(pos);
					manipulator_point = Some(ManipulatorPointId::PrimaryHandle(segment_id));
				}
			}
			if let Some(end_handle) = bezier.handle_end() {
				if valid(end_handle, bezier.end) && end_handle.distance_squared(pos) <= closest_distance_squared {
					closest_distance_squared = end_handle.distance_squared(pos);
					manipulator_point = Some(ManipulatorPointId::EndHandle(segment_id));
				}
			}
		}

		// Anchors
		for (&id, &point) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
			let point = viewspace.transform_point2(point);

			if point.distance_squared(pos) <= closest_distance_squared {
				closest_distance_squared = point.distance_squared(pos);
				manipulator_point = Some(ManipulatorPointId::Anchor(id));
			}
		}

		manipulator_point.map(|id| (id, closest_distance_squared))
	}

	/// Find the `t` value along the path segment we have clicked upon, together with that segment ID.
	fn closest_segment(&self, network_interface: &NodeNetworkInterface, layer: LayerNodeIdentifier, position: glam::DVec2, tolerance: f64) -> Option<ClosestSegment> {
		let transform = network_interface.document_metadata().transform_to_viewport(layer);
		let layer_pos = transform.inverse().transform_point2(position);

		let tolerance = tolerance + 0.5;

		let mut closest = None;
		let mut closest_distance_squared: f64 = tolerance * tolerance;

		let vector_data = network_interface.compute_modified_vector(layer)?;

		for (segment, mut bezier, start, end) in vector_data.segment_bezier_iter() {
			let t = bezier.project(layer_pos);
			let layerspace = bezier.evaluate(TValue::Parametric(t));

			let screenspace = transform.transform_point2(layerspace);
			let distance_squared = screenspace.distance_squared(position);

			if distance_squared < closest_distance_squared {
				closest_distance_squared = distance_squared;

				// 0.5 is half the line (center to side) but it's convenient to allow targeting slightly more than half the line width
				const STROKE_WIDTH_PERCENT: f64 = 0.7;

				let stroke_width = graph_modification_utils::get_stroke_width(layer, network_interface).unwrap_or(1.) as f64 * STROKE_WIDTH_PERCENT;

				// Convert to linear if handes are on top of control points
				if let bezier_rs::BezierHandles::Cubic { handle_start, handle_end } = bezier.handles {
					if handle_start.abs_diff_eq(bezier.start(), f64::EPSILON * 100.) && handle_end.abs_diff_eq(bezier.end(), f64::EPSILON * 100.) {
						bezier = Bezier::from_linear_dvec2(bezier.start, bezier.end);
					}
				}

				let primary_handle = vector_data.colinear_manipulators.iter().find(|handles| handles.contains(&HandleId::primary(segment)));
				let end_handle = vector_data.colinear_manipulators.iter().find(|handles| handles.contains(&HandleId::end(segment)));
				let primary_handle = primary_handle.and_then(|&handles| handles.into_iter().find(|handle| handle.segment != segment));
				let end_handle = end_handle.and_then(|&handles| handles.into_iter().find(|handle| handle.segment != segment));

				closest = Some(ClosestSegment {
					segment,
					bezier,
					points: [start, end],
					colinear: [primary_handle, end_handle],
					t,
					bezier_point_to_viewport: screenspace,
					layer,
					stroke_width,
				});
			}
		}

		closest
	}

	/// find closest to the position segment on selected layers. If there is more than one layers with close enough segment it return upper from them
	pub fn upper_closest_segment(&self, network_interface: &NodeNetworkInterface, position: glam::DVec2, tolerance: f64) -> Option<ClosestSegment> {
		let closest_seg = |layer| self.closest_segment(network_interface, layer, position, tolerance);
		match self.selected_shape_state.len() {
			0 => None,
			1 => self.selected_layers().next().copied().and_then(closest_seg),
			_ => self.sorted_selected_layers(network_interface.document_metadata()).find_map(closest_seg),
		}
	}

	/// Selects handles and anchor connected to current handle
	pub fn select_handles_and_anchor(&mut self, network_interface: &NodeNetworkInterface) {
		let mut anchor_and_handles_to_select = Vec::new();
		for (&layer, _) in &self.selected_shape_state {
			let Some(vector_data) = network_interface.compute_modified_vector(layer) else {
				continue;
			};

			for point in self.selected_points() {
				if let Some(handles) = point.get_handle_pair(&vector_data) {
					let anchor = handles[0].to_manipulator_point().get_anchor(&vector_data);
					//handle[0] is selected, handle[1] is other
					anchor_and_handles_to_select.push((layer, handles[1].to_manipulator_point(), anchor));
				}
			}
		}

		for (layer, handle, anchor) in anchor_and_handles_to_select {
			if let Some(state) = self.selected_shape_state.get_mut(&layer) {
				state.select_point(handle);
				match anchor {
					Some(anchor) => state.select_point(ManipulatorPointId::Anchor(anchor)),
					None => continue,
				}
			}
		}
	}

	pub fn select_points_by_manipulator_id(&mut self, points: &Vec<ManipulatorPointId>) {
		// Collect layers to modify first
		let layers_to_modify: Vec<_> = self.selected_shape_state.keys().cloned().collect();

		// Now perform the mutations on collected layers
		for layer in layers_to_modify {
			if let Some(state) = self.selected_shape_state.get_mut(&layer) {
				for point in points {
					state.select_point(*point);
				}
			}
		}
	}
	/// Converts a nearby clicked anchor point's handles between sharp (zero-length handles) and smooth (pulled-apart handle(s)).
	/// If both handles aren't zero-length, they are set that. If both are zero-length, they are stretched apart by a reasonable amount.
	/// This can can be activated by double clicking on an anchor with the Path tool.
	pub fn flip_smooth_sharp(&self, network_interface: &NodeNetworkInterface, target: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) -> bool {
		let mut process_layer = |layer| {
			let vector_data = network_interface.compute_modified_vector(layer)?;
			let transform_to_screenspace = network_interface.document_metadata().transform_to_viewport(layer);

			let mut result = None;
			let mut closest_distance_squared = tolerance * tolerance;

			// Find the closest anchor point on the current layer
			for (&id, &anchor) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
				let screenspace = transform_to_screenspace.transform_point2(anchor);
				let distance_squared = screenspace.distance_squared(target);

				if distance_squared < closest_distance_squared {
					closest_distance_squared = distance_squared;
					result = Some((id, anchor));
				}
			}

			let (id, anchor) = result?;
			let handles = vector_data.all_connected(id);
			let mut positions = handles
				.filter_map(|handle| handle.to_manipulator_point().get_position(&vector_data))
				.filter(|&handle| !anchor.abs_diff_eq(handle, 1e-5));

			// Check by comparing the handle positions to the anchor if this manipulator group is a point
			let already_sharp = positions.next().is_none();
			if already_sharp {
				self.convert_manipulator_handles_to_colinear(&vector_data, id, responses, layer);
			} else {
				for handle in vector_data.all_connected(id) {
					let Some(bezier) = vector_data.segment_from_id(handle.segment) else { continue };

					match bezier.handles {
						BezierHandles::Linear => {}
						BezierHandles::Quadratic { .. } => {
							let segment = handle.segment;
							// Convert to linear
							let modification_type = VectorModificationType::SetHandles { segment, handles: [None; 2] };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });

							// Set the manipulator to have non-colinear handles
							for &handles in &vector_data.colinear_manipulators {
								if handles.contains(&HandleId::primary(segment)) {
									let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
									responses.add(GraphOperationMessage::Vector { layer, modification_type });
								}
							}
						}
						BezierHandles::Cubic { .. } => {
							// Set handle position to anchor position
							let modification_type = handle.set_relative_position(DVec2::ZERO);
							responses.add(GraphOperationMessage::Vector { layer, modification_type });

							// Set the manipulator to have non-colinear handles
							for &handles in &vector_data.colinear_manipulators {
								if handles.contains(&handle) {
									let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
									responses.add(GraphOperationMessage::Vector { layer, modification_type });
								}
							}
						}
					}
				}
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

	pub fn select_all_in_quad(&mut self, network_interface: &NodeNetworkInterface, quad: [DVec2; 2], clear_selection: bool) {
		for (&layer, state) in &mut self.selected_shape_state {
			if clear_selection {
				state.clear_points()
			}

			let vector_data = network_interface.compute_modified_vector(layer);
			let Some(vector_data) = vector_data else { continue };
			let transform = network_interface.document_metadata().transform_to_viewport(layer);

			assert_eq!(vector_data.segment_domain.ids().len(), vector_data.start_point().count());
			assert_eq!(vector_data.segment_domain.ids().len(), vector_data.end_point().count());
			for start in vector_data.start_point() {
				assert!(vector_data.point_domain.ids().contains(&start));
			}
			for end in vector_data.end_point() {
				assert!(vector_data.point_domain.ids().contains(&end));
			}

			for (id, bezier, _, _) in vector_data.segment_bezier_iter() {
				for (position, id) in [(bezier.handle_start(), ManipulatorPointId::PrimaryHandle(id)), (bezier.handle_end(), ManipulatorPointId::EndHandle(id))] {
					let Some(position) = position else { continue };
					let transformed_position = transform.transform_point2(position);

					if quad[0].min(quad[1]).cmple(transformed_position).all() && quad[0].max(quad[1]).cmpge(transformed_position).all() {
						state.select_point(id);
					}
				}
			}

			for (&id, &position) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
				let transformed_position = transform.transform_point2(position);

				if quad[0].min(quad[1]).cmple(transformed_position).all() && quad[0].max(quad[1]).cmpge(transformed_position).all() {
					state.select_point(ManipulatorPointId::Anchor(id));
				}
			}
		}
	}
}
