use super::graph_modification_utils;
use super::snapping::{are_manipulator_handles_colinear, SnapCandidatePoint, SnapData, SnapManager, SnappedPoint};
use crate::consts::{DRAG_THRESHOLD, INSERT_POINT_ON_SEGMENT_TOO_CLOSE_DISTANCE};
use crate::messages::portfolio::document::graph_operation::utility_types::VectorDataModification;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::misc::{GeometrySnapSource, SnapSource};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{get_colinear_manipulators, get_manipulator_from_id, get_manipulator_groups, get_subpaths};

use bezier_rs::{Bezier, ManipulatorGroup, TValue};
use graph_craft::document::NodeNetwork;
use graphene_core::transform::Transform;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use glam::DVec2;

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

pub type OpposingHandleLengths = HashMap<LayerNodeIdentifier, HashMap<ManipulatorGroupId, Option<f64>>>;

struct ClosestSegmentInfo {
	pub bezier: Bezier,
	pub t: f64,
	pub bezier_point_to_viewport: DVec2,
	pub layer_scale: DVec2,
}

pub struct ClosestSegment {
	layer: LayerNodeIdentifier,
	start: ManipulatorGroupId,
	end: ManipulatorGroupId,
	bezier: Bezier,
	t: f64,
	t_min: f64,
	t_max: f64,
	scale: f64,
	stroke_width: f64,
	bezier_point_to_viewport: DVec2,
	has_start_handle: bool,
	has_end_handle: bool,
}

impl ClosestSegment {
	fn new(info: ClosestSegmentInfo, layer: LayerNodeIdentifier, document_network: &NodeNetwork, start: ManipulatorGroup<ManipulatorGroupId>, end: ManipulatorGroup<ManipulatorGroupId>) -> Self {
		// 0.5 is half the line (center to side) but it's convenient to allow targetting slightly more than half the line width
		const STROKE_WIDTH_PERCENT: f64 = 0.7;

		let bezier = info.bezier;
		let t = info.t;
		let (t_min, t_max) = ClosestSegment::t_min_max(&bezier, info.layer_scale);
		let stroke_width = graph_modification_utils::get_stroke_width(layer, document_network).unwrap_or(1.) as f64 * STROKE_WIDTH_PERCENT;
		let bezier_point_to_viewport = info.bezier_point_to_viewport;
		let has_start_handle = start.has_out_handle();
		let has_end_handle = end.has_in_handle();

		Self {
			layer,
			start: start.id,
			end: end.id,
			bezier,
			t,
			t_min,
			t_max,
			scale: 1.,
			stroke_width,
			bezier_point_to_viewport,
			has_start_handle,
			has_end_handle,
		}
	}

	pub fn layer(&self) -> LayerNodeIdentifier {
		self.layer
	}

	pub fn closest_point_to_viewport(&self) -> DVec2 {
		self.bezier_point_to_viewport
	}

	fn t_min_max(bezier: &Bezier, layer_scale: DVec2) -> (f64, f64) {
		let length = bezier.apply_transformation(|point| point * layer_scale).length(None);
		let too_close_t = (INSERT_POINT_ON_SEGMENT_TOO_CLOSE_DISTANCE / length).min(0.5);

		let t_min_euclidean = too_close_t;
		let t_max_euclidean = 1. - too_close_t;

		// We need parametric values because they are faster to calculate
		let t_min = bezier.euclidean_to_parametric(t_min_euclidean, 0.001);
		let t_max = bezier.euclidean_to_parametric(t_max_euclidean, 0.001);

		(t_min, t_max)
	}

	/// Updates this [`ClosestSegment`] with the viewport-space location of the closest point on the segment to the given mouse position.
	pub fn update_closest_point(&mut self, document_metadata: &DocumentMetadata, mouse_position: DVec2) {
		let transform = document_metadata.transform_to_viewport(self.layer);
		let layer_m_pos = transform.inverse().transform_point2(mouse_position);

		self.scale = document_metadata.document_to_viewport.decompose_scale().x.max(1.);

		// Linear approximation of parametric t-value ranges:
		let t_min = self.t_min / self.scale;
		let t_max = 1. - ((1. - self.t_max) / self.scale);
		let t = self.bezier.project(layer_m_pos).max(t_min).min(t_max);
		self.t = t;

		let bezier_point = self.bezier.evaluate(TValue::Parametric(t));
		let bezier_point = transform.transform_point2(bezier_point);
		self.bezier_point_to_viewport = bezier_point;
	}

