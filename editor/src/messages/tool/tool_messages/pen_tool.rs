use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, HIDE_HANDLE_DISTANCE, LINE_ROTATE_SNAP_ANGLE};
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_overlays;
use crate::messages::portfolio::document::overlays::utility_types::{DrawHandles, OverlayContext};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, merge_layers};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::snapping::{SnapCache, SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};
use crate::messages::tool::common_functionality::utility_functions::{closest_point, should_extend};
use bezier_rs::{Bezier, BezierHandles};
use graph_craft::document::NodeId;
use graphene_core::Color;
use graphene_core::vector::{PointId, VectorModificationType};
use graphene_std::vector::{HandleId, ManipulatorPointId, NoHashBuilder, SegmentId, VectorData};

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
	pen_overlay_mode: PenOverlayMode,
}

impl Default for PenOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_secondary(),
			stroke: ToolColorOptions::new_primary(),
			pen_overlay_mode: PenOverlayMode::FrontierHandles,
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
	AddPointLayerPosition {
		layer: LayerNodeIdentifier,
		viewport: DVec2,
	},
	Confirm,
	DragStart {
		append_to_selected: Key,
	},
	DragStop,
	PointerMove {
		snap_angle: Key,
		break_handle: Key,
		lock_angle: Key,
		colinear: Key,
		move_anchor_with_handles: Key,
	},
	PointerOutsideViewport {
		snap_angle: Key,
		break_handle: Key,
		lock_angle: Key,
		colinear: Key,
		move_anchor_with_handles: Key,
	},
	Redo,
	Undo,
	UpdateOptions(PenOptionsUpdate),
	RecalculateLatestPointsPosition,
	RemovePreviousHandle,
	GRS {
		grab: Key,
		rotate: Key,
		scale: Key,
	},
	FinalPosition {
		final_position: DVec2,
	},
	SwapHandles,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PenToolFsmState {
	#[default]
	Ready,
	DraggingHandle(HandleMode),
	PlacingAnchor,
	GRSHandle,
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PenOverlayMode {
	AllHandles = 0,
	FrontierHandles = 1,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PenOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
	OverlayModeType(PenOverlayMode),
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
			|color: &ColorInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::FillColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::StrokeColor(color.value.as_solid().map(|color| color.to_linear_srgb()))).into(),
		));

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.push(create_weight_widget(self.options.line_weight));

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.push(
			RadioInput::new(vec![
				RadioEntryData::new("all")
					.icon("HandleVisibilityAll")
					.tooltip("Show all handles regardless of selection")
					.on_update(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::OverlayModeType(PenOverlayMode::AllHandles)).into()),
				RadioEntryData::new("frontier")
					.icon("HandleVisibilityFrontier")
					.tooltip("Show only handles at the frontiers of the segments connected to selected points")
					.on_update(move |_| PenToolMessage::UpdateOptions(PenOptionsUpdate::OverlayModeType(PenOverlayMode::FrontierHandles)).into()),
			])
			.selected_index(Some(self.options.pen_overlay_mode as u32))
			.widget_holder(),
		);

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
			PenOptionsUpdate::OverlayModeType(overlay_mode_type) => {
				self.options.pen_overlay_mode = overlay_mode_type;
				responses.add(OverlaysMessage::Draw);
			}
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
				SwapHandles
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
	move_anchor_with_handles: bool,
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
	/// Pressing 'C' breaks colinearity
	Free,
	/// Pressing 'Alt': Handle length is locked
	#[default]
	ColinearLocked,
	/// Pressing 'Alt': Handles are equidistant
	ColinearEquidistant,
}

/// The handle which is opposite to the currently dragged handle under the cursor
#[derive(Clone, Debug, Default, PartialEq, Copy)]
enum TargetHandle {
	HandleEnd,
	HandleStart,
	PrimaryHandle(SegmentId),
	EndHandle(SegmentId),
	#[default]
	None,
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

	angle: f64,
	auto_panning: AutoPanning,
	modifiers: ModifierState,

	buffering_merged_vector: bool,

	previous_handle_start_pos: DVec2,
	previous_handle_end_pos: Option<DVec2>,
	alt_press: bool,
	space_press: bool,

	handle_mode: HandleMode,
	/// The point that is being dragged
	end_point: Option<PointId>,
	end_point_segment: Option<SegmentId>,
	handle_type: TargetHandle,
	handle_start_offset: Option<DVec2>,
	handle_end_offset: Option<DVec2>,
	snap_cache: SnapCache,
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

	/// Check whether target handle is primary,end or 'self.handle_end'
	fn check_end_handle_type(&self, vector_data: &VectorData) -> TargetHandle {
		match (self.handle_end, self.end_point, self.end_point_segment) {
			(Some(_), _, _) => TargetHandle::HandleEnd,
			(None, Some(point), Some(segment)) => {
				if vector_data.segment_start_from_id(segment) == Some(point) {
					TargetHandle::PrimaryHandle(segment)
				} else {
					TargetHandle::EndHandle(segment)
				}
			}
			_ => TargetHandle::None,
		}
	}

	fn update_handle_custom(&mut self, vector_data: &VectorData) {
		let target_handle = self.check_end_handle_type(vector_data);
		self.update_handle_type(target_handle);
	}

