use std::ops::ControlFlow;

use super::graph_modification_utils::{self, get_colinear_manipulators};
use super::snapping::{are_manipulator_handles_colinear, SnapCandidatePoint, SnapData, SnapManager, SnappedPoint};
use crate::consts::{DRAG_THRESHOLD, INSERT_POINT_ON_SEGMENT_TOO_CLOSE_DISTANCE};
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{GeometrySnapSource, SnapSource};
use crate::messages::prelude::*;

use bezier_rs::{Bezier, ManipulatorGroup, TValue};
use graph_craft::document::NodeNetwork;
use graphene_core::transform::Transform;
use graphene_core::vector::{ManipulatorPointId, PointId, SelectedType, VectorData, VectorModificationType};

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

pub type OpposingHandleLengths = HashMap<LayerNodeIdentifier, HashMap<PointId, Option<f64>>>;

struct ClosestSegmentInfo {
	pub segment: SegmentId,
	pub bezier: Bezier,
	pub t: f64,
}

pub struct ClosestSegment {
	layer: LayerNodeIdentifier,
	segment: SegmentId,
	bezier: Bezier,
	points: [PointId; 2],
	t: f64,
	bezier_point_to_viewport: DVec2,
	stroke_width: f64,
}

impl ClosestSegment {
	fn new(segment: SegmentId, mut bezier: Bezier, points: [PointId; 2], t: f64, bezier_point_to_viewport: DVec2, layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Self {
		// 0.5 is half the line (center to side) but it's convenient to allow targetting slightly more than half the line width
		const STROKE_WIDTH_PERCENT: f64 = 0.7;

		let stroke_width = graph_modification_utils::get_stroke_width(layer, document_network).unwrap_or(1.) as f64 * STROKE_WIDTH_PERCENT;

		// Convert to linear if handes are on top of control points
		if let bezier_rs::BezierHandles::Cubic { handle_start, handle_end } = bezier.handles {
			if handle_start.abs_diff_eq(bezier.start(), f64::EPSILON * 100.) && handle_end.abs_diff_eq(bezier.end(), f64::EPSILON * 100.) {
				bezier = Bezier::from_linear_dvec2(bezier.start, bezier.end);
			}
		}

		Self {
			layer,
			segment,
			bezier,
			points,
			t,
			bezier_point_to_viewport,
			stroke_width,
		}
	}

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

		let t = self.bezier.project(layer_mouse_pos).max(0.).min(1.);
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
		let modification_type = VectorModificationType::InsertPoint { id: midpoint, pos: first.end };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// First segment
		let segment_ids = [SegmentId::generate(), SegmentId::generate()];
		let modification_type = VectorModificationType::InsertSegment {
			id: segment_ids[0],
			start: self.points[0],
			end: midpoint,
			handles: first.handles,
		};
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Last segment
		let modification_type = VectorModificationType::InsertSegment {
			id: segment_ids[1],
			start: midpoint,
			end: self.points[1],
			handles: second.handles,
		};
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// G1 continous
		if self.bezier.handle_end().is_some() {
			let handles = [HandleId::end(segment_ids[0]), HandleId::primary(segment_ids[1])];
			let modification_type = VectorModificationType::SetG1Continous { handles, enabled: true };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}

		// Remove old segment
		let modification_type = VectorModificationType::RemoveSegment { id: self.segment };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

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
	pub fn snap(&self, snap_manager: &mut SnapManager, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, previous_mouse: DVec2) -> DVec2 {
		let mut snap_data = SnapData::new(document, input);

		for (layer, state) in &self.selected_shape_state {
			for point in &state.selected_points {
				// snap_data.manipulators.push((*layer, point.group));
			}
		}

		let mouse_delta = document.metadata.document_to_viewport.inverse().transform_vector2(input.mouse.position - previous_mouse);
		let mut offset = mouse_delta;
		let mut best_snapped = SnappedPoint::infinite_snap(document.metadata.document_to_viewport.inverse().transform_point2(input.mouse.position));
		for (layer, state) in &self.selected_shape_state {
			let Some(vector_data) = document.metadata.compute_modified_vector(*layer, &document.network) else {
				continue;
			};

			let to_document = document.metadata.transform_to_document(*layer);

			// for subpath in vector_data.stroke_bezier_paths() {
			// 	for (index, group) in subpath.manipulator_groups().iter().enumerate() {
			// 		for handle in [SelectedType::Anchor, SelectedType::InHandle, SelectedType::OutHandle] {
			// 			if !state.is_selected(ManipulatorPointId::new(group.id.into(), handle)) {
			// 				continue;
			// 			}
			// 			let source = if handle.is_handle() {
			// 				SnapSource::Geometry(GeometrySnapSource::Handle)
			// 			} else if are_manipulator_handles_colinear(group, to_document, &subpath, index) {
			// 				SnapSource::Geometry(GeometrySnapSource::AnchorWithColinearHandles)
			// 			} else {
			// 				SnapSource::Geometry(GeometrySnapSource::AnchorWithFreeHandles)
			// 			};
			// 			let Some(position) = handle.get_position(group) else { continue };
			// 			let mut point = SnapCandidatePoint::new_source(to_document.transform_point2(position) + mouse_delta, source);

			// 			let mut push_neighbor = |group: ManipulatorGroup<PointId>| {
			// 				if !state.is_selected(ManipulatorPointId::new(group.id.into(), SelectedType::Anchor)) {
			// 					point.neighbors.push(to_document.transform_point2(group.anchor));
			// 				}
			// 			};
			// 			if handle == SelectedType::Anchor {
			// 				// Previous anchor (looping if closed)
			// 				if index > 0 {
			// 					push_neighbor(subpath.manipulator_groups()[index - 1]);
			// 				} else if subpath.closed() {
			// 					push_neighbor(subpath.manipulator_groups()[subpath.len() - 1]);
			// 				}
			// 				// Next anchor (looping if closed)
			// 				if index + 1 < subpath.len() {
			// 					push_neighbor(subpath.manipulator_groups()[index + 1]);
			// 				} else if subpath.closed() {
			// 					push_neighbor(subpath.manipulator_groups()[0]);
			// 				}
			// 			}

			// 			let snapped = snap_manager.free_snap(&snap_data, &point, None, false);
			// 			if best_snapped.other_snap_better(&snapped) {
			// 				offset = snapped.snapped_point_document - point.document_point + mouse_delta;
			// 				best_snapped = snapped;
			// 			}
			// 		}
			// 	}
			// }
		}
		snap_manager.update_indicator(best_snapped);
		document.metadata.document_to_viewport.transform_vector2(offset)
	}

	/// Select/deselect the first point within the selection threshold.
	/// Returns a tuple of the points if found and the offset, or `None` otherwise.
	pub fn change_point_selection(
		&mut self,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		mouse_position: DVec2,
		select_threshold: f64,
		add_to_selection: bool,
	) -> Option<Option<SelectedPointsInfo>> {
		if self.selected_shape_state.is_empty() {
			return None;
		}

		if let Some((layer, manipulator_point_id)) = self.find_nearest_point_indices(document_network, document_metadata, mouse_position, select_threshold) {
			let vector_data = document_metadata.compute_modified_vector(layer, document_network)?;
			let point_position = manipulator_point_id.get_position(&vector_data)?;

			let selected_shape_state = self.selected_shape_state.get(&layer)?;
			let already_selected = selected_shape_state.is_selected(manipulator_point_id);

			// Should we select or deselect the point?
			let new_selected = if already_selected { !add_to_selection } else { true };

			// Offset to snap the selected point to the cursor
			let offset = mouse_position - document_metadata.transform_to_viewport(layer).transform_point2(point_position);

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

				return Some(Some(SelectedPointsInfo { points, offset }));
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

	/// Selects all anchors, and deselects all handles, for the given layer.
	pub fn select_all_anchors_in_layer(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier) {
		let Some(state) = self.selected_shape_state.get_mut(&layer) else { return };
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
		let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
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

	pub fn move_anchor(&self, point: PointId, vector_data: &VectorData, delta: DVec2, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: VectorModificationType::ApplyPointDelta { point, delta },
		});
		for segment in vector_data.segment_domain.start_connected(point) {
			if vector_data.segment_from_id(segment).is_some_and(|bezier| bezier.handle_start().is_some()) {
				self.move_primary(segment, delta, layer, responses);
			}
		}
		for segment in vector_data.segment_domain.end_connected(point) {
			let Some(bezier) = vector_data.segment_from_id(segment) else { continue };
			if bezier.handle_end().is_some() {
				self.move_end(segment, delta, layer, responses);
			} else if bezier.handle_start().is_some() {
				self.move_primary(segment, delta, layer, responses);
			}
		}
	}

	/// Moves a control point to a `new_position` in document space.
	/// Returns `Some(())` if successful and `None` otherwise.
	pub fn reposition_control_point(
		&self,
		point: &ManipulatorPointId,
		network: &NodeNetwork,
		metadata: &DocumentMetadata,
		new_position: DVec2,
		layer: LayerNodeIdentifier,
		responses: &mut VecDeque<Message>,
	) -> Option<()> {
		let vector_data = metadata.compute_modified_vector(layer, network)?;
		let transform = metadata.transform_to_document(layer).inverse();
		let position = transform.transform_point2(new_position);
		let current_position = point.get_position(&vector_data)?;
		let delta = position - current_position;

		match *point {
			ManipulatorPointId::Anchor(point) => self.move_anchor(point, &vector_data, delta, layer, responses),
			ManipulatorPointId::PrimaryHandle(segment) => {
				self.move_primary(segment, delta, layer, responses);
				if let Some(handles) = point.get_handle_pair(&vector_data) {
					let modification_type = VectorModificationType::SetG1Continous { handles, enabled: false };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
			ManipulatorPointId::EndHandle(segment) => {
				self.move_end(segment, delta, layer, responses);
				if let Some(handles) = point.get_handle_pair(&vector_data) {
					let modification_type = VectorModificationType::SetG1Continous { handles, enabled: false };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}

		Some(())
	}

	/// Iterates over the selected manipulator groups, returning whether their handles have mixed, colinear, or free angles.
	/// If there are no points selected this function returns mixed.
	pub fn selected_manipulator_angles(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata) -> ManipulatorAngle {
		// This iterator contains a bool indicating whether or not selected points' manipulator groups have colinear handles.
		let mut points_colinear_status = self
			.selected_shape_state
			.iter()
			.map(|(&layer, selection_state)| (document_metadata.compute_modified_vector(layer, document_network), selection_state))
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
		let Some(handles) = ManipulatorPointId::Anchor(point_id).get_handle_pair(&vector_data) else {
			return;
		};

		let handle_positions = handles.map(|handle| handle.to_point().get_position(vector_data));

		// Grab the next and previous manipulator groups by simply looking at the next / previous index
		let points = handles.map(|handle| {
			vector_data
				.segment_domain
				.segment_start_from_id(handle.segment)
				.filter(|&id| id != point_id)
				.or_else(|| vector_data.segment_domain.segment_end_from_id(handle.segment))
		});
		let anchor_positions = points.map(|point| point.and_then(|point| ManipulatorPointId::Anchor(point).get_position(vector_data)));

		// To find the length of the new tangent we just take the distance to the anchor and divide by 3 (pretty arbitrary)
		let lengths = anchor_positions.map(|position| position.map(|position| (position - anchor_position).length() / 3.));

		// Use the position relative to the anchor
		let directions = anchor_positions.map(|position| position.map(|position| (position - anchor_position)).and_then(DVec2::try_normalize));

		// The direction of the handles is either the perpendicular vector to the sum of the anchors' positions or just the anchor's position (if only one)
		let mut handle_direction = match (directions[0], directions[1]) {
			(Some(previous), Some(next)) => (previous - next).try_normalize().unwrap_or(next.perp()),
			(Some(val), None) | (None, Some(val)) => val,
			(None, None) => return,
		};

		// Set the manipulator to have colinear handles
		let modification_type = VectorModificationType::SetG1Continous { handles, enabled: true };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Flip the vector if it is not facing towards the same direction as the anchor
		if anchor_positions[0].filter(|&group| (group - anchor_position).normalize_or_zero().dot(handle_direction) < 0.).is_some()
			|| anchor_positions[1].filter(|&group| (group - anchor_position).normalize_or_zero().dot(handle_direction) > 0.).is_some()
		{
			handle_direction = -handle_direction;
		}

		// Push both in and out handles into the correct position
		if let Some(new_position) = lengths[0].map(|length| anchor_position + handle_direction * length) {
			let modification_type = handles[0].move_pos(new_position - handle_positions[0].unwrap_or_default());

			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}
		if let Some(new_position) = lengths[1].map(|length| anchor_position - handle_direction * length) {
			let modification_type = handles[1].move_pos(new_position - handle_positions[1].unwrap_or_default());
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}
	}

	/// Converts all selected points to colinear while moving the handles to ensure their 180° angle separation.
	/// If only one handle is selected, the other handle will be moved to match the angle of the selected handle.
	/// If both or neither handles are selected, the angle of both handles will be averaged from their current angles, weighted by their lengths.
	/// Assumes all selected manipulators have handles that are already not colinear.
	pub fn convert_selected_manipulators_to_colinear_handles(&self, responses: &mut VecDeque<Message>, document: &DocumentMessageHandler) -> Option<()> {
		let mut skip_set = HashSet::new();

		for (&layer, layer_state) in self.selected_shape_state.iter() {
			let vector_data = document.metadata.compute_modified_vector(layer, &document.network)?;

			for &point in layer_state.selected_points.iter() {
				let Some(handles) = point.get_handle_pair(&vector_data) else { continue };
				if skip_set.contains(&handles) || skip_set.contains(&[handles[1], handles[0]]) {
					continue;
				};

				skip_set.insert(handles);

				let [selected0, selected1] = handles.map(|handle| layer_state.selected_points.contains(&handle.to_point()));
				let [Some(pos0), Some(pos1)] = handles.map(|handle| handle.to_point().get_position(&vector_data)) else {
					continue;
				};

				let Some(anchor) = point.get_anchor(&vector_data).and_then(|id| vector_data.point_domain.pos_from_id(id)) else {
					continue;
				};

				if (selected0 || selected1) && !(selected0 && selected1) {
					// If one handle is selected, only move the other handle
					let [(selected_handle, selected_position), (unselected_handle, unselected_position)] = if selected0 {
						[(handles[0], pos0), (handles[1], pos1)]
					} else {
						[(handles[1], pos1), (handles[0], pos0)]
					};
					let position = anchor * 2. - selected_position;
					let modification_type = unselected_handle.move_pos(position - unselected_position);
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				} else {
					// If both handles are selected, average the angles of the handles
					// We could normalise these directions?
					let mut handle0_direction = pos0 - anchor;
					let mut handle1_direction = pos1 - anchor;
					// Prevent division by zero if handles are on top of each other
					if !(handle0_direction - handle1_direction).length_squared().recip().is_finite() {
						handle0_direction = handle0_direction.perp();
						handle1_direction = -handle0_direction;
					}
					let new0 = anchor + handle0_direction.project_onto(handle0_direction - handle1_direction);
					let new1 = anchor + handle1_direction.project_onto(handle1_direction - handle0_direction);
					let modification_type = handles[0].move_pos(new0 - pos0);
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
					let modification_type = handles[1].move_pos(new1 - pos1);
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}

		Some(())
	}

	/// Move the selected points by dragging the mouse.
	pub fn move_selected_points(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, delta: DVec2, equidistant: bool, responses: &mut VecDeque<Message>) {
		// for (&layer, state) in &self.selected_shape_state {
		// 	let Some(vector_data) = document_metadata.compute_modified_vector(layer, document_network) else {
		// 		continue;
		// 	};
		// 	let colinear_manipulators = get_colinear_manipulators(layer, document_network);

		// 	let transform = document_metadata.transform_to_viewport(layer);
		// 	let delta = transform.inverse().transform_vector2(delta);

		// 	for &point in state.selected_points.iter() {
		// 		if point.manipulator_type.is_handle() && state.is_selected(ManipulatorPointId::new(point.group, SelectedType::Anchor)) {
		// 			continue;
		// 		}

		// 		let Some(group) = vector_data.manipulator_group_id(point.group) else { continue };

		// 		let mut move_point = |point: ManipulatorPointId| {
		// 			// let Some(previous_position) = point.manipulator_type.get_position(&group) else { return };
		// 			// let position = previous_position + delta;
		// 			// responses.add(GraphOperationMessage::Vector {
		// 			// 	layer,
		// 			// 	modification_type: VectorModificationType::SetManipulatorPosition { point, position },
		// 			// });
		// 		};

		// 		move_point(point);

		// 		// if point.manipulator_type == SelectedType::Anchor {
		// 		// 	move_point(ManipulatorPointId::new(point.group, SelectedType::InHandle));
		// 		// 	move_point(ManipulatorPointId::new(point.group, SelectedType::OutHandle));
		// 		// }

		// 		if equidistant && point.manipulator_type != SelectedType::Anchor {
		// 			let mut colinear = colinear_manipulators.contains(&point.group);

		// 			// If there is no opposing handle, we consider it colinear
		// 			// if !colinear && point.manipulator_type.opposite().get_position(&group).is_none() {
		// 			// 	responses.add(GraphOperationMessage::Vector {
		// 			// 		layer,
		// 			// 		modification_type: VectorModificationType::SetManipulatorColinearHandlesState { id: group.id, colinear: true },
		// 			// 	});
		// 			// 	colinear = true;
		// 			// }

		// 			// if colinear {
		// 			// 	let Some(mut original_handle_position) = point.manipulator_type.get_position(&group) else {
		// 			// 		continue;
		// 			// 	};
		// 			// 	original_handle_position += delta;

		// 			// 	let point = ManipulatorPointId::new(point.group, point.manipulator_type.opposite());
		// 			// 	if state.is_selected(point) {
		// 			// 		continue;
		// 			// 	}
		// 			// 	let position = group.anchor - (original_handle_position - group.anchor);
		// 			// 	responses.add(GraphOperationMessage::Vector {
		// 			// 		layer,
		// 			// 		modification_type: VectorModificationType::SetManipulatorPosition { point, position },
		// 			// 	});
		// 			// }
		// 		}
		// 	}
		// }
	}

	/// Delete selected and colinear handles with zero length when the drag stops.
	pub fn delete_selected_handles_with_zero_length(
		&self,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		opposing_handle_lengths: &Option<OpposingHandleLengths>,
		responses: &mut VecDeque<Message>,
	) {
		// for (&layer, state) in &self.selected_shape_state {
		// 	let Some(vector_data) = document_metadata.compute_modified_vector(layer, document_network) else {
		// 		continue;
		// 	};
		// 	let colinear_manipulators = get_colinear_manipulators(layer, document_network);

		// 	let opposing_handle_lengths = opposing_handle_lengths.as_ref().and_then(|lengths| lengths.get(&layer));

		// 	let transform = document_metadata.transform_to_viewport(layer);

		// 	for &point in state.selected_points.iter() {
		// 		let anchor = ManipulatorPointId::new(point.group, SelectedType::Anchor);
		// 		if !point.manipulator_type.is_handle() || state.is_selected(anchor) {
		// 			continue;
		// 		}

		// 		let Some(group) = vector_data.manipulator_group_id(point.group) else { continue };

		// 		let anchor_position = transform.transform_point2(group.anchor);

		// 		let point_position = if let Some(position) = point.manipulator_type.get_position(&group) {
		// 			transform.transform_point2(position)
		// 		} else {
		// 			continue;
		// 		};

		// 		if (anchor_position - point_position).length() < DRAG_THRESHOLD {
		// 			responses.add(GraphOperationMessage::Vector {
		// 				layer,
		// 				modification_type: VectorModificationType::RemoveManipulatorPoint { point },
		// 			});

		// 			// Remove opposing handle if it is not selected and is colinear.
		// 			let opposite_point = ManipulatorPointId::new(point.group, point.manipulator_type.opposite());
		// 			if !state.is_selected(opposite_point) && colinear_manipulators.contains(&point.group) {
		// 				if let Some(lengths) = opposing_handle_lengths {
		// 					if lengths.contains_key(&point.group) {
		// 						responses.add(GraphOperationMessage::Vector {
		// 							layer,
		// 							modification_type: VectorModificationType::RemoveManipulatorPoint { point: opposite_point },
		// 						});
		// 					}
		// 				}
		// 			}
		// 		}
		// 	}
		// }
	}

	/// The opposing handle lengths.
	pub fn opposing_handle_lengths(&self, document: &DocumentMessageHandler) -> OpposingHandleLengths {
		// self.selected_shape_state
		// 	.iter()
		// 	.filter_map(|(&layer, state)| {
		// 		let vector_data = document.metadata.compute_modified_vector(layer, &document.network)?;
		// 		let opposing_handle_lengths = vector_data
		// 			.manipulator_groups()
		// 			.filter_map(|manipulator_group| {
		// 				// We will keep track of the opposing handle length when:
		// 				// i) Exactly one handle is selected.
		// 				// ii) The anchor is not selected.

		// 				let in_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::InHandle));
		// 				let out_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::OutHandle));
		// 				let anchor_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::Anchor));

		// 				if anchor_selected {
		// 					return None;
		// 				}

		// 				let single_selected_handle = match (in_handle_selected, out_handle_selected) {
		// 					(true, false) => SelectedType::InHandle,
		// 					(false, true) => SelectedType::OutHandle,
		// 					_ => return None,
		// 				};

		// 				let Some(opposing_handle_position) = single_selected_handle.opposite().get_position(&manipulator_group) else {
		// 					return Some((manipulator_group.id, None));
		// 				};

		// 				let opposing_handle_length = opposing_handle_position.distance(manipulator_group.anchor);
		// 				Some((manipulator_group.id, Some(opposing_handle_length)))
		// 			})
		// 			.collect::<HashMap<_, _>>();
		// 		Some((layer, opposing_handle_lengths))
		// 	})
		// 	.collect::<HashMap<_, _>>()
		HashMap::new()
	}

	/// Reset the opposing handle lengths.
	pub fn reset_opposing_handle_lengths(&self, document: &DocumentMessageHandler, opposing_handle_lengths: &OpposingHandleLengths, responses: &mut VecDeque<Message>) {
		// for (&layer, state) in &self.selected_shape_state {
		// 	let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
		// 		continue;
		// 	};
		// 	let colinear_manipulators = get_colinear_manipulators(layer, &document.network);
		// 	let Some(opposing_handle_lengths) = opposing_handle_lengths.get(&layer) else { continue };

		// 	for manipulator_group in vector_data.manipulator_groups() {
		// 		if !colinear_manipulators.contains(&manipulator_group.id) {
		// 			continue;
		// 		}

		// 		let Some(opposing_handle_length) = opposing_handle_lengths.get(&manipulator_group.id) else {
		// 			continue;
		// 		};

		// 		let in_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::InHandle));
		// 		let out_handle_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::OutHandle));
		// 		let anchor_selected = state.is_selected(ManipulatorPointId::new(manipulator_group.id, SelectedType::Anchor));

		// 		if anchor_selected {
		// 			continue;
		// 		}

		// 		let single_selected_handle = match (in_handle_selected, out_handle_selected) {
		// 			(true, false) => SelectedType::InHandle,
		// 			(false, true) => SelectedType::OutHandle,
		// 			_ => continue,
		// 		};

		// 		let Some(opposing_handle_length) = opposing_handle_length else {
		// 			responses.add(GraphOperationMessage::Vector {
		// 				layer,
		// 				modification_type: VectorModificationType::RemoveManipulatorPoint {
		// 					point: ManipulatorPointId::new(manipulator_group.id, single_selected_handle.opposite()),
		// 				},
		// 			});
		// 			continue;
		// 		};

		// 		let Some(opposing_handle) = single_selected_handle.opposite().get_position(&manipulator_group) else {
		// 			continue;
		// 		};

		// 		let Some(offset) = (opposing_handle - manipulator_group.anchor).try_normalize() else { continue };

		// 		let point = ManipulatorPointId::new(manipulator_group.id, single_selected_handle.opposite());
		// 		let position = manipulator_group.anchor + offset * (*opposing_handle_length);
		// 		assert!(position.is_finite(), "Opposing handle not finite!");

		// 		responses.add(GraphOperationMessage::Vector {
		// 			layer,
		// 			modification_type: VectorModificationType::SetManipulatorPosition { point, position },
		// 		});
		// 	}
		// }
	}

	fn disolve_anchor(anchor: PointId, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, vector_data: &VectorData) {
		// Delete point
		let modification_type = VectorModificationType::RemovePoint { id: anchor };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Delete connected segments
		for HandleId { segment, .. } in vector_data.segment_domain.all_connected(anchor) {
			let modification_type = VectorModificationType::RemoveSegment { id: segment };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}

		// Add in new segment if possible
		if let Some(handles) = ManipulatorPointId::Anchor(anchor).get_handle_pair(vector_data) {
			let opposites = handles.map(|handle| handle.opposite());
			let [Some(start), Some(end)] = opposites.map(|opposite| opposite.to_point().get_anchor(vector_data)) else {
				return;
			};
			let [Some(handle_start), Some(handle_end)] = opposites.map(|handle| {
				handle
					.to_point()
					.get_position(vector_data)
					.or_else(|| handle.to_point().get_anchor(vector_data).and_then(|anchor| vector_data.point_domain.pos_from_id(anchor)))
			}) else {
				return;
			};
			let modification_type = VectorModificationType::InsertSegment {
				id: opposites[0].segment,
				start,
				end,
				handles: bezier_rs::BezierHandles::Cubic { handle_start, handle_end },
			};
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}
	}

	/// Dissolve the selected points.
	pub fn delete_selected_points(&self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
				continue;
			};
			for &point in &state.selected_points {
				match point {
					ManipulatorPointId::Anchor(anchor) => {
						Self::disolve_anchor(anchor, responses, layer, &vector_data);
					}

					ManipulatorPointId::PrimaryHandle(_) | ManipulatorPointId::EndHandle(_) => {
						let Some(handle) = point.as_handle() else { continue };
						let Some(handle_position) = point.get_position(&vector_data) else { continue };
						let Some(anchor) = point.get_anchor(&vector_data) else { continue };
						let Some(anchor_position) = vector_data.point_domain.pos_from_id(anchor) else { continue };

						// Place the handle on top of the anchor
						let modification_type = handle.move_pos(anchor_position - handle_position);
						responses.add(GraphOperationMessage::Vector { layer, modification_type });

						// Disable the g1 continous
						for &handles in &vector_data.colinear_manipulators {
							if handles.contains(&handle) {
								let modification_type = VectorModificationType::SetG1Continous { handles, enabled: false };
								responses.add(GraphOperationMessage::Vector { layer, modification_type });
							}
						}
					}
				}
			}
		}
	}

	pub fn break_path_at_selected_point(&self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
				continue;
			};
			for &delete in &state.selected_points {
				let Some(point) = delete.get_anchor(&vector_data) else { continue };
				let Some(pos) = vector_data.point_domain.pos_from_id(point) else { continue };

				let mut used_initial_point = false;
				for handle in vector_data.segment_domain.all_connected(point) {
					// Disable the g1 continous
					for &handles in &vector_data.colinear_manipulators {
						if handles.contains(&handle) {
							let modification_type = VectorModificationType::SetG1Continous { handles, enabled: false };
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
					let modification_type = VectorModificationType::InsertPoint { id, pos };
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
	pub fn delete_point_and_break_path(&self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
				continue;
			};

			for &delete in &state.selected_points {
				let Some(point) = delete.get_anchor(&vector_data) else { continue };

				// Delete point
				let modification_type = VectorModificationType::RemovePoint { id: point };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });

				// Delete connectedsegments
				for HandleId { segment, .. } in vector_data.segment_domain.all_connected(point) {
					let modification_type = VectorModificationType::RemoveSegment { id: segment };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}
	}

	/// Toggle if the handles of the selected points should be colinear.
	pub fn toggle_colinear_handles_state_on_selected(&self, responses: &mut VecDeque<Message>) {
		// for (&layer, state) in &self.selected_shape_state {
		// 	for point in &state.selected_points {
		// 		let modification = VectorModificationType::ToggleManipulatorColinearHandlesState { id: point.group };
		// 		responses.add(GraphOperationMessage::Vector {
		// 			layer,
		// 			modification_type: modification,
		// 		})
		// 	}
		// }
	}

	/// Set whether the handles of the selected points should be colinear.
	pub fn set_colinear_handles_state_on_selected(&self, enabled: bool, metadata: &DocumentMetadata, network: &NodeNetwork, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(vector_data) = metadata.compute_modified_vector(layer, network) else { continue };
			for &point in &state.selected_points {
				let Some(handles) = point.get_handle_pair(&vector_data) else {
					continue;
				};
				let modification = VectorModificationType::SetG1Continous { handles, enabled };
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification_type: modification,
				});
			}
		}
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
		let mut manipulator_point = None;

		let vector_data = document_metadata.compute_modified_vector(layer, document_network)?;
		let viewspace = document_metadata.transform_to_viewport(layer);

		// Handles
		for (segment_id, bezier, _, _) in vector_data.segment_bezier_iter() {
			let bezier = bezier.apply_transformation(|point| viewspace.transform_point2(point));
			let valid = |handle: DVec2, control: DVec2| handle.distance_squared(control) > crate::consts::HIDE_HANDLE_DISTANCE.powi(2);

			if let Some(primary_handle) = bezier.handle_start() {
				if valid(primary_handle, bezier.start) && primary_handle.distance_squared(pos) < closest_distance_squared {
					closest_distance_squared = primary_handle.distance_squared(pos);
					manipulator_point = Some(ManipulatorPointId::PrimaryHandle(segment_id));
				}
			}
			if let Some(end_handle) = bezier.handle_end() {
				if valid(end_handle, bezier.end) && end_handle.distance_squared(pos) < closest_distance_squared {
					closest_distance_squared = end_handle.distance_squared(pos);
					manipulator_point = Some(ManipulatorPointId::EndHandle(segment_id));
				}
			}
		}

		// Anchors
		for (&id, &point) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
			let point = viewspace.transform_point2(point);

			if point.distance_squared(pos) < closest_distance_squared {
				closest_distance_squared = point.distance_squared(pos);
				manipulator_point = Some(ManipulatorPointId::Anchor(id));
			}
		}

		manipulator_point.map(|id| (id, closest_distance_squared))
	}

	/// Find the `t` value along the path segment we have clicked upon, together with that segment ID.
	fn closest_segment(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, layer: LayerNodeIdentifier, position: glam::DVec2, tolerance: f64) -> Option<ClosestSegment> {
		let transform = document_metadata.transform_to_viewport(layer);
		let layer_pos = transform.inverse().transform_point2(position);

		let scale = document_metadata.document_to_viewport.decompose_scale().x;
		let tolerance = tolerance + 0.5 * scale; // make more talerance at large scale

		let mut closest = None;
		let mut closest_distance_squared: f64 = tolerance * tolerance;

		let vector_data = document_metadata.compute_modified_vector(layer, document_network)?;

		for (segment, bezier, start, end) in vector_data.segment_bezier_iter() {
			let t = bezier.project(layer_pos);
			let layerspace = bezier.evaluate(TValue::Parametric(t));

			let screenspace = transform.transform_point2(layerspace);
			let distance_squared = screenspace.distance_squared(position);

			if distance_squared < closest_distance_squared {
				closest_distance_squared = distance_squared;

				closest = Some(ClosestSegment::new(segment, bezier, [start, end], t, screenspace, layer, document_network))
			}
		}

		closest
	}

	/// find closest to the position segment on selected layers. If there is more than one layers with close enough segment it return upper from them
	pub fn upper_closest_segment(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, position: glam::DVec2, tolerance: f64) -> Option<ClosestSegment> {
		let closest_seg = |layer| self.closest_segment(document_network, document_metadata, layer, position, tolerance);
		match self.selected_shape_state.len() {
			0 => None,
			1 => self.selected_layers().next().copied().and_then(closest_seg),
			_ => self.sorted_selected_layers(document_metadata).find_map(closest_seg),
		}
	}

	/// Converts a nearby clicked anchor point's handles between sharp (zero-length handles) and smooth (pulled-apart handle(s)).
	/// If both handles aren't zero-length, they are set that. If both are zero-length, they are stretched apart by a reasonable amount.
	/// This can can be activated by double clicking on an anchor with the Path tool.
	pub fn flip_smooth_sharp(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, target: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) -> bool {
		let mut process_layer = |layer| {
			let vector_data = document_metadata.compute_modified_vector(layer, document_network)?;

			let transform_to_screenspace = document_metadata.transform_to_viewport(layer);
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
			let handles = vector_data.segment_domain.all_connected(id);
			let mut positions = handles
				.filter_map(|handle| handle.to_point().get_position(&vector_data))
				.filter(|&handle| !anchor.abs_diff_eq(handle, 1e-5));

			// Check by comparing the handle positions to the anchor if this manipulator group is a point
			let already_sharp = positions.next().is_none();

			if already_sharp {
				self.convert_manipulator_handles_to_colinear(&vector_data, id, responses, layer);
			} else {
				for handle in vector_data.segment_domain.all_connected(id) {
					// Set handle position to anchor position
					let Some(position) = handle.to_point().get_position(&vector_data) else { continue };
					let modification_type = handle.move_pos(anchor - position);
					responses.add(GraphOperationMessage::Vector { layer, modification_type });

					// Set the manipulator to have non-colinear handles
					for &handles in &vector_data.colinear_manipulators {
						if handles.contains(&handle) {
							let modification_type = VectorModificationType::SetG1Continous { handles, enabled: false };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
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

	pub fn select_all_in_quad(&mut self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, quad: [DVec2; 2], clear_selection: bool) {
		for (&layer, state) in &mut self.selected_shape_state {
			if clear_selection {
				state.clear_points()
			}

			let Some(vector_data) = document_metadata.compute_modified_vector(layer, document_network) else {
				continue;
			};

			let transform = document_metadata.transform_to_viewport(layer);

			assert_eq!(vector_data.segment_domain.ids().len(), vector_data.segment_domain.start_point().len());
			assert_eq!(vector_data.segment_domain.ids().len(), vector_data.segment_domain.end_point().len());
			for start in vector_data.segment_domain.start_point() {
				assert!(vector_data.point_domain.ids().contains(start));
			}
			for end in vector_data.segment_domain.end_point() {
				assert!(vector_data.point_domain.ids().contains(end));
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