	pub fn distance_squared(&self, mouse_position: DVec2) -> f64 {
		self.bezier_point_to_viewport.distance_squared(mouse_position)
	}

	pub fn split(&self) -> [Bezier; 2] {
		self.bezier.split(TValue::Parametric(self.t))
	}

	pub fn too_far(&self, mouse_position: DVec2, tolerance: f64) -> bool {
		let dist_sq = self.distance_squared(mouse_position);
		let stroke_width = self.scale * self.stroke_width;
		let stroke_width_sq = stroke_width * stroke_width;
		let tolerance_sq = tolerance * tolerance;
		(stroke_width_sq + tolerance_sq) < dist_sq
	}

	pub fn adjust_start_handle(&self, responses: &mut VecDeque<Message>) {
		if !self.has_start_handle {
			return;
		}

		let [first, _] = self.split();
		let point = ManipulatorPointId::new(self.start, SelectedType::OutHandle);

		// `first.handle_start()` should always be expected
		let position = first.handle_start().unwrap_or(first.start());

		let out_handle = GraphOperationMessage::Vector {
			layer: self.layer,
			modification: VectorDataModification::SetManipulatorPosition { point, position },
		};
		responses.add(out_handle);
	}

	pub fn adjust_end_handle(&self, responses: &mut VecDeque<Message>) {
		if !self.has_end_handle {
			return;
		}

		let [_, second] = self.split();
		let point = ManipulatorPointId::new(self.end, SelectedType::InHandle);

		// `second.handle_end()` should not be expected in the quadratic case
		let position = if second.handles.is_cubic() { second.handle_end() } else { second.handle_start() };
		let position = position.unwrap_or(second.end());

		let in_handle = GraphOperationMessage::Vector {
			layer: self.layer,
			modification: VectorDataModification::SetManipulatorPosition { point, position },
		};
		responses.add(in_handle);
	}

	/// Inserts the point that this [`ClosestSegment`] currently has. Returns the [`ManipulatorGroupId`] of the inserted point.
	pub fn insert_point(&self, responses: &mut VecDeque<Message>) -> ManipulatorGroupId {
		let [first, second] = self.split();

		let layer = self.layer;
		let anchor = first.end();

		// `first.handle_end()` should not be expected in the quadratic case
		let in_handle = if first.handles.is_cubic() { first.handle_end() } else { first.handle_start() };
		let out_handle = second.handle_start();
		let (in_handle, out_handle) = match (self.has_start_handle, self.has_end_handle) {
			(false, false) => (None, None),
			(false, true) => (in_handle, if second.handles.is_cubic() { out_handle } else { None }),
			(true, false) => (if first.handles.is_cubic() { in_handle } else { None }, out_handle),
			(true, true) => (in_handle, out_handle),
		};

		let manipulator_group = ManipulatorGroup::new(anchor, in_handle, out_handle);
		let modification = VectorDataModification::AddManipulatorGroup {
			manipulator_group,
			after_id: self.start,
		};
		let insert = GraphOperationMessage::Vector { layer, modification };
		responses.add(insert);

		manipulator_group.id
	}