	fn get_opposite_handle_type(&self, vector_data: &VectorData) -> TargetHandle {
		match self.handle_type {
			TargetHandle::HandleStart => self.check_end_handle_type(vector_data),
			_ => TargetHandle::HandleStart,
		}
	}

	fn update_handle_type(&mut self, handle_type: TargetHandle) {
		self.handle_type = handle_type;
	}

	fn update_target_handle_pos(&mut self, handle_type: TargetHandle, responses: &mut VecDeque<Message>, delta: DVec2, layer: LayerNodeIdentifier) {
		match handle_type {
			TargetHandle::HandleStart => {
				self.next_handle_start = delta;
			}
			TargetHandle::HandleEnd => {
				if let Some(handle) = self.handle_end.as_mut() {
					*handle = delta;
				}
			}
			TargetHandle::EndHandle(segment) => {
				let relative_position = delta - self.next_point;
				let modification_type = VectorModificationType::SetEndHandle { segment, relative_position };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
			TargetHandle::PrimaryHandle(segment) => {
				let relative_position = delta - self.next_point;
				let modification_type = VectorModificationType::SetPrimaryHandle { segment, relative_position };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
			TargetHandle::None => {}
		}
	}

	fn target_handle_position(&self, handle_type: TargetHandle, vector_data: &VectorData) -> Option<DVec2> {
		match handle_type {
			TargetHandle::PrimaryHandle(segment) => ManipulatorPointId::PrimaryHandle(segment).get_position(vector_data),
			TargetHandle::EndHandle(segment) => ManipulatorPointId::EndHandle(segment).get_position(vector_data),
			TargetHandle::HandleEnd => self.handle_end,
			TargetHandle::HandleStart => Some(self.next_handle_start),
			TargetHandle::None => None,
		}
	}

	fn opposite_target_handle_position(&self, vector_data: &VectorData) -> Option<DVec2> {
		let opposite_handle_type = self.get_opposite_handle_type(vector_data);
		self.target_handle_position(opposite_handle_type, vector_data)
	}

	/// Remove the handles selected when swapping handles
	fn cleanup_target_selections(&self, shape_editor: &mut ShapeState, layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(shape_state) = layer.and_then(|layer| shape_editor.selected_shape_state.get_mut(&layer)) else {
			return;
		};

		let Some(vector_data) = layer.and_then(|layer| document.network_interface.compute_modified_vector(layer)) else {
			return;
		};

		match self.check_end_handle_type(&vector_data) {
			TargetHandle::EndHandle(segment) => shape_state.deselect_point(ManipulatorPointId::EndHandle(segment)),
			TargetHandle::PrimaryHandle(segment) => shape_state.deselect_point(ManipulatorPointId::PrimaryHandle(segment)),
			_ => {}
		}
		responses.add(OverlaysMessage::Draw);
	}

	/// Selects the handle which is currently dragged by the user  
	fn add_target_selections(&self, shape_editor: &mut ShapeState, layer: Option<LayerNodeIdentifier>) {
		let Some(shape_state) = layer.and_then(|layer| shape_editor.selected_shape_state.get_mut(&layer)) else {
			return;
		};

		match self.handle_type {
			TargetHandle::EndHandle(segment) => shape_state.select_point(ManipulatorPointId::EndHandle(segment)),
			TargetHandle::PrimaryHandle(segment) => shape_state.select_point(ManipulatorPointId::PrimaryHandle(segment)),
			_ => {}
		}
	}

	/// Check whether moving the initially created point.
	fn moving_start_point(&self) -> bool {
		self.latest_points.len() == 1 && self.latest_point().is_some_and(|point| point.pos == self.next_point)
	}

	// When the vector data transform changes, the positions of the points must be recalculated.
	fn recalculate_latest_points_position(&mut self, document: &DocumentMessageHandler) {
		let selected_nodes = document.network_interface.selected_nodes();
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
	fn bend_from_previous_point(&mut self, snap_data: SnapData, transform: DAffine2, layer: LayerNodeIdentifier, preferences: &PreferencesMessageHandler) {
		self.g1_continuous = true;
		let document = snap_data.document;
		self.next_handle_start = self.next_point;
		let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();

		// Break the control
		let Some((last_pos, id)) = self.latest_point().map(|point| (point.pos, point.id)) else { return };

		let transform = document.metadata().document_to_viewport * transform;
		let on_top = transform.transform_point2(self.next_point).distance_squared(transform.transform_point2(last_pos)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		self.update_handle_type(TargetHandle::HandleEnd);
		if on_top {
			self.handle_end = None;
			self.handle_mode = HandleMode::Free;

			// Update `end_point_segment` that was clicked on
			self.store_clicked_endpoint(document, &transform, snap_data.input, preferences);

			if self.modifiers.lock_angle {
				self.set_lock_angle(&vector_data, id, self.end_point_segment);
				let last_segment = self.end_point_segment;
				let Some(point) = self.latest_point_mut() else { return };
				point.in_segment = last_segment;
				return;
			}

			if let Some(point) = self.latest_point_mut() {
				point.in_segment = None;
			}
		}
	}

	fn finish_placing_handle(&mut self, snap_data: SnapData, transform: DAffine2, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;
		let next_handle_start = self.next_handle_start;
		let handle_start = self.latest_point()?.handle_start;
		let mouse = snap_data.input.mouse.position;
		self.handle_end_offset = None;
		self.handle_start_offset = None;
		let Some(handle_end) = self.handle_end else {
			responses.add(DocumentMessage::EndTransaction);
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
		let selected_nodes = document.network_interface.selected_nodes();
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

		// Store the segment
		let id = SegmentId::generate();
		self.end_point_segment = Some(id);

		let points = [start, end];
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

	#[allow(clippy::too_many_arguments)]
	/// Calculates snap position delta while moving anchor and its handles.
	fn space_anchor_handle_snap(
		&mut self,
		viewport_to_document: &DAffine2,
		transform: &DAffine2,
		snap_data: &SnapData<'_>,
		mouse: &DVec2,
		vector_data: &VectorData,
		input: &InputPreprocessorMessageHandler,
	) -> Option<DVec2> {
		let end_handle = self.check_end_handle_type(vector_data);
		let end_handle_pos = self.target_handle_position(end_handle, vector_data);
		let snap = &mut self.snap_manager;
		let snap_data = SnapData::new_snap_cache(snap_data.document, input, &self.snap_cache);

		let handle_start_offset = self.handle_start_offset.unwrap_or(DVec2::ZERO);
		let document_pos = viewport_to_document.transform_point2(*mouse + handle_start_offset);

		let anchor_offset = transform.transform_point2(self.next_point - self.next_handle_start);

		let handle_start = SnapCandidatePoint::handle(document_pos);
		let anchor = SnapCandidatePoint::handle(document_pos + anchor_offset);

		let snapped_near_handle_start = snap.free_snap(&snap_data, &handle_start, SnapTypeConfiguration::default());
		let snapped_anchor = snap.free_snap(&snap_data, &anchor, SnapTypeConfiguration::default());

		let handle_snap_option = end_handle_pos.and_then(|handle| match end_handle {
			TargetHandle::None => None,
			TargetHandle::HandleStart => None,
			_ => {
				let handle_offset = transform.transform_point2(handle - self.next_handle_start);
				let handle_snap = SnapCandidatePoint::handle(document_pos + handle_offset);
				Some((handle, handle_snap))
			}
		});

		let mut delta: DVec2;
		let best_snapped = if snapped_near_handle_start.other_snap_better(&snapped_anchor) {
			delta = snapped_anchor.snapped_point_document - transform.transform_point2(self.next_point);
			snapped_anchor
		} else {
			delta = snapped_near_handle_start.snapped_point_document - transform.transform_point2(self.next_handle_start);
			snapped_near_handle_start
		};

		let Some((handle, handle_snap)) = handle_snap_option else {
			snap.update_indicator(best_snapped);
			return Some(transform.inverse().transform_vector2(delta));
		};

		let snapped_handle = snap.free_snap(&snap_data, &handle_snap, SnapTypeConfiguration::default());

		if best_snapped.other_snap_better(&snapped_handle) {
			delta = snapped_handle.snapped_point_document - transform.transform_point2(handle);
			snap.update_indicator(snapped_handle);
		} else {
			snap.update_indicator(best_snapped);
		}

		// Transform delta back to original coordinate space
		Some(transform.inverse().transform_vector2(delta))
	}

	/// Handles moving the initially created point
	fn handle_single_point_path_drag(&mut self, delta: DVec2, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		self.next_handle_start += delta;
		self.next_point += delta;

		let Some(latest) = self.latest_point_mut() else {
			return Some(PenToolFsmState::DraggingHandle(self.handle_mode));
		};

		latest.pos += delta;
		let modification_type = VectorModificationType::ApplyPointDelta { point: latest.id, delta };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		responses.add(OverlaysMessage::Draw);
		Some(PenToolFsmState::DraggingHandle(self.handle_mode))
	}

	fn move_anchor_and_handles(&mut self, delta: DVec2, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>, vector_data: &VectorData) {
		if let Some(latest_pt) = self.latest_point_mut() {
			latest_pt.pos += delta;
		}

		let Some(end_point) = self.end_point else { return };
		let modification_type_anchor = VectorModificationType::ApplyPointDelta { point: end_point, delta };
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: modification_type_anchor,
		});

		// Move the end handle
		let end_handle_type = self.check_end_handle_type(vector_data);
		match end_handle_type {
			TargetHandle::EndHandle(..) | TargetHandle::PrimaryHandle(..) => {
				let Some(handle_pos) = self.target_handle_position(end_handle_type, vector_data) else { return };
				self.update_target_handle_pos(end_handle_type, responses, handle_pos + delta, layer);
			}
			_ => {}
		}
	}

	fn drag_handle(
		&mut self,
		snap_data: SnapData,
		transform: DAffine2,
		mouse: DVec2,
		responses: &mut VecDeque<Message>,
		layer: Option<LayerNodeIdentifier>,
		input: &InputPreprocessorMessageHandler,
	) -> Option<PenToolFsmState> {
		let colinear = (self.handle_mode == HandleMode::ColinearEquidistant && self.modifiers.break_handle) || (self.handle_mode == HandleMode::ColinearLocked && !self.modifiers.break_handle);
		let document = snap_data.document;
		let Some(layer) = layer else { return Some(PenToolFsmState::DraggingHandle(self.handle_mode)) };
		let vector_data = document.network_interface.compute_modified_vector(layer)?;
		let viewport_to_document = document.metadata().document_to_viewport.inverse();

		// Handles pressing `space` to drag anchor and its handles
		if self.modifiers.move_anchor_with_handles {
			let Some(delta) = self.space_anchor_handle_snap(&viewport_to_document, &transform, &snap_data, &mouse, &vector_data, input) else {
				return Some(PenToolFsmState::DraggingHandle(self.handle_mode));
			};

			if self.moving_start_point() {
				return self.handle_single_point_path_drag(delta, layer, responses);
			}

			self.next_handle_start += delta;
			self.next_point += delta;

			if let Some(handle) = self.handle_end.as_mut() {
				*handle += delta;
			} else {
				self.move_anchor_and_handles(delta, layer, responses, &vector_data);
			}
			responses.add(OverlaysMessage::Draw);
			return Some(PenToolFsmState::DraggingHandle(self.handle_mode));
		}

		if self.handle_type != TargetHandle::HandleStart {
			let offset = self.handle_start_offset.unwrap_or(DVec2::ZERO);
			self.next_handle_start = self.compute_snapped_angle(snap_data.clone(), transform, colinear, mouse + offset, Some(self.next_point), false);
		} else {
			let mouse_offset = mouse + self.handle_end_offset.unwrap_or(DVec2::ZERO);
			let mouse_pos = self.compute_snapped_angle(snap_data.clone(), transform, colinear, mouse_offset, Some(self.next_point), false);
			let opposite_target = self.get_opposite_handle_type(&vector_data);
			self.update_target_handle_pos(opposite_target, responses, mouse_pos, layer);
		}

		match self.handle_mode {
			HandleMode::ColinearLocked | HandleMode::ColinearEquidistant => {
				self.g1_continuous = true;
				self.apply_colinear_constraint(responses, layer, self.next_point, &vector_data);
				self.adjust_handle_length(responses, layer, &vector_data);
			}
			HandleMode::Free => {
				self.g1_continuous = false;
			}
		}

		responses.add(OverlaysMessage::Draw);

		Some(PenToolFsmState::DraggingHandle(self.handle_mode))
	}

	/// Makes the opposite handle equidistant or locks its length.
	fn adjust_handle_length(&mut self, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, vector_data: &VectorData) {
		match self.handle_mode {
			HandleMode::ColinearEquidistant => {
				if self.modifiers.break_handle {
					// Store handle for later restoration only when Alt is first pressed
					if !self.alt_press {
						self.previous_handle_end_pos = self.target_handle_position(self.handle_type, vector_data);
						self.alt_press = true;
					}

					// Set handle to opposite position of the other handle
					let Some(new_position) = self.opposite_target_handle_position(vector_data).map(|handle| self.next_point * 2. - handle) else {
						return;
					};
					self.update_target_handle_pos(self.handle_type, responses, new_position, layer);
				} else if self.alt_press {
					// Restore the previous handle position when Alt is released
					if let Some(previous_handle) = self.previous_handle_end_pos {
						self.update_target_handle_pos(self.handle_type, responses, previous_handle, layer);
					}
					self.alt_press = false;
					self.previous_handle_end_pos = None;
				}
			}
			HandleMode::ColinearLocked => {
				if !self.modifiers.break_handle {
					let Some(new_position) = self.opposite_target_handle_position(vector_data).map(|handle| self.next_point * 2. - handle) else {
						return;
					};
					self.update_target_handle_pos(self.handle_type, responses, new_position, layer);
				}
			}
			HandleMode::Free => {}
		}
	}

	fn apply_colinear_constraint(&mut self, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, anchor_pos: DVec2, vector_data: &VectorData) {
		let Some(handle) = self.opposite_target_handle_position(vector_data) else {
			return;
		};

		let Some(direction) = (anchor_pos - handle).try_normalize() else {
			log::trace!("Skipping colinear adjustment: handle_start and anchor_point are too close!");
			return;
		};

		let Some(handle_offset) = self.target_handle_position(self.handle_type, vector_data).map(|handle| (handle - anchor_pos).length()) else {
			return;
		};

		let new_handle_position = anchor_pos + handle_offset * direction;
		self.update_target_handle_pos(self.handle_type, responses, new_handle_position, layer);
	}

	fn place_anchor(&mut self, snap_data: SnapData, transform: DAffine2, mouse: DVec2, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;

		let relative = self.latest_point().map(|point| point.pos);
		self.next_point = self.compute_snapped_angle(snap_data, transform, false, mouse, relative, true);

		let selected_nodes = document.network_interface.selected_nodes();
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
			if (relative - document_pos) != DVec2::ZERO && (relative - document_pos).length_squared() > f64::EPSILON * 100. {
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

		let selected_nodes = document.network_interface.selected_nodes();
		self.handle_end = None;

		let tolerance = crate::consts::SNAP_POINT_TOLERANCE;
		let extension_choice = should_extend(document, viewport, tolerance, selected_nodes.selected_layers(document.metadata()), preferences);
		if let Some((layer, point, position)) = extension_choice {
			self.extend_existing_path(document, layer, point, position, responses);
			return;
		}

		if append {
			if let Some((layer, point, _)) = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences) {
				let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();
				let segment = vector_data.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);

				if self.modifiers.lock_angle {
					self.set_lock_angle(&vector_data, point, segment);
				}
			}
			self.end_point_segment = None;
			let mut selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&document.network_interface);
			let existing_layer = selected_layers_except_artboards.next().filter(|_| selected_layers_except_artboards.next().is_none());
			if let Some(layer) = existing_layer {
				// Add point to existing layer
				responses.add(PenToolMessage::AddPointLayerPosition { layer, viewport });
				return;
			}
		}

		if let Some((layer, point, _position)) = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences) {
			let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();
			let segment = vector_data.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);

			if self.modifiers.lock_angle {
				self.set_lock_angle(&vector_data, point, segment);
				self.handle_mode = HandleMode::Free;
			}
		}

		// New path layer
		let node_type = resolve_document_node_type("Path").expect("Path node does not exist");
		let nodes = vec![(NodeId(0), node_type.default_node_template())];

		let parent = document.new_layer_bounding_artboard(input);
		let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
		tool_options.fill.apply_fill(layer, responses);
		tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
		self.end_point_segment = None;
		self.handle_type = TargetHandle::HandleEnd;
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });

		// This causes the following message to be run only after the next graph evaluation runs and the transforms are updated
		responses.add(Message::StartBuffer);
		// It is necessary to defer this until the transform of the layer can be accurately computed (quite hacky)
		responses.add(PenToolMessage::AddPointLayerPosition { layer, viewport });
	}

	/// Perform extension of an existing path
	fn extend_existing_path(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier, point: PointId, position: DVec2, responses: &mut VecDeque<Message>) {
		let vector_data = document.network_interface.compute_modified_vector(layer);
		let (handle_start, in_segment) = if let Some(vector_data) = &vector_data {
			vector_data
				.segment_bezier_iter()
				.find_map(|(segment_id, bezier, start, end)| {
					let is_end = point == end;
					let is_start = point == start;
					if !is_end && !is_start {
						return None;
					}

					let handle = match bezier.handles {
						BezierHandles::Cubic { handle_start, handle_end, .. } => {
							if is_start {
								handle_start
							} else {
								handle_end
							}
						}
						BezierHandles::Quadratic { handle } => handle,
						_ => return None,
					};
					Some((segment_id, is_end, handle))
				})
				.map(|(segment_id, is_end, handle)| {
					let mirrored_handle = position * 2. - handle;
					let in_segment = if is_end { Some(segment_id) } else { None };
					(mirrored_handle, in_segment)
				})
				.unwrap_or_else(|| (position, None))
		} else {
			(position, None)
		};

		let in_segment = if self.modifiers.lock_angle { self.end_point_segment } else { in_segment };

		self.add_point(LastPoint {
			id: point,
			pos: position,
			in_segment,
			handle_start,
		});

		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });

		self.next_point = position;
		self.next_handle_start = handle_start;
		let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();
		let segment = vector_data.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);

		if self.modifiers.lock_angle {
			self.set_lock_angle(&vector_data, point, segment);
		}
		self.handle_mode = HandleMode::Free;
	}

	// Stores the segment and point ID of the clicked endpoint
	fn store_clicked_endpoint(&mut self, document: &DocumentMessageHandler, transform: &DAffine2, input: &InputPreprocessorMessageHandler, preferences: &PreferencesMessageHandler) {
		let mut manipulators = HashMap::with_hasher(NoHashBuilder);
		let mut unselected = Vec::new();
		let mut layer_manipulators = HashSet::with_hasher(NoHashBuilder);

		let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));

		let snapped = self.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
		let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);

		let tolerance = crate::consts::SNAP_POINT_TOLERANCE;

		if let Some((layer, point, _position)) = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences) {
			self.end_point = Some(point);
			let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();
			let segment = vector_data.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);
			self.end_point_segment = segment;
			self.update_handle_custom(&vector_data);
			layer_manipulators.insert(point);
			for (&id, &position) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
				if id == point {
					continue;
				}
				unselected.push(SnapCandidatePoint::handle(transform.transform_point2(position)))
			}
			manipulators.insert(layer, layer_manipulators);
			self.snap_cache = SnapCache { manipulators, unselected }
		}
	}

	fn set_lock_angle(&mut self, vector_data: &VectorData, anchor: PointId, segment: Option<SegmentId>) {
		let anchor_position = vector_data.point_domain.position_from_id(anchor);

		let Some((anchor_position, segment)) = anchor_position.zip(segment) else {
			self.handle_mode = HandleMode::Free;
			return;
		};

		// Closure to check if a point is the start or end of a segment
		let is_start = |point: PointId, segment: SegmentId| vector_data.segment_start_from_id(segment) == Some(point);

		let end_handle = ManipulatorPointId::EndHandle(segment).get_position(vector_data);
		let start_handle = ManipulatorPointId::PrimaryHandle(segment).get_position(vector_data);

		let start_point = if is_start(anchor, segment) {
			vector_data.segment_end_from_id(segment).and_then(|id| vector_data.point_domain.position_from_id(id))
		} else {
			vector_data.segment_start_from_id(segment).and_then(|id| vector_data.point_domain.position_from_id(id))
		};

		let required_handle = if is_start(anchor, segment) {
			start_handle
				.filter(|&handle| handle != anchor_position)
				.or(end_handle.filter(|&handle| Some(handle) != start_point))
				.or(start_point)
		} else {
			end_handle
				.filter(|&handle| handle != anchor_position)
				.or(start_handle.filter(|&handle| Some(handle) != start_point))
				.or(start_point)
		};

		if let Some(required_handle) = required_handle {
			self.angle = -(required_handle - anchor_position).angle_to(DVec2::X);
			self.handle_mode = HandleMode::ColinearEquidistant;
		}
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

		let selected_nodes = document.network_interface.selected_nodes();
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
				let Some(layer) = layer else { return PenToolFsmState::PlacingAnchor };

				let Some(latest) = tool_data.latest_point() else { return PenToolFsmState::PlacingAnchor };
				if latest.handle_start == latest.pos {
					return PenToolFsmState::PlacingAnchor;
				}

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

				tool_data.previous_handle_start_pos = latest.handle_start;

				// Store the handle_end position
				let segment = tool_data.end_point_segment;
				if let Some(segment) = segment {
					let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();
					tool_data.previous_handle_end_pos = ManipulatorPointId::EndHandle(segment).get_position(&vector_data);
				}
				PenToolFsmState::GRSHandle
			}
			(PenToolFsmState::GRSHandle, PenToolMessage::FinalPosition { final_position }) => {
				let Some(layer) = layer else { return PenToolFsmState::GRSHandle };
				let vector_data = document.network_interface.compute_modified_vector(layer);
				let Some(vector_data) = vector_data else { return PenToolFsmState::GRSHandle };

				if let Some(latest_pt) = tool_data.latest_point_mut() {
					let layer_space_to_viewport = document.metadata().transform_to_viewport(layer);
					let final_pos = layer_space_to_viewport.inverse().transform_point2(final_position);
					latest_pt.handle_start = final_pos;
				}

				responses.add(OverlaysMessage::Draw);

				// Making the end handle colinear
				match tool_data.handle_mode {
					HandleMode::Free => {}
					HandleMode::ColinearEquidistant | HandleMode::ColinearLocked => {
						if let Some((latest, segment)) = tool_data.latest_point().zip(tool_data.end_point_segment) {
							let Some(direction) = (latest.pos - latest.handle_start).try_normalize() else {
								return PenToolFsmState::GRSHandle;
							};

							if (latest.pos - latest.handle_start).length_squared() < f64::EPSILON {
								return PenToolFsmState::GRSHandle;
							}

							let is_start = vector_data.segment_start_from_id(segment) == Some(latest.id);

							let handle = if is_start {
								ManipulatorPointId::PrimaryHandle(segment).get_position(&vector_data)
							} else {
								ManipulatorPointId::EndHandle(segment).get_position(&vector_data)
							};
							let Some(handle) = handle else { return PenToolFsmState::GRSHandle };
							let relative_distance = (handle - latest.pos).length();
							let relative_position = relative_distance * direction;
							let modification_type = if is_start {
								VectorModificationType::SetPrimaryHandle { segment, relative_position }
							} else {
								VectorModificationType::SetEndHandle { segment, relative_position }
							};
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
					move_anchor_with_handles: Key::Space,
				});

				PenToolFsmState::PlacingAnchor
			}
			(PenToolFsmState::GRSHandle, PenToolMessage::Abort) => {
				tool_data.next_point = input.mouse.position;
				tool_data.next_handle_start = input.mouse.position;

				let Some(layer) = layer else { return PenToolFsmState::GRSHandle };

				let previous = tool_data.previous_handle_start_pos;
				if let Some(latest) = tool_data.latest_point_mut() {
					latest.handle_start = previous;
				}

				responses.add(OverlaysMessage::Draw);
				responses.add(PenToolMessage::PointerMove {
					snap_angle: Key::Control,
					break_handle: Key::Alt,
					lock_angle: Key::Shift,
					colinear: Key::KeyC,
					move_anchor_with_handles: Key::Space,
				});

				// Set the handle-end back to original position
				if let Some(((latest, segment), handle_end)) = tool_data.latest_point().zip(tool_data.end_point_segment).zip(tool_data.previous_handle_end_pos) {
					let relative = handle_end - latest.pos;
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
				match tool_options.pen_overlay_mode {
					PenOverlayMode::AllHandles => {
						path_overlays(document, DrawHandles::All, shape_editor, &mut overlay_context);
					}
					PenOverlayMode::FrontierHandles => {
						path_overlays(document, DrawHandles::None, shape_editor, &mut overlay_context);
					}
				}
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
					if (end - start).length_squared() > f64::EPSILON {
						// Draw the curve for the currently-being-placed segment
						overlay_context.outline_bezier(bezier, transform);
					}
				}

				// Draw the line between the currently-being-placed anchor and its currently-being-dragged-out outgoing handle (opposite the one currently being dragged out)
				overlay_context.line(next_anchor, next_handle_start, None, None);

				match tool_options.pen_overlay_mode {
					PenOverlayMode::AllHandles => {
						path_overlays(document, DrawHandles::All, shape_editor, &mut overlay_context);
					}
					PenOverlayMode::FrontierHandles => {
						if let Some(latest_segment) = tool_data.end_point_segment {
							path_overlays(document, DrawHandles::SelectedAnchors(vec![latest_segment]), shape_editor, &mut overlay_context);
						} else {
							path_overlays(document, DrawHandles::None, shape_editor, &mut overlay_context);
						};
					}
				}

				if let (Some(anchor_start), Some(handle_start), Some(handle_end)) = (anchor_start, handle_start, handle_end) {
					// Draw the line between the most recently placed anchor and its outgoing handle (which is currently influencing the currently-being-placed segment)
					overlay_context.line(anchor_start, handle_start, None, None);

					// Draw the line between the currently-being-placed anchor and its incoming handle (opposite the one currently being dragged out)
					overlay_context.line(next_anchor, handle_end, None, None);

					if self == PenToolFsmState::PlacingAnchor && anchor_start != handle_start && tool_data.modifiers.lock_angle {
						// Draw the line between the currently-being-placed anchor and last-placed point (lock angle bent overlays)
						overlay_context.dashed_line(anchor_start, next_anchor, None, None, Some(4.), Some(4.), Some(0.5));
					}

					// Draw the line between the currently-being-placed anchor and last-placed point (snap angle bent overlays)
					if self == PenToolFsmState::PlacingAnchor && anchor_start != handle_start && tool_data.modifiers.snap_angle {
						overlay_context.dashed_line(anchor_start, next_anchor, None, None, Some(4.), Some(4.), Some(0.5));
					}

					if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) && valid(next_anchor, handle_end) {
						// Draw the handle circle for the currently-being-dragged-out incoming handle (opposite the one currently being dragged out)
						let selected = tool_data.handle_type == TargetHandle::HandleStart;
						overlay_context.manipulator_handle(handle_end, selected, None);
					}

					if valid(anchor_start, handle_start) {
						// Draw the handle circle for the most recently placed anchor's outgoing handle (which is currently influencing the currently-being-placed segment)
						overlay_context.manipulator_handle(handle_start, false, None);
					}
				} else {
					// Draw the whole path and its manipulators when the user is clicking-and-dragging out from the most recently placed anchor to set its outgoing handle, during which it would otherwise not have its overlays drawn
					match tool_options.pen_overlay_mode {
						PenOverlayMode::AllHandles => {
							path_overlays(document, DrawHandles::All, shape_editor, &mut overlay_context);
						}
						PenOverlayMode::FrontierHandles => {
							path_overlays(document, DrawHandles::None, shape_editor, &mut overlay_context);
						}
					}
				}

				if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) && valid(next_anchor, next_handle_start) {
					// Draw the handle circle for the currently-being-dragged-out outgoing handle (the one currently being dragged out, under the user's cursor)
					let selected = tool_data.handle_type != TargetHandle::HandleStart;
					overlay_context.manipulator_handle(next_handle_start, selected, None);
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

				// Get the closest point and the segment it is on
				tool_data.store_clicked_endpoint(document, &transform, input, preferences);
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
					if let Some(layer) = layer {
						tool_data.buffering_merged_vector = false;
						tool_data.handle_mode = HandleMode::ColinearLocked;
						tool_data.bend_from_previous_point(SnapData::new(document, input), transform, layer, preferences);
						tool_data.place_anchor(SnapData::new(document, input), transform, input.mouse.position, preferences, responses);
					}
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
						let selected_nodes = document.network_interface.selected_nodes();
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
					log::trace!("No latest point available to modify handle_start.");
				}
				self
			}
			(PenToolFsmState::DraggingHandle(_), PenToolMessage::DragStop) => {
				tool_data.cleanup_target_selections(shape_editor, layer, document, responses);
				tool_data.end_point = None;
				tool_data
					.finish_placing_handle(SnapData::new(document, input), transform, preferences, responses)
					.unwrap_or(PenToolFsmState::PlacingAnchor)
			}
			(
				PenToolFsmState::DraggingHandle(_),
				PenToolMessage::PointerMove {
					snap_angle,
					break_handle,
					lock_angle,
					colinear,
					move_anchor_with_handles,
				},
			) => {
				tool_data.modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
					colinear: input.keyboard.key(colinear),
					move_anchor_with_handles: input.keyboard.key(move_anchor_with_handles),
				};
				let snap_data = SnapData::new(document, input);
				if tool_data.modifiers.colinear && !tool_data.toggle_colinear_debounce {
					tool_data.handle_mode = match tool_data.handle_mode {
						HandleMode::Free => HandleMode::ColinearEquidistant,
						HandleMode::ColinearEquidistant | HandleMode::ColinearLocked => HandleMode::Free,
					};
					tool_data.toggle_colinear_debounce = true;
				}

				if tool_data.modifiers.move_anchor_with_handles && !tool_data.space_press {
					let handle_start = layer.map(|layer| document.metadata().transform_to_viewport(layer).transform_point2(tool_data.next_handle_start));
					tool_data.handle_start_offset = handle_start.map(|start| start - input.mouse.position);
					tool_data.space_press = true;
				}

				if !tool_data.modifiers.move_anchor_with_handles {
					tool_data.space_press = false;
				}

				if !tool_data.modifiers.colinear {
					tool_data.toggle_colinear_debounce = false;
				}

				let state = tool_data
					.drag_handle(snap_data, transform, input.mouse.position, responses, layer, input)
					.unwrap_or(PenToolFsmState::Ready);

				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
						move_anchor_with_handles,
					}
					.into(),
					PenToolMessage::PointerMove {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
						move_anchor_with_handles,
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
					move_anchor_with_handles,
				},
			) => {
				tool_data.alt_press = false;
				tool_data.modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
					colinear: input.keyboard.key(colinear),
					move_anchor_with_handles: input.keyboard.key(move_anchor_with_handles),
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
						move_anchor_with_handles,
					}
					.into(),
					PenToolMessage::PointerMove {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
						move_anchor_with_handles,
					}
					.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				state
			}
			(PenToolFsmState::DraggingHandle(_), PenToolMessage::SwapHandles) => {
				let Some(vector_data) = layer.and_then(|layer| document.network_interface.compute_modified_vector(layer)) else {
					return self;
				};
				match tool_data.handle_type {
					TargetHandle::None => {}
					TargetHandle::HandleStart => {
						// The hanlde which will be dragged will be the `next_handle_start` need to check the offset
						tool_data.handle_start_offset = layer.map(|layer| document.metadata().transform_to_viewport(layer).transform_point2(tool_data.next_handle_start) - input.mouse.position);

						tool_data.update_handle_custom(&vector_data);
						tool_data.cleanup_target_selections(shape_editor, layer, document, responses);
					}
					_ => {
						tool_data.add_target_selections(shape_editor, layer);

						// The handle which will be dragged will be the handle end ,need to offset the mouse position
						let viewport = layer.map(|layer| document.metadata().transform_to_viewport(layer));
						let offset = tool_data
							.target_handle_position(tool_data.handle_type, &vector_data)
							.zip(viewport)
							.map(|(opposite, transform)| transform.transform_point2(opposite) - input.mouse.position);
						tool_data.handle_start_offset = layer.map(|layer| document.metadata().transform_to_viewport(layer).transform_point2(tool_data.next_handle_start) - input.mouse.position);
						tool_data.handle_end_offset = offset;
						tool_data.update_handle_type(TargetHandle::HandleStart);
					}
				}
				responses.add(OverlaysMessage::Draw);
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::None });
				self
			}
			(
				PenToolFsmState::Ready,
				PenToolMessage::PointerMove {
					snap_angle,
					break_handle,
					lock_angle,
					colinear,
					move_anchor_with_handles,
				},
			) => {
				tool_data.modifiers = ModifierState {
					snap_angle: input.keyboard.key(snap_angle),
					lock_angle: input.keyboard.key(lock_angle),
					break_handle: input.keyboard.key(break_handle),
					colinear: input.keyboard.key(colinear),
					move_anchor_with_handles: input.keyboard.key(move_anchor_with_handles),
				};
				tool_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(PenToolFsmState::DraggingHandle(mode), PenToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panningF
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
					move_anchor_with_handles,
				},
			) => {
				// Auto-panning
				let messages = [
					PenToolMessage::PointerOutsideViewport {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
						move_anchor_with_handles,
					}
					.into(),
					PenToolMessage::PointerMove {
						snap_angle,
						break_handle,
						lock_angle,
						colinear,
						move_anchor_with_handles,
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
				tool_data.cleanup_target_selections(shape_editor, layer, document, responses);

				PenToolFsmState::Ready
			}
			(_, PenToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.handle_end = None;
				tool_data.latest_points.clear();
				tool_data.point_index = 0;
				tool_data.snap_manager.cleanup(responses);
				tool_data.cleanup_target_selections(shape_editor, layer, document, responses);

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
				HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments"), HintInfo::keys([Key::Control], "Lock Angle")]),
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
						vec![HintInfo::keys([Key::KeyC], "Make Handles Colinear")]
					}
					HandleMode::ColinearLocked | HandleMode::ColinearEquidistant => {
						vec![HintInfo::keys([Key::KeyC], "Break Colinear Handles")]
					}
				};

				let mut common_hints = vec![HintInfo::keys([Key::Shift], "15° Increments"), HintInfo::keys([Key::Control], "Lock Angle")];
				let hold_group = match mode {
					HandleMode::Free => common_hints,
					HandleMode::ColinearLocked => {
						common_hints.push(HintInfo::keys([Key::Alt], "Non-Equidistant Handles"));
						common_hints
					}
					HandleMode::ColinearEquidistant => {
						common_hints.push(HintInfo::keys([Key::Alt], "Equidistant Handles"));
						common_hints
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
