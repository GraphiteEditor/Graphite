use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, HIDE_HANDLE_DISTANCE, LINE_ROTATE_SNAP_ANGLE};
use crate::messages::portfolio::document::node_graph::document_node_definitions::{self, resolve_document_node_type};
use crate::messages::portfolio::document::overlays::utility_functions::path_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};
use crate::messages::tool::common_functionality::utility_functions::should_extend;

use bezier_rs::{Bezier, BezierHandles};
use graph_craft::document::NodeId;
use graphene_core::vector::{PointId, VectorModificationType};
use graphene_core::Color;
use graphene_std::vector::{HandleId, ManipulatorPointId, SegmentId, VectorData};

#[derive(Default)]
pub struct PenTool {
	fsm_state: PenToolFsmState,
	tool_data: PenToolData,
	options: PenOptions,
}

pub struct PenOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for PenOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PenToolMessage {
	// Standard messages
	Abort,
	SelectionChanged,
	WorkingColorChanged,
	Overlays(OverlayContext),

	// Tool-specific messages

	// It is necessary to defer this until the transform of the layer can be accurately computed (quite hacky)
	AddPointLayerPosition { layer: LayerNodeIdentifier, viewport: DVec2 },
	Confirm,
	DragStart { append_to_selected: Key },
	DragStop,
	PointerMove { snap_angle: Key, break_handle: Key, lock_angle: Key, colinear: Key },
	PointerOutsideViewport { snap_angle: Key, break_handle: Key, lock_angle: Key, colinear: Key },
	Redo,
	Undo,
	UpdateOptions(PenOptionsUpdate),
	RecalculateLatestPointsPosition,
	RemovePreviousHandle,
	GRS { grab: Key, rotate: Key, scale: Key },
	FinalPosition { final_position: DVec2 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PenToolFsmState {
	#[default]
	Ready,
	DraggingHandle(HandleMode),
	PlacingAnchor,
	GRSHandle,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PenOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for PenTool {
	fn icon_name(&self) -> String {
		"VectorPenTool".into()
	}
	fn tooltip(&self) -> String {
		"Pen Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Pen
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for PenTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(color.value.as_solid())).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for PenTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Pen(PenToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			PenOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			PenOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			PenOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			PenOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			PenOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			PenToolFsmState::Ready | PenToolFsmState::GRSHandle => actions!(PenToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				Confirm,
				Abort,
				PointerMove,
				FinalPosition
			),
			PenToolFsmState::DraggingHandle(_) | PenToolFsmState::PlacingAnchor => actions!(PenToolMessageDiscriminant;
				DragStart,
				DragStop,
				PointerMove,
				Confirm,
				Abort,
				RemovePreviousHandle,
				GRS,
			),
		}
	}
}

impl ToolTransition for PenTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(PenToolMessage::Abort.into()),
			selection_changed: Some(PenToolMessage::SelectionChanged.into()),
			working_color_changed: Some(PenToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|overlay_context| PenToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}
#[derive(Clone, Debug, Default)]
struct ModifierState {
	snap_angle: bool,
	lock_angle: bool,
	break_handle: bool,
	colinear: bool,
}
#[derive(Clone, Debug)]
struct LastPoint {
	id: PointId,
	pos: DVec2,
	in_segment: Option<SegmentId>,
	handle_start: DVec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HandleMode {
	Free,
	#[default] // Pressing 'C' breaks colinearity
	ColinearLocked, // Alt pressed: Handle length is locked
	ColinearEquidistant, // Alt pressed: Handle is equidistant
}

#[derive(Clone, Debug, Default)]
struct PenToolData {
	snap_manager: SnapManager,
	latest_points: Vec<LastPoint>,
	point_index: usize,
	handle_end: Option<DVec2>,
	next_point: DVec2,
	next_handle_start: DVec2,

	g1_continuous: bool,
	toggle_colinear_debounce: bool,

	segment_end_before_bent: Option<SegmentId>,

	angle: f64,
	auto_panning: AutoPanning,
	modifiers: ModifierState,

	buffering_merged_vector: bool,

	before_grs_pos: DVec2,  // For next_point
	temp_handle_pos: DVec2, // For handle end
	alt_press: bool,

	handle_mode: HandleMode,
}
impl PenToolData {
	fn latest_point(&self) -> Option<&LastPoint> {
		self.latest_points.get(self.point_index)
	}

	fn latest_point_mut(&mut self) -> Option<&mut LastPoint> {
		self.latest_points.get_mut(self.point_index)
	}

	fn add_point(&mut self, point: LastPoint) {
		self.point_index = (self.point_index + 1).min(self.latest_points.len());
		self.latest_points.truncate(self.point_index);
		self.latest_points.push(point);
	}

	// When the vector data transform changes, the positions of the points must be recalculated.
	fn recalculate_latest_points_position(&mut self, document: &DocumentMessageHandler) {
		let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		if let (Some(layer), None) = (selected_layers.next(), selected_layers.next()) {
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				return;
			};
			for point in &mut self.latest_points {
				let Some(pos) = vector_data.point_domain.position_from_id(point.id) else {
					continue;
				};
				point.pos = pos;
				point.handle_start = point.pos;
			}
		}
	}

	/// If the user places the anchor on top of the previous anchor, it becomes sharp and the outgoing handle may be dragged.
	fn bend_from_previous_point(&mut self, snap_data: SnapData, transform: DAffine2, layer: LayerNodeIdentifier) {
		self.g1_continuous = true;
		let document = snap_data.document;
		self.next_handle_start = self.next_point;
		let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();

		// Break the control
		let Some(last_pos) = self.latest_point().map(|point| point.pos) else { return };

		let transform = document.metadata().document_to_viewport * transform;
		let on_top = transform.transform_point2(self.next_point).distance_squared(transform.transform_point2(last_pos)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		if on_top {
			if let Some(point) = self.latest_point_mut() {
				point.in_segment = None;
			}

			self.segment_end_before_bent = vector_data.segment_bezier_iter().last().map(|(segment, _, _, _)| segment);
			self.handle_mode = HandleMode::Free;
			self.handle_end = None;
		}
	}

	fn finish_placing_handle(&mut self, snap_data: SnapData, transform: DAffine2, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;
		let next_handle_start = self.next_handle_start;
		let handle_start = self.latest_point()?.handle_start;
		let mouse = snap_data.input.mouse.position;
		let Some(handle_end) = self.handle_end else {
			self.handle_end = Some(next_handle_start);
			self.place_anchor(snap_data, transform, mouse, preferences, responses);
			self.latest_point_mut()?.handle_start = next_handle_start;
			return None;
		};
		let next_point = self.next_point;
		self.place_anchor(snap_data, transform, mouse, preferences, responses);
		let handles = [handle_start - self.latest_point()?.pos, handle_end - next_point].map(Some);

		// Get close path
		let mut end = None;
		let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		let layer = selected_layers.next().filter(|_| selected_layers.next().is_none())?;
		let vector_data = document.network_interface.compute_modified_vector(layer)?;
		let start = self.latest_point()?.id;
		let transform = document.metadata().document_to_viewport * transform;
		for id in vector_data.extendable_points(preferences.vector_meshes).filter(|&point| point != start) {
			let Some(pos) = vector_data.point_domain.position_from_id(id) else { continue };
			let transformed_distance_between_squared = transform.transform_point2(pos).distance_squared(transform.transform_point2(next_point));
			let snap_point_tolerance_squared = crate::consts::SNAP_POINT_TOLERANCE.powi(2);
			if transformed_distance_between_squared < snap_point_tolerance_squared {
				end = Some(id);
			}
		}
		let close_subpath = end.is_some();

		// Generate new point if not closing
		let end = end.unwrap_or_else(|| {
			let end = PointId::generate();
			let modification_type = VectorModificationType::InsertPoint { id: end, position: next_point };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });

			end
		});

		let points = [start, end];
		let id = SegmentId::generate();
		self.segment_end_before_bent = Some(id);
		let modification_type = VectorModificationType::InsertSegment { id, points, handles };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Mirror
		if let Some(last_segment) = self.latest_point().and_then(|point| point.in_segment) {
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification_type: VectorModificationType::SetG1Continuous {
					handles: [HandleId::end(last_segment), HandleId::primary(id)],
					enabled: true,
				},
			});
		}
		if !close_subpath {
			self.add_point(LastPoint {
				id: end,
				pos: next_point,
				in_segment: self.g1_continuous.then_some(id),
				handle_start: next_handle_start,
			});
		}
		responses.add(DocumentMessage::EndTransaction);
		Some(if close_subpath { PenToolFsmState::Ready } else { PenToolFsmState::PlacingAnchor })
	}

	fn drag_handle(&mut self, snap_data: SnapData, transform: DAffine2, mouse: DVec2, responses: &mut VecDeque<Message>, layer: Option<LayerNodeIdentifier>) -> Option<PenToolFsmState> {
		let colinear = (self.handle_mode == HandleMode::ColinearEquidistant && self.modifiers.break_handle) || (self.handle_mode == HandleMode::ColinearLocked && !self.modifiers.break_handle);
		let document = snap_data.document;
		self.next_handle_start = self.compute_snapped_angle(snap_data, transform, colinear, mouse, Some(self.next_point), false);
		let Some(layer) = layer else { return Some(PenToolFsmState::DraggingHandle(self.handle_mode)) };
		let vector_data = document.network_interface.compute_modified_vector(layer)?;

		match self.handle_mode {
			HandleMode::ColinearLocked | HandleMode::ColinearEquidistant => {
				self.g1_continuous = true;
				self.colinear(responses, layer, self.next_handle_start, self.next_point, &vector_data);
				self.adjust_handle_length(responses, layer, &vector_data);
			}
			HandleMode::Free => {
				self.g1_continuous = false;
			}
		}

		responses.add(OverlaysMessage::Draw);

		Some(PenToolFsmState::DraggingHandle(self.handle_mode))
	}

	fn adjust_handle_length(&mut self, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, vector_data: &VectorData) {
		let Some(latest) = self.latest_point() else { return };
		let anchor_pos = latest.pos;

		match self.handle_mode {
			HandleMode::ColinearEquidistant => self.adjust_equidistant_handle(anchor_pos, responses, layer, vector_data),
			HandleMode::ColinearLocked => self.adjust_locked_length_handle(anchor_pos, responses, layer),
			HandleMode::Free => {} // No adjustments needed in free mode
		}
	}

	fn colinear(&mut self, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, handle_start: DVec2, anchor_point: DVec2, vector_data: &VectorData) {
		let Some(direction) = (anchor_point - handle_start).try_normalize() else {
			log::warn!("Skipping colinear adjustment: handle_start and anchor_point are too close!");
			return;
		};

		let handle_offset = if let Some(handle_end) = self.handle_end {
			(handle_end - anchor_point).length()
		} else {
			let Some(segment) = self.segment_end_before_bent else { return };
			let end_handle = ManipulatorPointId::EndHandle(segment);
			let Some(end_handle) = end_handle.get_position(vector_data) else { return };
			(end_handle - anchor_point).length()
		};

		let new_handle_position = anchor_point + handle_offset * direction;
		self.update_handle_position(new_handle_position, anchor_point, responses, layer);
	}

	fn adjust_equidistant_handle(&mut self, anchor_pos: DVec2, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, vector_data: &VectorData) {
		if self.modifiers.break_handle {
			// Store original handle before equidistant adjustment
			self.store_handle(vector_data);
			self.alt_press = true;

			let new_position = self.next_point * 2. - self.next_handle_start;
			self.update_handle_position(new_position, anchor_pos, responses, layer);
		} else {
			self.restore_previous_handle(anchor_pos, responses, layer);
		}
	}

	fn adjust_locked_length_handle(&mut self, anchor_pos: DVec2, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier) {
		if !self.modifiers.break_handle {
			let new_position = self.next_point * 2. - self.next_handle_start;
			self.update_handle_position(new_position, anchor_pos, responses, layer);
		}
	}

	fn store_handle(&mut self, vector_data: &VectorData) {
		if self.temp_handle_pos == DVec2::ZERO {
			self.temp_handle_pos = self.handle_end.unwrap_or_else(|| {
				let Some(segment) = self.segment_end_before_bent else { return DVec2::ZERO };
				ManipulatorPointId::EndHandle(segment).get_position(vector_data).unwrap_or(DVec2::ZERO)
			});
		}
	}

	fn restore_previous_handle(&mut self, anchor_pos: DVec2, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier) {
		if self.alt_press {
			self.alt_press = false;
			self.update_handle_position(self.temp_handle_pos, anchor_pos, responses, layer);
			self.temp_handle_pos = DVec2::ZERO; // Reset storage to avoid reuse
		}
	}

	fn update_handle_position(&mut self, new_position: DVec2, anchor_pos: DVec2, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier) {
		if let Some(handle) = self.handle_end.as_mut() {
			*handle = new_position;
		} else {
			let Some(segment) = self.segment_end_before_bent else { return };

			let relative_position = new_position - anchor_pos;
			let modification_type = VectorModificationType::SetEndHandle { segment, relative_position };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}
	}

	fn place_anchor(&mut self, snap_data: SnapData, transform: DAffine2, mouse: DVec2, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;

		let relative = self.latest_point().map(|point| point.pos);
		self.next_point = self.compute_snapped_angle(snap_data, transform, false, mouse, relative, true);

		let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		let layer = selected_layers.next().filter(|_| selected_layers.next().is_none())?;
		let vector_data = document.network_interface.compute_modified_vector(layer)?;
		let transform = document.metadata().document_to_viewport * transform;
		for point in vector_data.extendable_points(preferences.vector_meshes) {
			let Some(pos) = vector_data.point_domain.position_from_id(point) else { continue };
			let transformed_distance_between_squared = transform.transform_point2(pos).distance_squared(transform.transform_point2(self.next_point));
			let snap_point_tolerance_squared = crate::consts::SNAP_POINT_TOLERANCE.powi(2);
			if transformed_distance_between_squared < snap_point_tolerance_squared {
				self.next_point = pos;
			}
		}
		if let Some(handle_end) = self.handle_end.as_mut() {
			*handle_end = self.next_point;
			self.next_handle_start = self.next_point;
		}
		responses.add(OverlaysMessage::Draw);

		Some(PenToolFsmState::PlacingAnchor)
	}

	/// Snap the angle of the line from relative to position if the key is pressed.
	fn compute_snapped_angle(&mut self, snap_data: SnapData, transform: DAffine2, colinear: bool, mouse: DVec2, relative: Option<DVec2>, neighbor: bool) -> DVec2 {
		let ModifierState { snap_angle, lock_angle, .. } = self.modifiers;
		let document = snap_data.document;
		let mut document_pos = document.metadata().document_to_viewport.inverse().transform_point2(mouse);
		let snap = &mut self.snap_manager;

		let neighbors = relative.filter(|_| neighbor).map_or(Vec::new(), |neighbor| vec![neighbor]);

		let config = SnapTypeConfiguration::default();
		if let Some(relative) = relative
			.map(|layer| transform.transform_point2(layer))
			.filter(|&relative| (snap_angle || lock_angle) && (relative - document_pos).length_squared() > f64::EPSILON * 100.)
		{
			let resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();

			let angle = if lock_angle {
				self.angle
			} else if (relative - document_pos) != DVec2::ZERO && !lock_angle {
				(-(relative - document_pos).angle_to(DVec2::X) / resolution).round() * resolution
			} else {
				self.angle
			};
			document_pos = relative - (relative - document_pos).project_onto(DVec2::new(angle.cos(), angle.sin()));

			let constraint = SnapConstraint::Line {
				origin: relative,
				direction: document_pos - relative,
			};
			let near_point = SnapCandidatePoint::handle_neighbors(document_pos, neighbors.clone());
			let far_point = SnapCandidatePoint::handle_neighbors(2. * relative - document_pos, neighbors);
			if colinear {
				let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
				let snapped_far = snap.constrained_snap(&snap_data, &far_point, constraint, config);
				document_pos = if snapped_far.other_snap_better(&snapped) {
					snapped.snapped_point_document
				} else {
					2. * relative - snapped_far.snapped_point_document
				};
				snap.update_indicator(if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far });
			} else {
				let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
				document_pos = snapped.snapped_point_document;
				snap.update_indicator(snapped);
			}
		} else if let Some(relative) = relative.map(|layer| transform.transform_point2(layer)).filter(|_| colinear) {
			let snapped = snap.free_snap(&snap_data, &SnapCandidatePoint::handle_neighbors(document_pos, neighbors.clone()), config);
			let snapped_far = snap.free_snap(&snap_data, &SnapCandidatePoint::handle_neighbors(2. * relative - document_pos, neighbors), config);
			document_pos = if snapped_far.other_snap_better(&snapped) {
				snapped.snapped_point_document
			} else {
				2. * relative - snapped_far.snapped_point_document
			};
			snap.update_indicator(if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far });
		} else {
			let snapped = snap.free_snap(&snap_data, &SnapCandidatePoint::handle_neighbors(document_pos, neighbors), config);
			document_pos = snapped.snapped_point_document;
			snap.update_indicator(snapped);
		}

		if let Some(relative) = relative.map(|layer| transform.transform_point2(layer)) {
			if (relative - document_pos) != DVec2::ZERO {
				self.angle = -(relative - document_pos).angle_to(DVec2::X)
			}
		}

		transform.inverse().transform_point2(document_pos)
	}

	fn create_initial_point(
		&mut self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
		tool_options: &PenOptions,
		append: bool,
		preferences: &PreferencesMessageHandler,
	) {
		let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
		let snapped = self.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
		let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);

		let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
		self.handle_end = None;

		let tolerance = crate::consts::SNAP_POINT_TOLERANCE;
		if let Some((layer, point, position)) = should_extend(document, viewport, tolerance, selected_nodes.selected_layers(document.metadata()), preferences) {
			// Perform extension of an existing path
			self.add_point(LastPoint {
				id: point,
				pos: position,
				in_segment: None,
				handle_start: position,
			});
			responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
			self.next_point = position;
			self.next_handle_start = position;

			return;
		}

		if append {
			let mut selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&document.network_interface);
			let existing_layer = selected_layers_except_artboards.next().filter(|_| selected_layers_except_artboards.next().is_none());
			if let Some(layer) = existing_layer {
				// Add point to existing layer
				responses.add(PenToolMessage::AddPointLayerPosition { layer, viewport });
				return;
			}
		}

		// New path layer
		let node_type = resolve_document_node_type("Path").expect("Path node does not exist");
		let nodes = vec![(NodeId(0), node_type.default_node_template())];

		let parent = document.new_layer_bounding_artboard(input);
		let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
		tool_options.fill.apply_fill(layer, responses);
		tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });

		// This causes the following message to be run only after the next graph evaluation runs and the transforms are updated
		responses.add(Message::StartBuffer);
		// It is necessary to defer this until the transform of the layer can be accurately computed (quite hacky)
		responses.add(PenToolMessage::AddPointLayerPosition { layer, viewport });
	}

	fn add_point_layer_position(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, viewport: DVec2) {
		// Add the first point
		let id = PointId::generate();
		let pos = document.metadata().transform_to_viewport(layer).inverse().transform_point2(viewport);
		let modification_type = VectorModificationType::InsertPoint { id, position: pos };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });
		self.add_point(LastPoint {
			id,
			pos,
			in_segment: None,
			handle_start: pos,
		});
		self.next_point = pos;
		self.next_handle_start = pos;
		self.handle_end = None;
	}
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;
	type ToolOptions = PenOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			shape_editor,
			preferences,
			..
		} = tool_action_data;

		let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		let layer = selected_layers.next().filter(|_| selected_layers.next().is_none());
		let mut transform = layer.map(|layer| document.metadata().transform_to_document(layer)).unwrap_or_default();

		if !transform.inverse().is_finite() {
			let parent_transform = layer.and_then(|layer| layer.parent(document.metadata())).map(|layer| document.metadata().transform_to_document(layer));

			transform = parent_transform.unwrap_or(DAffine2::IDENTITY);
		}

		if !transform.inverse().is_finite() {
			transform = DAffine2::IDENTITY;
		}

		let ToolMessage::Pen(event) = event else { return self };
		match (self, event) {
			(PenToolFsmState::PlacingAnchor | PenToolFsmState::GRSHandle, PenToolMessage::GRS { grab, rotate, scale }) => {
				let segment = tool_data.segment_end_before_bent;
				let Some(layer) = layer else { return PenToolFsmState::PlacingAnchor };

				let Some(latest) = tool_data.latest_point() else { return PenToolFsmState::PlacingAnchor };

				let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();

				if latest.handle_start != latest.pos {
					let viewport = document.metadata().transform_to_viewport(layer);
					let last_point = viewport.transform_point2(latest.pos);
					let handle = viewport.transform_point2(latest.handle_start);

					if input.keyboard.key(grab) {
						responses.add(TransformLayerMessage::BeginGrabPen { last_point, handle });
					} else if input.keyboard.key(rotate) {
						responses.add(TransformLayerMessage::BeginRotatePen { last_point, handle });
					} else if input.keyboard.key(scale) {
						responses.add(TransformLayerMessage::BeginScalePen { last_point, handle });
					}

					tool_data.before_grs_pos = latest.handle_start;
					// Store the handle_end position
					if let Some(segment) = segment {
						tool_data.temp_handle_pos = ManipulatorPointId::EndHandle(segment).get_position(&vector_data).unwrap_or(DVec2::ZERO);
					}
				}
				PenToolFsmState::GRSHandle
			}
			(PenToolFsmState::GRSHandle, PenToolMessage::FinalPosition { final_position: final_pos }) => {
				let Some(layer) = layer else { return PenToolFsmState::GRSHandle };
				let vector_data = document.network_interface.compute_modified_vector(layer);
				let Some(vector_data) = vector_data else { return PenToolFsmState::GRSHandle };

				if let Some(latest_pt) = tool_data.latest_point_mut() {
					let layer_space_to_viewport = document.metadata().transform_to_viewport(layer);
					let final_pos = layer_space_to_viewport.inverse().transform_point2(final_pos);
					latest_pt.handle_start = final_pos;
				}

				// Making the end handle colinear
				match tool_data.handle_mode {
					HandleMode::Free => {}
					HandleMode::ColinearEquidistant | HandleMode::ColinearLocked => {
						if let Some((latest, segment)) = tool_data.latest_point().zip(tool_data.segment_end_before_bent) {
							let handle = ManipulatorPointId::EndHandle(segment).get_position(&vector_data);
							let Some(handle) = handle else { return PenToolFsmState::GRSHandle };

							let Some(direction) = (latest.pos - latest.handle_start).try_normalize() else {
								log::warn!("Skipping handle adjustment: latest.pos and latest.handle_start are too close!");
								return PenToolFsmState::GRSHandle;
							};

							let relative_distance = (handle - latest.pos).length();
							let relative_position = relative_distance * direction;
							let modification_type = VectorModificationType::SetEndHandle { segment, relative_position };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}
				}

				responses.add(OverlaysMessage::Draw);

				PenToolFsmState::GRSHandle
			}
			(PenToolFsmState::GRSHandle, PenToolMessage::Confirm) => {
				tool_data.next_point = input.mouse.position;
				tool_data.next_handle_start = input.mouse.position;

				responses.add(OverlaysMessage::Draw);
				responses.add(PenToolMessage::PointerMove {
					snap_angle: Key::Control,
					break_handle: Key::Alt,
					lock_angle: Key::Shift,
					colinear: Key::KeyC,
				});

				PenToolFsmState::PlacingAnchor
			}
			(PenToolFsmState::GRSHandle, PenToolMessage::Abort) => {
				tool_data.next_point = input.mouse.position;
				tool_data.next_handle_start = input.mouse.position;

				let Some(layer) = layer else { return PenToolFsmState::GRSHandle };

				let previous = tool_data.before_grs_pos;
				if let Some(latest) = tool_data.latest_point_mut() {
					latest.handle_start = previous;
				}

				responses.add(OverlaysMessage::Draw);
				responses.add(PenToolMessage::PointerMove {
					snap_angle: Key::Control,
					break_handle: Key::Alt,
					lock_angle: Key::Shift,
					colinear: Key::KeyC,
				});

				// Set the handle-end back to original position
				if let Some((latest, segment)) = tool_data.latest_point().zip(tool_data.segment_end_before_bent) {
					let relative = tool_data.temp_handle_pos - latest.pos;
					let modification_type = VectorModificationType::SetEndHandle { segment, relative_position: relative };
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}

				PenToolFsmState::PlacingAnchor
			}
			(_, PenToolMessage::SelectionChanged) => {
				responses.add(OverlaysMessage::Draw);
				self
			}
			(PenToolFsmState::Ready, PenToolMessage::Overlays(mut overlay_context)) => {
				path_overlays(document, shape_editor, &mut overlay_context);
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				self
			}
			(_, PenToolMessage::Overlays(mut overlay_context)) => {
				let valid = |point: DVec2, handle: DVec2| point.distance_squared(handle) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;

				let transform = document.metadata().document_to_viewport * transform;

				// The currently-being-placed anchor
				let next_anchor = transform.transform_point2(tool_data.next_point);
				// The currently-being-placed anchor's outgoing handle (the one currently being dragged out)
				let next_handle_start = transform.transform_point2(tool_data.next_handle_start);

				// The most recently placed anchor
				let anchor_start = tool_data.latest_point().map(|point| transform.transform_point2(point.pos));
				// The most recently placed anchor's incoming handle (opposite the one currently being dragged out)
				let handle_end = tool_data.handle_end.map(|point| transform.transform_point2(point));
				// The most recently placed anchor's outgoing handle (which is currently influencing the currently-being-placed segment)
				let handle_start = tool_data.latest_point().map(|point| transform.transform_point2(point.handle_start));

				if let (Some((start, handle_start)), Some(handle_end)) = (tool_data.latest_point().map(|point| (point.pos, point.handle_start)), tool_data.handle_end) {
					let handles = BezierHandles::Cubic { handle_start, handle_end };
					let end = tool_data.next_point;
					let bezier = Bezier { start, handles, end };
					// Draw the curve for the currently-being-placed segment
					overlay_context.outline_bezier(bezier, transform);
				}

				// Draw the line between the currently-being-placed anchor and its currently-being-dragged-out outgoing handle (opposite the one currently being dragged out)
				overlay_context.line(next_anchor, next_handle_start, None);

				if let (Some(anchor_start), Some(handle_start), Some(handle_end)) = (anchor_start, handle_start, handle_end) {
					// Draw the line between the most recently placed anchor and its outgoing handle (which is currently influencing the currently-being-placed segment)
					overlay_context.line(anchor_start, handle_start, None);

					// Draw the line between the currently-being-placed anchor and its incoming handle (opposite the one currently being dragged out)
					overlay_context.line(next_anchor, handle_end, None);

					path_overlays(document, shape_editor, &mut overlay_context);

					if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) && valid(next_anchor, handle_end) {
						// Draw the handle circle for the currently-being-dragged-out incoming handle (opposite the one currently being dragged out)
						overlay_context.manipulator_handle(handle_end, false, None);
					}

					if valid(anchor_start, handle_start) {
						// Draw the handle circle for the most recently placed anchor's outgoing handle (which is currently influencing the currently-being-placed segment)
						overlay_context.manipulator_handle(handle_start, false, None);
					}
				} else {
					// Draw the whole path and its manipulators when the user is clicking-and-dragging out from the most recently placed anchor to set its outgoing handle, during which it would otherwise not have its overlays drawn
					path_overlays(document, shape_editor, &mut overlay_context);
				}

				if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) && valid(next_anchor, next_handle_start) {
					// Draw the handle circle for the currently-being-dragged-out outgoing handle (the one currently being dragged out, under the user's cursor)
					overlay_context.manipulator_handle(next_handle_start, false, None);
				}

				if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) {
					// Draw the anchor square for the most recently placed anchor
					overlay_context.manipulator_anchor(next_anchor, false, None);
				}

				// Draw the overlays that visualize current snapping
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

				self
			}
			(_, PenToolMessage::WorkingColorChanged) => {
				responses.add(PenToolMessage::UpdateOptions(PenOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			(PenToolFsmState::Ready, PenToolMessage::DragStart { append_to_selected }) => {
				responses.add(DocumentMessage::StartTransaction);
				tool_data.handle_mode = HandleMode::Free;
				tool_data.create_initial_point(document, input, responses, tool_options, input.keyboard.key(append_to_selected), preferences);

				// Enter the dragging handle state while the mouse is held down, allowing the user to move the mouse and position the handle
				PenToolFsmState::DraggingHandle(tool_data.handle_mode)
			}
			(_, PenToolMessage::AddPointLayerPosition { layer, viewport }) => {
				tool_data.add_point_layer_position(document, responses, layer, viewport);

				self
			}
			(state, PenToolMessage::RecalculateLatestPointsPosition) => {
				tool_data.recalculate_latest_points_position(document);
				state
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::DragStart { append_to_selected }) => {
				let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
				let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
				let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);

				// Early return if the buffer was started and this message is being run again after the buffer (so that place_anchor updates the state with the newly merged vector)
				if tool_data.buffering_merged_vector {
					tool_data.buffering_merged_vector = false;
					tool_data.handle_mode = HandleMode::ColinearLocked;
					tool_data.bend_from_previous_point(SnapData::new(document, input), transform, layer.unwrap());
					tool_data.place_anchor(SnapData::new(document, input), transform, input.mouse.position, preferences, responses);
					tool_data.buffering_merged_vector = false;
					PenToolFsmState::DraggingHandle(tool_data.handle_mode)
				} else {
					if tool_data.handle_end.is_some() {
						responses.add(DocumentMessage::StartTransaction);
					}
					// Merge two layers if the point is connected to the end point of another path

					// This might not be the correct solution to artboards being included as the other layer, which occurs due to the compute_modified_vector call in should_extend using the click targets for a layer instead of vector data.
					let layers = LayerNodeIdentifier::ROOT_PARENT
						.descendants(document.metadata())
						.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));
					if let Some((other_layer, _, _)) = should_extend(document, viewport, crate::consts::SNAP_POINT_TOLERANCE, layers, preferences) {
						let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
						let mut selected_layers = selected_nodes.selected_layers(document.metadata());
						if let Some(current_layer) = selected_layers.next().filter(|current_layer| selected_layers.next().is_none() && *current_layer != other_layer) {
							merge_layers(document, current_layer, other_layer, responses);
						}
					}
					// Even if no buffer was started, the message still has to be run again in order to call bend_from_previous_point
					tool_data.buffering_merged_vector = true;
					responses.add(PenToolMessage::DragStart { append_to_selected });
					PenToolFsmState::PlacingAnchor
				}
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::RemovePreviousHandle) => {
				if let Some(last_point) = tool_data.latest_points.last_mut() {
					last_point.handle_start = last_point.pos;
					responses.add(OverlaysMessage::Draw);
				} else {
					log::warn!("No latest point available to modify handle_start.");
				}
				self
			}
			(PenToolFsmState::DraggingHandle(_), PenToolMessage::DragStop) => tool_data
				.finish_placing_handle(SnapData::new(document, input), transform, preferences, responses)
				.unwrap_or(PenToolFsmState::PlacingAnchor),
			(
				PenToolFsmState::DraggingHandle(_),
				PenToolMessage::PointerMove {
					snap_angle,
					break_handle,
					lock_angle,
					colinear,
				},
			) => {
				tool_data.modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
					colinear: input.keyboard.key(colinear),
				};
				let snap_data = SnapData::new(document, input);

				if tool_data.modifiers.colinear && !tool_data.toggle_colinear_debounce {
					tool_data.handle_mode = match tool_data.handle_mode {
						HandleMode::Free => HandleMode::ColinearEquidistant,
						HandleMode::ColinearEquidistant | HandleMode::ColinearLocked => HandleMode::Free,
					};
					tool_data.toggle_colinear_debounce = true;
				}

				if !tool_data.modifiers.colinear {
					tool_data.toggle_colinear_debounce = false;
				}

				let state = tool_data.drag_handle(snap_data, transform, input.mouse.position, responses, layer).unwrap_or(PenToolFsmState::Ready);

				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
					}
					.into(),
					PenToolMessage::PointerMove {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
					}
					.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				state
			}
			(
				PenToolFsmState::PlacingAnchor,
				PenToolMessage::PointerMove {
					snap_angle,
					break_handle,
					lock_angle,
					colinear,
				},
			) => {
				tool_data.modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
					colinear: input.keyboard.key(colinear),
				};
				let state = tool_data
					.place_anchor(SnapData::new(document, input), transform, input.mouse.position, preferences, responses)
					.unwrap_or(PenToolFsmState::Ready);

				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
					}
					.into(),
					PenToolMessage::PointerMove {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
					}
					.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				state
			}
			(PenToolFsmState::Ready, PenToolMessage::PointerMove { .. }) => {
				tool_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(PenToolFsmState::DraggingHandle(mode), PenToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				PenToolFsmState::DraggingHandle(mode)
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				PenToolFsmState::PlacingAnchor
			}
			(
				state,
				PenToolMessage::PointerOutsideViewport {
					snap_angle,
					break_handle,
					lock_angle,
					colinear,
				},
			) => {
				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
					}
					.into(),
					PenToolMessage::PointerMove {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
					}
					.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(PenToolFsmState::DraggingHandle(..) | PenToolFsmState::PlacingAnchor, PenToolMessage::Confirm) => {
				responses.add(DocumentMessage::EndTransaction);
				tool_data.handle_end = None;
				tool_data.latest_points.clear();
				tool_data.point_index = 0;
				tool_data.snap_manager.cleanup(responses);

				PenToolFsmState::Ready
			}
			(_, PenToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.handle_end = None;
				tool_data.latest_points.clear();
				tool_data.point_index = 0;
				tool_data.snap_manager.cleanup(responses);

				responses.add(OverlaysMessage::Draw);

				PenToolFsmState::Ready
			}
			(PenToolFsmState::DraggingHandle(..) | PenToolFsmState::PlacingAnchor, PenToolMessage::Undo) => {
				if tool_data.point_index > 0 {
					tool_data.point_index -= 1;
					tool_data
						.place_anchor(SnapData::new(document, input), transform, input.mouse.position, preferences, responses)
						.unwrap_or(PenToolFsmState::PlacingAnchor)
				} else {
					responses.add(PenToolMessage::Abort);
					self
				}
			}
			(_, PenToolMessage::Redo) => {
				tool_data.point_index = (tool_data.point_index + 1).min(tool_data.latest_points.len().saturating_sub(1));
				tool_data.place_anchor(SnapData::new(document, input), transform, input.mouse.position, preferences, responses);
				match tool_data.point_index {
					0 => PenToolFsmState::Ready,
					_ => PenToolFsmState::PlacingAnchor,
				}
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			PenToolFsmState::Ready | PenToolFsmState::GRSHandle => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Draw Path"),
				// TODO: Only show this if a single layer is selected and it's of a valid type (e.g. a vector path but not raster or artboard)
				HintInfo::keys([Key::Shift], "Append to Selected Layer").prepend_plus(),
			])]),
			PenToolFsmState::PlacingAnchor => HintData(vec![
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::Rmb, ""),
					HintInfo::keys([Key::Escape], "").prepend_slash(),
					HintInfo::keys([Key::Enter], "End Path").prepend_slash(),
				]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Snap 15°"), HintInfo::keys([Key::Control], "Lock Angle")]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Add Sharp Point"), HintInfo::mouse(MouseMotion::LmbDrag, "Add Smooth Point")]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, ""), HintInfo::mouse(MouseMotion::LmbDrag, "Bend Prev. Point").prepend_slash()]),
			]),
			PenToolFsmState::DraggingHandle(mode) => {
				let mut dragging_hint_data = HintData(Vec::new());
				dragging_hint_data.0.push(HintGroup(vec![
					HintInfo::mouse(MouseMotion::Rmb, ""),
					HintInfo::keys([Key::Escape], "").prepend_slash(),
					HintInfo::keys([Key::Enter], "End Path").prepend_slash(),
				]));

				let toggle_group = match mode {
					HandleMode::Free => {
						let mut hints = vec![];
						hints.push(HintInfo::keys([Key::KeyC], "Make Handles Colinear"));
						hints
					}
					HandleMode::ColinearLocked | HandleMode::ColinearEquidistant => {
						let mut hints = vec![];
						hints.push(HintInfo::keys([Key::KeyC], "Break Colinear Handles"));
						hints
					}
				};

				let hold_group = match mode {
					HandleMode::Free => {
						let mut hints = vec![];
						hints.push(HintInfo::keys([Key::Shift], "Snap 15°"));
						hints.push(HintInfo::keys([Key::Control], "Lock Angle"));
						hints
					}
					HandleMode::ColinearLocked => {
						let mut hints = vec![];
						hints.push(HintInfo::keys([Key::Alt], "Non-Equidistant Handles"));
						hints.push(HintInfo::keys([Key::Shift], "Snap 15°"));
						hints.push(HintInfo::keys([Key::Control], "Lock Angle"));
						hints
					}
					HandleMode::ColinearEquidistant => {
						let mut hints = vec![];
						hints.push(HintInfo::keys([Key::Alt], "Equidistant Handles"));
						hints.push(HintInfo::keys([Key::Shift], "Snap 15°"));
						hints.push(HintInfo::keys([Key::Control], "Lock Angle"));
						hints
					}
				};

				dragging_hint_data.0.push(HintGroup(toggle_group));
				dragging_hint_data.0.push(HintGroup(hold_group));
				dragging_hint_data
			}
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn merge_layers(document: &DocumentMessageHandler, current_layer: LayerNodeIdentifier, other_layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	// Calculate the downstream transforms in order to bring the other vector data into the same layer space
	let current_transform = document.metadata().downstream_transform_to_document(current_layer);
	let other_transform = document.metadata().downstream_transform_to_document(other_layer);
	// Represents the change in position that would occur if the other layer was moved below the current layer
	let transform_delta = current_transform * other_transform.inverse();
	let offset = transform_delta.inverse();
	responses.add(GraphOperationMessage::TransformChange {
		layer: other_layer,
		transform: offset,
		transform_in: crate::messages::portfolio::document::graph_operation::utility_types::TransformIn::Local,
		skip_rerender: false,
	});

	// Move the other layer below the current layer for positioning purposes
	let current_layer_parent = current_layer.parent(document.metadata()).unwrap();
	let current_layer_index = current_layer_parent.children(document.metadata()).position(|child| child == current_layer).unwrap();
	responses.add(NodeGraphMessage::MoveLayerToStack {
		layer: other_layer,
		parent: current_layer_parent,
		insert_index: current_layer_index + 1,
	});

	// Merge the inputs of the two layers
	let merge_node_id = NodeId::new();
	let merge_node = document_node_definitions::resolve_document_node_type("Merge")
		.expect("Failed to create merge node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: merge_node_id,
		node_template: merge_node,
	});
	responses.add(NodeGraphMessage::SetToNodeOrLayer {
		node_id: merge_node_id,
		is_layer: false,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: merge_node_id,
		parent: current_layer,
	});
	responses.add(NodeGraphMessage::ConnectUpstreamOutputToInput {
		downstream_input: InputConnector::node(other_layer.to_node(), 1),
		input_connector: InputConnector::node(merge_node_id, 1),
	});
	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: vec![other_layer.to_node()],
		delete_children: false,
	});

	// Add a flatten vector elements node after the merge
	let flatten_node_id = NodeId::new();
	let flatten_node = document_node_definitions::resolve_document_node_type("Flatten Vector Elements")
		.expect("Failed to create flatten node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: flatten_node_id,
		node_template: flatten_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: flatten_node_id,
		parent: current_layer,
	});

	// Add a path node after the flatten node
	let path_node_id = NodeId::new();
	let path_node = document_node_definitions::resolve_document_node_type("Path")
		.expect("Failed to create path node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: path_node_id,
		node_template: path_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: path_node_id,
		parent: current_layer,
	});

	// Add a transform node to ensure correct tooling modifications
	let transform_node_id = NodeId::new();
	let transform_node = document_node_definitions::resolve_document_node_type("Transform")
		.expect("Failed to create transform node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: transform_node_id,
		node_template: transform_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: transform_node_id,
		parent: current_layer,
	});

	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(Message::StartBuffer);
	responses.add(PenToolMessage::RecalculateLatestPointsPosition);
}