	pub fn adjusted_insert(&self, responses: &mut VecDeque<Message>) -> ManipulatorGroupId {
		self.adjust_start_handle(responses);
		self.adjust_end_handle(responses);
		self.insert_point(responses)
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
				snap_data.manipulators.push((*layer, point.group));
			}
		}

		let mouse_delta = document.metadata.document_to_viewport.inverse().transform_vector2(input.mouse.position - previous_mouse);
		let mut offset = mouse_delta;
		let mut best_snapped = SnappedPoint::infinite_snap(document.metadata.document_to_viewport.inverse().transform_point2(input.mouse.position));
		for (layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(*layer, &document.network) else { continue };

			let to_document = document.metadata.transform_to_document(*layer);

			for subpath in subpaths {
				for (index, group) in subpath.manipulator_groups().iter().enumerate() {
					for handle in [SelectedType::Anchor, SelectedType::InHandle, SelectedType::OutHandle] {
						if !state.is_selected(ManipulatorPointId::new(group.id, handle)) {
							continue;
						}
						let source = if handle.is_handle() {
							SnapSource::Geometry(GeometrySnapSource::Handle)
						} else if are_manipulator_handles_colinear(group, to_document, subpath, index) {
							SnapSource::Geometry(GeometrySnapSource::AnchorWithColinearHandles)
						} else {
							SnapSource::Geometry(GeometrySnapSource::AnchorWithFreeHandles)
						};
						let Some(position) = handle.get_position(group) else { continue };
						let mut point = SnapCandidatePoint::new_source(to_document.transform_point2(position) + mouse_delta, source);

						let mut push_neighbor = |group: ManipulatorGroup<ManipulatorGroupId>| {
							if !state.is_selected(ManipulatorPointId::new(group.id, SelectedType::Anchor)) {
								point.neighbors.push(to_document.transform_point2(group.anchor));
							}
						};
						if handle == SelectedType::Anchor {
							// Previous anchor (looping if closed)
							if index > 0 {
								push_neighbor(subpath.manipulator_groups()[index - 1]);
							} else if subpath.closed() {
								push_neighbor(subpath.manipulator_groups()[subpath.len() - 1]);
							}
							// Next anchor (looping if closed)
							if index + 1 < subpath.len() {
								push_neighbor(subpath.manipulator_groups()[index + 1]);
							} else if subpath.closed() {
								push_neighbor(subpath.manipulator_groups()[0]);
							}
						}

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
			let subpaths = get_subpaths(layer, document_network)?;
			let manipulator_group = get_manipulator_groups(subpaths).find(|group| group.id == manipulator_point_id.group)?;
			let point_position = manipulator_point_id.manipulator_type.get_position(manipulator_group)?;

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

	pub fn select_anchor_point_by_id(&mut self, layer: LayerNodeIdentifier, id: ManipulatorGroupId, add_to_selection: bool) {
		if !add_to_selection {
			self.deselect_all_points();
		}
		let point = ManipulatorPointId::new(id, SelectedType::Anchor);
		let Some(selected_state) = self.selected_shape_state.get_mut(&layer) else { return };
		selected_state.select_point(point);
	}

	/// Selects all anchors, and deselects all handles, for the given layer.
	pub fn select_all_anchors_in_layer(&mut self, document_network: &NodeNetwork, layer: LayerNodeIdentifier) {
		let Some(state) = self.selected_shape_state.get_mut(&layer) else { return };
		Self::select_all_anchors_in_layer_with_state(document_network, layer, state);
	}

	/// Selects all anchors, and deselects all handles, for the selected layers.
	pub fn select_all_anchors_in_selected_layers(&mut self, document_network: &NodeNetwork) {
		for (&layer, state) in self.selected_shape_state.iter_mut() {
			Self::select_all_anchors_in_layer_with_state(document_network, layer, state);
		}
	}

	/// Internal helper function that selects all anchors, and deselects all handles, for a layer given its [`LayerNodeIdentifier`] and [`SelectedLayerState`].
	fn select_all_anchors_in_layer_with_state(document_network: &NodeNetwork, layer: LayerNodeIdentifier, state: &mut SelectedLayerState) {
		let Some(subpaths) = get_subpaths(layer, document_network) else { return };
		for manipulator in get_manipulator_groups(subpaths) {
			state.select_point(ManipulatorPointId::new(manipulator.id, SelectedType::Anchor));
			state.deselect_point(ManipulatorPointId::new(manipulator.id, SelectedType::InHandle));
			state.deselect_point(ManipulatorPointId::new(manipulator.id, SelectedType::OutHandle));
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

	/// A mutable iterator of all the manipulators, regardless of selection.
	pub fn manipulator_groups<'a>(&'a self, document_network: &'a NodeNetwork) -> impl Iterator<Item = &'a ManipulatorGroup<ManipulatorGroupId>> {
		self.iter(document_network).flat_map(|subpaths| get_manipulator_groups(subpaths))
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
		let transform = document_metadata.transform_to_document(layer).inverse();
		let position = transform.transform_point2(new_position);
		let group = graph_modification_utils::get_manipulator_from_id(subpaths, point.group)?;
		let delta = position - point.manipulator_type.get_position(group)?;

		if point.manipulator_type.is_handle() {
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::SetManipulatorColinearHandlesState { id: group.id, colinear: false },
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

	/// Iterates over the selected manipulator groups, returning whether their handles have mixed, colinear, or free angles.
	/// If there are no points selected this function returns mixed.
	pub fn selected_manipulator_angles(&self, document_network: &NodeNetwork) -> ManipulatorAngle {
		// This iterator contains a bool indicating whether or not selected points' manipulator groups have colinear handles.
		let mut points_colinear_status = self
			.selected_shape_state
			.iter()
			.filter_map(|(&layer, selection_state)| Some((graph_modification_utils::get_colinear_manipulators(layer, document_network)?, selection_state)))
			.flat_map(|(colinear_manipulators, selection_state)| selection_state.selected_points.iter().map(|selected_point| colinear_manipulators.contains(&selected_point.group)));

		let Some(first_is_colinear) = points_colinear_status.next() else { return ManipulatorAngle::Mixed };
		if points_colinear_status.any(|point| first_is_colinear != point) {
			return ManipulatorAngle::Mixed;
		}

		match first_is_colinear {
			false => ManipulatorAngle::Free,
			true => ManipulatorAngle::Colinear,
		}
	}

	pub fn convert_manipulator_handles_to_colinear(&self, subpath: &bezier_rs::Subpath<ManipulatorGroupId>, index: usize, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier) {
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

		// Set the manipulator to have colinear handles
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification: VectorDataModification::SetManipulatorColinearHandlesState { id: manipulator.id, colinear: true },
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

	/// Converts all selected points to colinear while moving the handles to ensure their 180° angle separation.
	/// If only one handle is selected, the other handle will be moved to match the angle of the selected handle.
	/// If both or neither handles are selected, the angle of both handles will be averaged from their current angles, weighted by their lengths.
	/// Assumes all selected manipulators have handles that are already not colinear.
	pub fn convert_selected_manipulators_to_colinear_handles(&self, responses: &mut VecDeque<Message>, document_network: &NodeNetwork) -> Option<()> {
		let mut skip_set = HashSet::new();

		for (&layer, layer_state) in self.selected_shape_state.iter() {
			let subpaths = get_subpaths(layer, document_network)?;

			for point in layer_state.selected_points.iter() {
				if skip_set.contains(&point.group) {
					continue;
				};

				skip_set.insert(point.group);

				let out_selected = layer_state.selected_points.contains(&ManipulatorPointId {
					group: point.group,
					manipulator_type: SelectedType::OutHandle,
				});
				let in_selected = layer_state.selected_points.contains(&ManipulatorPointId {
					group: point.group,
					manipulator_type: SelectedType::InHandle,
				});
				let group = graph_modification_utils::get_manipulator_from_id(subpaths, point.group)?;

				match (out_selected, in_selected) {
					// If the out handle is selected, only move the angle of the in handle
					(true, false) => {
						let out_handle = ManipulatorPointId::new(point.group, SelectedType::OutHandle);
						if let Some(position) = group.out_handle {
							responses.add(GraphOperationMessage::Vector {
								layer,
								modification: VectorDataModification::SetManipulatorPosition { point: out_handle, position },
							});
						}
					}
					// If the in handle is selected, only move the angle of the out handle
					(false, true) => {
						let in_handle = ManipulatorPointId::new(point.group, SelectedType::InHandle);
						if let Some(position) = group.in_handle {
							responses.add(GraphOperationMessage::Vector {
								layer,
								modification: VectorDataModification::SetManipulatorPosition { point: in_handle, position },
							});
						}
					}
					// If both or neither handles are selected, average the angles of the handles weighted proportional to their lengths
					// TODO: This is bugged, it doesn't successfully average the angles
					(_, _) => {
						let found = subpaths.iter().find_map(|subpath| {
							let group_slice = subpath.manipulator_groups();
							let index = group_slice.iter().position(|manipulator| manipulator.id == group.id)?;
							// TODO: try subpath closed? wrapping
							Some((subpath, index))
						});

						if let Some((subpath, index)) = found {
							self.convert_manipulator_handles_to_colinear(subpath, index, responses, layer);
						}
					}
				}
			}
		}

		Some(())
	}

	/// Move the selected points by dragging the mouse.
	pub fn move_selected_points(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, delta: DVec2, equidistant: bool, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(layer, document_network) else { continue };
			let Some(colinear_manipulators) = get_colinear_manipulators(layer, document_network) else {
				continue;
			};

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

				if equidistant && point.manipulator_type != SelectedType::Anchor {
					let mut colinear = colinear_manipulators.contains(&point.group);

					// If there is no opposing handle, we consider it colinear
					if !colinear && point.manipulator_type.opposite().get_position(group).is_none() {
						responses.add(GraphOperationMessage::Vector {
							layer,
							modification: VectorDataModification::SetManipulatorColinearHandlesState { id: group.id, colinear: true },
						});
						colinear = true;
					}

					if colinear {
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

	/// Delete selected and colinear handles with zero length when the drag stops.
	pub fn delete_selected_handles_with_zero_length(
		&self,
		document_network: &NodeNetwork,
		document_metadata: &DocumentMetadata,
		opposing_handle_lengths: &Option<OpposingHandleLengths>,
		responses: &mut VecDeque<Message>,
	) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(layer, document_network) else { continue };
			let Some(colinear_manipulators) = get_colinear_manipulators(layer, document_network) else {
				continue;
			};

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

					// Remove opposing handle if it is not selected and is colinear.
					let opposite_point = ManipulatorPointId::new(point.group, point.manipulator_type.opposite());
					if !state.is_selected(opposite_point) && colinear_manipulators.contains(&point.group) {
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
			let Some(colinear_manipulators) = get_colinear_manipulators(layer, document_network) else {
				continue;
			};
			let Some(opposing_handle_lengths) = opposing_handle_lengths.get(&layer) else { continue };

			for subpath in subpaths {
				for manipulator_group in subpath.manipulator_groups() {
					if !colinear_manipulators.contains(&manipulator_group.id) {
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

	pub fn break_path_at_selected_point(&self, document_network: &NodeNetwork, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(layer, document_network) else {
				continue;
			};

			let mut broken_subpaths = Vec::<bezier_rs::Subpath<ManipulatorGroupId>>::new();

			for subpath in subpaths {
				let mut points: Vec<_> = state
					.selected_points
					.iter()
					.filter_map(|&point| {
						let Some(manipulator_index) = subpath.manipulator_index_from_id(point.group) else {
							return None;
						};
						let Some(manipulator) = subpath.manipulator_from_id(point.group) else {
							return None;
						};
						Some((manipulator_index, manipulator))
					})
					.collect();

				if points.is_empty() {
					broken_subpaths.push(subpath.clone());
					continue;
				}

				points.sort_by(|&a, &b| match a.0 > b.0 {
					true => std::cmp::Ordering::Greater,
					false => std::cmp::Ordering::Less,
				});

				let mut last_manipulator_index = 0;
				let mut to_extend_with_last_group: Option<Vec<ManipulatorGroup<ManipulatorGroupId>>> = None;
				let mut last_manipulator_group: Option<&ManipulatorGroup<ManipulatorGroupId>> = None;
				for (i, &(manipulator_index, group)) in points.iter().enumerate() {
					if manipulator_index == 0 && !subpath.closed {
						last_manipulator_index = manipulator_index + 1;
						last_manipulator_group = Some(group);
						continue;
					}

					let mut segment = subpath.manipulator_groups()[last_manipulator_index..manipulator_index].to_vec();
					if i != 0 {
						segment.insert(0, ManipulatorGroup::new(last_manipulator_group.unwrap().anchor, None, last_manipulator_group.unwrap().out_handle));
					}

					segment.push(ManipulatorGroup::new(group.anchor, group.in_handle, None));

					if subpath.closed && i == 0 {
						to_extend_with_last_group = Some(segment);
					} else {
						broken_subpaths.push(bezier_rs::Subpath::new(segment, false));
					}

					last_manipulator_index = manipulator_index + 1;
					last_manipulator_group = Some(group);
				}

				if last_manipulator_index == subpath.len() && !subpath.closed {
					continue;
				}

				let mut final_segment = subpath.manipulator_groups()[last_manipulator_index..].to_vec();
				final_segment.insert(0, ManipulatorGroup::new(last_manipulator_group.unwrap().anchor, None, last_manipulator_group.unwrap().out_handle));

				if let Some(group) = to_extend_with_last_group {
					final_segment.extend(group);
				}

				broken_subpaths.push(bezier_rs::Subpath::new(final_segment, false));
			}

			responses.add(GraphOperationMessage::Vector {
				layer,
				modification: VectorDataModification::UpdateSubpaths { subpaths: broken_subpaths },
			});
		}
	}

	/// Delete point(s) and adjacent segments, which breaks a closed path as open, or an open path into multiple.
	pub fn delete_point_and_break_path(&self, document_network: &NodeNetwork, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			let Some(subpaths) = get_subpaths(layer, document_network) else {
				continue;
			};

			let mut broken_subpaths = Vec::<bezier_rs::Subpath<ManipulatorGroupId>>::with_capacity(subpaths.len());

			for subpath in subpaths {
				let mut selected_points: Vec<_> = state.selected_points.iter().filter_map(|&point| subpath.manipulator_index_from_id(point.group)).collect();

				if selected_points.is_empty() {
					broken_subpaths.push(subpath.clone());
					continue;
				}

				selected_points.sort();

				// Required to remove duplicates when the handles and anchors are selected
				selected_points.dedup();

				let mut last_manipulator_index = 0;
				let mut to_extend_with_last_group: Option<Vec<ManipulatorGroup<ManipulatorGroupId>>> = None;
				for (i, &manipulator_index) in selected_points.iter().enumerate() {
					if (manipulator_index == 0 || manipulator_index == 1) && !subpath.closed {
						last_manipulator_index = manipulator_index + 1;
						continue;
					}

					let segment = subpath.manipulator_groups()[last_manipulator_index..manipulator_index].to_vec();
					if subpath.closed && i == 0 {
						to_extend_with_last_group = Some(segment);
					} else {
						broken_subpaths.push(bezier_rs::Subpath::new(segment, false));
					}

					last_manipulator_index = manipulator_index + 1;
				}

				if (last_manipulator_index == subpath.len() || last_manipulator_index == subpath.len() - 1) && !subpath.closed {
					continue;
				}

				let mut final_segment = subpath.manipulator_groups()[last_manipulator_index..].to_vec();

				if let Some(group) = to_extend_with_last_group {
					final_segment.extend(group);
				}

				broken_subpaths.push(bezier_rs::Subpath::new(final_segment, false));
			}

			let modification = VectorDataModification::UpdateSubpaths { subpaths: broken_subpaths };
			responses.add(GraphOperationMessage::Vector { layer, modification });
		}
	}

	/// Toggle if the handles of the selected points should be colinear.
	pub fn toggle_colinear_handles_state_on_selected(&self, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			for point in &state.selected_points {
				let modification = VectorDataModification::ToggleManipulatorColinearHandlesState { id: point.group };
				responses.add(GraphOperationMessage::Vector { layer, modification })
			}
		}
	}

	/// Set whether the handles of the selected points should be colinear.
	pub fn set_colinear_handles_state_on_selected(&self, colinear: bool, responses: &mut VecDeque<Message>) {
		for (&layer, state) in &self.selected_shape_state {
			for point in &state.selected_points {
				let modification = VectorDataModification::SetManipulatorColinearHandlesState { id: point.group, colinear };
				responses.add(GraphOperationMessage::Vector { layer, modification });
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
	fn closest_segment(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, layer: LayerNodeIdentifier, position: glam::DVec2, tolerance: f64) -> Option<ClosestSegment> {
		let transform = document_metadata.transform_to_viewport(layer);
		let layer_pos = transform.inverse().transform_point2(position);

		let scale = document_metadata.document_to_viewport.decompose_scale().x;
		let tolerance = tolerance + 0.5 * scale; // make more talerance at large scale

		let mut closest = None;
		let mut closest_distance_squared: f64 = tolerance * tolerance;

		let subpaths = get_subpaths(layer, document_network)?;

		for (subpath_index, subpath) in subpaths.iter().enumerate() {
			for (manipulator_index, bezier) in subpath.iter().enumerate() {
				let t = bezier.project(layer_pos);
				let layerspace = bezier.evaluate(TValue::Parametric(t));

				let screenspace = transform.transform_point2(layerspace);
				let distance_squared = screenspace.distance_squared(position);

				if distance_squared < closest_distance_squared {
					closest_distance_squared = distance_squared;

					let info = ClosestSegmentInfo {
						bezier,
						t,
						// needs for correct length calc when there is non 1x1 layer scale
						layer_scale: transform.decompose_scale() / scale,
						bezier_point_to_viewport: screenspace,
					};
					closest = Some(((subpath_index, manipulator_index), info))
				}
			}
		}

		closest.map(|((subpath_index, manipulator_index), info)| {
			let subpath = &subpaths[subpath_index];
			let start = subpath.manipulator_groups()[manipulator_index];
			let end = subpath.manipulator_groups()[(manipulator_index + 1) % subpath.len()];
			ClosestSegment::new(info, layer, document_network, start, end)
		})
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

	/// Handles the splitting of a curve to insert new points (which can be activated by double clicking on a curve with the Path tool).
	pub fn split(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) {
		if let Some(segment) = self.upper_closest_segment(document_network, document_metadata, position, tolerance) {
			segment.adjusted_insert(responses);
		}
	}

	/// Converts a nearby clicked anchor point's handles between sharp (zero-length handles) and smooth (pulled-apart handle(s)).
	/// If both handles aren't zero-length, they are set that. If both are zero-length, they are stretched apart by a reasonable amount.
	/// This can can be activated by double clicking on an anchor with the Path tool.
	pub fn flip_smooth_sharp(&self, document_network: &NodeNetwork, document_metadata: &DocumentMetadata, position: glam::DVec2, tolerance: f64, responses: &mut VecDeque<Message>) -> bool {
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
				// Check if both handles are zero-length (sharp)
				(Some(in_handle), Some(out_handle)) => anchor_position.abs_diff_eq(in_handle, 1e-10) && anchor_position.abs_diff_eq(out_handle, 1e-10),
				// Check if the only one handle is zero-length (sharp)
				(Some(handle), None) | (None, Some(handle)) => anchor_position.abs_diff_eq(handle, 1e-10),
				// No handles mean zero-length (sharp)
				(None, None) => true,
			};

			if already_sharp {
				self.convert_manipulator_handles_to_colinear(subpath, index, responses, layer);
			} else {
				// Set in handle position to anchor position
				let point = ManipulatorPointId::new(manipulator.id, SelectedType::InHandle);
				let modification = VectorDataModification::SetManipulatorPosition { point, position: anchor_position };
				responses.add(GraphOperationMessage::Vector { layer, modification });

				// Set out handle position to anchor position
				let point = ManipulatorPointId::new(manipulator.id, SelectedType::OutHandle);
				let modification = VectorDataModification::SetManipulatorPosition { point, position: anchor_position };
				responses.add(GraphOperationMessage::Vector { layer, modification });

				// Set the manipulator to have non-colinear handles
				let modification = VectorDataModification::SetManipulatorColinearHandlesState { id: manipulator.id, colinear: false };
				responses.add(GraphOperationMessage::Vector { layer, modification });
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
