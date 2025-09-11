use super::tool_prelude::*;
use crate::consts::{COLOR_OVERLAY_BLUE, DEFAULT_STROKE_WIDTH, HIDE_HANDLE_DISTANCE, LINE_ROTATE_SNAP_ANGLE, SEGMENT_OVERLAY_SIZE};
use crate::messages::input_mapper::utility_types::input_mouse::MouseKeys;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_overlays;
use crate::messages::portfolio::document::overlays::utility_types::{DrawHandles, OverlayContext};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, merge_layers};
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::common_functionality::snapping::{SnapCache, SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};
use crate::messages::tool::common_functionality::utility_functions::{calculate_segment_angle, closest_point, should_extend};
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::subpath::pathseg_points;
use graphene_std::vector::misc::{HandleId, ManipulatorPointId, dvec2_to_point};
use graphene_std::vector::{NoHashBuilder, PointId, SegmentId, StrokeId, Vector, VectorModificationType};
use kurbo::{CubicBez, PathSeg};

#[derive(Default, ExtractField)]
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
	Overlays {
		context: OverlayContext,
	},

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
	UpdateOptions {
		options: PenOptionsUpdate,
	},
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
		.on_update(|number_input: &NumberInput| {
			PenToolMessage::UpdateOptions {
				options: PenOptionsUpdate::LineWeight(number_input.value.unwrap()),
			}
			.into()
		})
		.widget_holder()
}

impl LayoutHolder for PenTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| {
				PenToolMessage::UpdateOptions {
					options: PenOptionsUpdate::FillColor(None),
				}
				.into()
			},
			|color_type: ToolColorType| {
				WidgetCallback::new(move |_| {
					PenToolMessage::UpdateOptions {
						options: PenOptionsUpdate::FillColorType(color_type.clone()),
					}
					.into()
				})
			},
			|color: &ColorInput| {
				PenToolMessage::UpdateOptions {
					options: PenOptionsUpdate::FillColor(color.value.as_solid().map(|color| color.to_linear_srgb())),
				}
				.into()
			},
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| {
				PenToolMessage::UpdateOptions {
					options: PenOptionsUpdate::StrokeColor(None),
				}
				.into()
			},
			|color_type: ToolColorType| {
				WidgetCallback::new(move |_| {
					PenToolMessage::UpdateOptions {
						options: PenOptionsUpdate::StrokeColorType(color_type.clone()),
					}
					.into()
				})
			},
			|color: &ColorInput| {
				PenToolMessage::UpdateOptions {
					options: PenOptionsUpdate::StrokeColor(color.value.as_solid().map(|color| color.to_linear_srgb())),
				}
				.into()
			},
		));

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.push(create_weight_widget(self.options.line_weight));

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.push(
			RadioInput::new(vec![
				RadioEntryData::new("all")
					.icon("HandleVisibilityAll")
					.tooltip("Show all handles regardless of selection")
					.on_update(move |_| {
						PenToolMessage::UpdateOptions {
							options: PenOptionsUpdate::OverlayModeType(PenOverlayMode::AllHandles),
						}
						.into()
					}),
				RadioEntryData::new("frontier")
					.icon("HandleVisibilityFrontier")
					.tooltip("Show only handles at the frontiers of the segments connected to selected points")
					.on_update(move |_| {
						PenToolMessage::UpdateOptions {
							options: PenOptionsUpdate::OverlayModeType(PenOverlayMode::FrontierHandles),
						}
						.into()
					}),
			])
			.selected_index(Some(self.options.pen_overlay_mode as u32))
			.widget_holder(),
		);

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for PenTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Pen(PenToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
			return;
		};

		match options {
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
			overlay_provider: Some(|context| PenToolMessage::Overlays { context }.into()),
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

/// The type of handle which is dragged by the cursor (under the cursor).
///
/// ![Terminology](https://files.keavon.com/-/EachNotedLovebird/capture.png)
#[derive(Clone, Debug, Default, PartialEq, Copy)]
enum TargetHandle {
	#[default]
	None,
	/// This is the handle being dragged and represents the out handle of the next preview segment that will be placed
	/// after the current preview segment is finalized. Its position is stored in `tool_data.next_handle_start`.
	///
	/// Pressing Tab swaps to the opposite handle type. The swapped handle can be either [`ManipulatorPointId::PreviewInHandle`]
	/// or, in the case of a bent segment, [`ManipulatorPointId::EndHandle`] or [`ManipulatorPointId::PrimaryHandle`].
	///
	/// When closing a path, the handle being dragged becomes the end handle of the currently placed anchor.
	///
	/// ![Terminology](https://files.keavon.com/-/EachNotedLovebird/capture.png)
	FuturePreviewOutHandle,
	/// The opposite handle that is drawn after placing an anchor and starting to drag the "next handle start",
	/// continuing until Tab is pressed to swap the handles.
	///
	/// ![Terminology](https://files.keavon.com/-/EachNotedLovebird/capture.png)
	PreviewInHandle,
	/// This is the primary handle of the segment from whose endpoint a new handle is being drawn.
	/// When closing the path, the handle being dragged will be the [`TargetHandle::PreviewInHandle`] (see its documentation);
	/// otherwise, it will be [`TargetHandle::FuturePreviewOutHandle`].
	///
	/// If a handle is dragged from a different endpoint within the same layer, the opposite handle will be
	/// `ManipulatorPoint::Primary` if that point is the starting point of its path.
	///
	/// ![Terminology](https://files.keavon.com/-/EachNotedLovebird/capture.png)
	PriorOutHandle(SegmentId),
	/// This is the end handle of the segment from whose endpoint a new handle is being drawn (same cases apply
	/// as mentioned in [`TargetHandle::PriorOutHandle`]). If a handle is dragged from a different endpoint within the same
	/// layer, the opposite handle will be `ManipulatorPoint::EndHandle` if that point is the end point of its path.
	///
	/// ![Terminology](https://files.keavon.com/-/EachNotedLovebird/capture.png)
	PriorInHandle(SegmentId),
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

	angle: f64,
	auto_panning: AutoPanning,
	modifiers: ModifierState,

	buffering_merged_vector: bool,

	previous_handle_start_pos: DVec2,
	previous_handle_end_pos: Option<DVec2>,
	toggle_colinear_debounce: bool,
	colinear: bool,
	alt_pressed: bool,
	space_pressed: bool,
	/// Tracks whether to switch from `HandleMode::ColinearEquidistant` to `HandleMode::Free`
	/// after releasing Ctrl, specifically when Ctrl was held before the handle was dragged from the anchor.
	switch_to_free_on_ctrl_release: bool,
	/// To prevent showing cursor when `KeyC` is pressed when handles are swapped.
	handle_swapped: bool,
	/// Prevents conflicts when the handle's angle is already locked and it passes near the anchor,
	/// avoiding unintended direction changes. Specifically handles the case where a handle is being dragged,
	/// and Ctrl is pressed near the anchor to make it colinear with its opposite handle.
	angle_locked: bool,
	path_closed: bool,

	handle_mode: HandleMode,
	prior_segment_layer: Option<LayerNodeIdentifier>,
	current_layer: Option<LayerNodeIdentifier>,
	prior_segment_endpoint: Option<PointId>,
	prior_segment: Option<SegmentId>,

	/// For vector meshes, storing all the previous segments the last anchor point was connected to
	prior_segments: Option<Vec<SegmentId>>,
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

	fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.handle_end = None;
		self.latest_points.clear();
		self.point_index = 0;
		self.snap_manager.cleanup(responses);
	}

	/// Check whether target handle is primary, end, or `self.handle_end`
	fn check_end_handle_type(&self, vector: &Vector) -> TargetHandle {
		match (self.handle_end, self.prior_segment_endpoint, self.prior_segment, self.path_closed) {
			(Some(_), _, _, false) => TargetHandle::PreviewInHandle,
			(None, Some(point), Some(segment), false) | (Some(_), Some(point), Some(segment), true) => {
				if vector.segment_start_from_id(segment) == Some(point) {
					TargetHandle::PriorOutHandle(segment)
				} else {
					TargetHandle::PriorInHandle(segment)
				}
			}
			_ => TargetHandle::None,
		}
	}

	fn check_grs_end_handle(&self, vector: &Vector) -> TargetHandle {
		let Some(point) = self.latest_point().map(|point| point.id) else { return TargetHandle::None };
		let Some(segment) = self.prior_segment else { return TargetHandle::None };

		if vector.segment_start_from_id(segment) == Some(point) {
			TargetHandle::PriorOutHandle(segment)
		} else {
			TargetHandle::PriorInHandle(segment)
		}
	}

	fn get_opposite_handle_type(&self, handle_type: TargetHandle, vector: &Vector) -> TargetHandle {
		match handle_type {
			TargetHandle::FuturePreviewOutHandle => self.check_end_handle_type(vector),
			TargetHandle::PreviewInHandle => match (self.path_closed, self.prior_segment_endpoint, self.prior_segment) {
				(true, Some(point), Some(segment)) => {
					if vector.segment_start_from_id(segment) == Some(point) {
						TargetHandle::PriorOutHandle(segment)
					} else {
						TargetHandle::PriorInHandle(segment)
					}
				}
				(false, _, _) => TargetHandle::FuturePreviewOutHandle,
				_ => TargetHandle::None,
			},
			_ => {
				if self.path_closed {
					TargetHandle::PreviewInHandle
				} else {
					TargetHandle::FuturePreviewOutHandle
				}
			}
		}
	}

	fn update_handle_type(&mut self, handle_type: TargetHandle) {
		self.handle_type = handle_type;
	}

	fn update_target_handle_pos(&mut self, handle_type: TargetHandle, anchor_pos: DVec2, responses: &mut VecDeque<Message>, delta: DVec2, layer: LayerNodeIdentifier) {
		match handle_type {
			TargetHandle::FuturePreviewOutHandle => {
				self.next_handle_start = delta;
			}
			TargetHandle::PreviewInHandle => {
				if let Some(handle) = self.handle_end.as_mut() {
					*handle = delta;
				}
			}
			TargetHandle::PriorInHandle(segment) => {
				let relative_position = delta - anchor_pos;
				let modification_type = VectorModificationType::SetEndHandle { segment, relative_position };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
			TargetHandle::PriorOutHandle(segment) => {
				let relative_position = delta - anchor_pos;
				let modification_type = VectorModificationType::SetPrimaryHandle { segment, relative_position };
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
			TargetHandle::None => {}
		}
	}

	fn target_handle_position(&self, handle_type: TargetHandle, vector: &Vector) -> Option<DVec2> {
		match handle_type {
			TargetHandle::PriorOutHandle(segment) => ManipulatorPointId::PrimaryHandle(segment).get_position(vector),
			TargetHandle::PriorInHandle(segment) => ManipulatorPointId::EndHandle(segment).get_position(vector),
			TargetHandle::PreviewInHandle => self.handle_end,
			TargetHandle::FuturePreviewOutHandle => Some(self.next_handle_start),
			TargetHandle::None => None,
		}
	}

	/// Remove the handles selected when swapping handles
	fn cleanup_target_selections(&self, shape_editor: &mut ShapeState, layer: Option<LayerNodeIdentifier>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(shape_state) = layer.and_then(|layer| shape_editor.selected_shape_state.get_mut(&layer)) else {
			return;
		};

		let Some(vector) = layer.and_then(|layer| document.network_interface.compute_modified_vector(layer)) else {
			return;
		};

		match self.check_end_handle_type(&vector) {
			TargetHandle::PriorInHandle(segment) => shape_state.deselect_point(ManipulatorPointId::EndHandle(segment)),
			TargetHandle::PriorOutHandle(segment) => shape_state.deselect_point(ManipulatorPointId::PrimaryHandle(segment)),
			_ => {}
		}
		responses.add(OverlaysMessage::Draw);
	}

	/// Selects the handle which is currently dragged by the user.
	fn add_target_selections(&self, shape_editor: &mut ShapeState, layer: Option<LayerNodeIdentifier>) {
		let Some(shape_state) = layer.and_then(|layer| shape_editor.selected_shape_state.get_mut(&layer)) else {
			return;
		};

		match self.handle_type {
			TargetHandle::PriorInHandle(segment) => shape_state.select_point(ManipulatorPointId::EndHandle(segment)),
			TargetHandle::PriorOutHandle(segment) => shape_state.select_point(ManipulatorPointId::PrimaryHandle(segment)),
			_ => {}
		}
	}

	/// Check whether moving the initially created point.
	fn moving_start_point(&self) -> bool {
		self.latest_points.len() == 1 && self.latest_point().is_some_and(|point| point.pos == self.next_point)
	}

	// When the vector transform changes, the positions of the points must be recalculated.
	fn recalculate_latest_points_position(&mut self, document: &DocumentMessageHandler) {
		let selected_nodes = document.network_interface.selected_nodes();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		if let (Some(layer), None) = (selected_layers.next(), selected_layers.next()) {
			let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return };
			for point in &mut self.latest_points {
				let Some(pos) = vector.point_domain.position_from_id(point.id) else { continue };
				point.pos = pos;
				point.handle_start = point.pos;
			}
		}
	}

	/// If the user places the anchor on top of the previous anchor, it becomes sharp and the outgoing handle may be dragged.
	fn bend_from_previous_point(
		&mut self,
		snap_data: SnapData,
		transform: DAffine2,
		layer: LayerNodeIdentifier,
		preferences: &PreferencesMessageHandler,
		shape_editor: &mut ShapeState,
		responses: &mut VecDeque<Message>,
	) {
		self.g1_continuous = true;
		let document = snap_data.document;
		self.next_handle_start = self.next_point;
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return };
		self.update_handle_type(TargetHandle::FuturePreviewOutHandle);
		self.handle_mode = HandleMode::ColinearLocked;

		// Break the control
		let Some((last_pos, id)) = self.latest_point().map(|point| (point.pos, point.id)) else { return };

		let transform = document.metadata().document_to_viewport * transform;
		let on_top = transform.transform_point2(self.next_point).distance_squared(transform.transform_point2(last_pos)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2);
		if on_top {
			self.handle_end = None;
			self.handle_mode = HandleMode::Free;

			self.store_clicked_endpoint(document, &transform, snap_data.input, preferences);

			if self.modifiers.lock_angle {
				self.set_lock_angle(&vector, id, self.prior_segment);
				let last_segment = self.prior_segment;
				let Some(point) = self.latest_point_mut() else { return };
				point.in_segment = last_segment;
				self.switch_to_free_on_ctrl_release = true;
				return;
			}

			if let Some(point) = self.latest_point_mut() {
				point.in_segment = None;
			}
		}

		// Closing path
		let closing_path_on_point = self.close_path_on_point(snap_data, &vector, document, preferences, id, &transform);
		if !closing_path_on_point && preferences.vector_meshes {
			// Attempt to find nearest segment and close path on segment by creating an anchor point on it
			let tolerance = crate::consts::SNAP_POINT_TOLERANCE;
			if let Some(closest_segment) = shape_editor.upper_closest_segment(&document.network_interface, transform.transform_point2(self.next_point), tolerance) {
				let (point, _) = closest_segment.adjusted_insert(responses);

				self.update_handle_type(TargetHandle::PreviewInHandle);
				self.handle_end_offset = None;
				self.path_closed = true;
				self.next_handle_start = self.next_point;

				self.prior_segment_endpoint = Some(point);
				self.prior_segment_layer = Some(closest_segment.layer());
				self.prior_segments = None;
				self.prior_segment = None;

				// Should also update the SnapCache here?

				self.handle_mode = HandleMode::Free;
				if let (true, Some(prior_endpoint)) = (self.modifiers.lock_angle, self.prior_segment_endpoint) {
					self.set_lock_angle(&vector, prior_endpoint, self.prior_segment);
					self.switch_to_free_on_ctrl_release = true;
				}
			}
		}
	}

	fn close_path_on_point(&mut self, snap_data: SnapData, vector: &Vector, document: &DocumentMessageHandler, preferences: &PreferencesMessageHandler, id: PointId, transform: &DAffine2) -> bool {
		for id in vector.extendable_points(preferences.vector_meshes).filter(|&point| point != id) {
			let Some(pos) = vector.point_domain.position_from_id(id) else { continue };
			let transformed_distance_between_squared = transform.transform_point2(pos).distance_squared(transform.transform_point2(self.next_point));
			let snap_point_tolerance_squared = crate::consts::SNAP_POINT_TOLERANCE.powi(2);

			if transformed_distance_between_squared < snap_point_tolerance_squared {
				self.update_handle_type(TargetHandle::PreviewInHandle);
				self.handle_end_offset = None;
				self.path_closed = true;
				self.next_handle_start = self.next_point;
				self.store_clicked_endpoint(document, transform, snap_data.input, preferences);
				self.handle_mode = HandleMode::Free;
				if let (true, Some(prior_endpoint)) = (self.modifiers.lock_angle, self.prior_segment_endpoint) {
					self.set_lock_angle(vector, prior_endpoint, self.prior_segment);
					self.switch_to_free_on_ctrl_release = true;
				}
				return true;
			}
		}
		false
	}

	fn finish_placing_handle(&mut self, snap_data: SnapData, transform: DAffine2, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;
		let next_handle_start = self.next_handle_start;
		let handle_start = self.latest_point()?.handle_start;
		let mouse = snap_data.input.mouse.position;
		self.handle_swapped = false;
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
		let layer = selected_layers.next().filter(|_| selected_layers.next().is_none()).or(self.current_layer)?;
		let vector = document.network_interface.compute_modified_vector(layer)?;
		let start = self.latest_point()?.id;
		let transform = document.metadata().document_to_viewport * transform;
		for id in vector.extendable_points(preferences.vector_meshes).filter(|&point| point != start) {
			let Some(pos) = vector.point_domain.position_from_id(id) else { continue };
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
		if self.path_closed {
			if let Some((handles, handle1_pos)) = match self.get_opposite_handle_type(TargetHandle::PreviewInHandle, &vector) {
				TargetHandle::PriorOutHandle(segment) => {
					let handles = [HandleId::end(id), HandleId::primary(segment)];
					let handle1_pos = handles[1].to_manipulator_point().get_position(&vector);
					handle1_pos.map(|pos| (handles, pos))
				}
				TargetHandle::PriorInHandle(segment) => {
					let handles = [HandleId::end(id), HandleId::end(segment)];
					let handle1_pos = handles[1].to_manipulator_point().get_position(&vector);
					handle1_pos.map(|pos| (handles, pos))
				}
				_ => None,
			} {
				let angle = (handle_end - next_point).angle_to(handle1_pos - next_point);
				let pi = std::f64::consts::PI;
				let colinear = (angle - pi).abs() < 1e-6 || (angle + pi).abs() < 1e-6;
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification_type: VectorModificationType::SetG1Continuous { handles, enabled: colinear },
				});
				self.cleanup(responses);
			}
		}

		self.prior_segment = Some(id);

		let points = [start, end];
		let modification_type = VectorModificationType::InsertSegment { id, points, handles };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		// Mirror
		if let Some((last_segment, last_point)) = self.latest_point().and_then(|point| point.in_segment).zip(self.latest_point()) {
			let end = vector.segment_end_from_id(last_segment) == Some(last_point.id);
			let handles = if end {
				[HandleId::end(last_segment), HandleId::primary(id)]
			} else {
				[HandleId::primary(last_segment), HandleId::primary(id)]
			};

			if let Some(h1) = handles[0].to_manipulator_point().get_position(&vector) {
				let angle = (h1 - last_point.pos).angle_to(last_point.handle_start - last_point.pos);
				let pi = std::f64::consts::PI;
				let colinear = (angle - pi).abs() < 1e-6 || (angle + pi).abs() < 1e-6;
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification_type: VectorModificationType::SetG1Continuous { handles, enabled: colinear },
				});
			}
		}
		if !close_subpath {
			self.add_point(LastPoint {
				id: end,
				pos: next_point,
				in_segment: self.g1_continuous.then_some(id),
				handle_start: next_handle_start,
			});
		}
		self.path_closed = false;
		self.prior_segment_endpoint = None;
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
		vector: &Vector,
		input: &InputPreprocessorMessageHandler,
	) -> Option<DVec2> {
		let reference_handle = if self.path_closed { TargetHandle::PreviewInHandle } else { TargetHandle::FuturePreviewOutHandle };
		let end_handle = self.get_opposite_handle_type(reference_handle, vector);
		let end_handle_pos = self.target_handle_position(end_handle, vector);
		let ref_pos = self.target_handle_position(reference_handle, vector)?;
		let snap = &mut self.snap_manager;
		let snap_data = SnapData::new_snap_cache(snap_data.document, input, &self.snap_cache);

		let handle_start_offset = self.handle_start_offset.unwrap_or(DVec2::ZERO);
		let document_pos = viewport_to_document.transform_point2(*mouse + handle_start_offset);

		let anchor_offset = transform.transform_point2(self.next_point - ref_pos);

		let handle_start = SnapCandidatePoint::handle(document_pos);
		let anchor = SnapCandidatePoint::handle(document_pos + anchor_offset);

		let snapped_near_handle_start = snap.free_snap(&snap_data, &handle_start, SnapTypeConfiguration::default());
		let snapped_anchor = snap.free_snap(&snap_data, &anchor, SnapTypeConfiguration::default());

		let handle_snap_option = end_handle_pos.and_then(|handle| match end_handle {
			TargetHandle::None => None,
			TargetHandle::FuturePreviewOutHandle => None,
			_ => {
				let handle_offset = transform.transform_point2(handle - ref_pos);
				let handle_snap = SnapCandidatePoint::handle(document_pos + handle_offset);
				Some((handle, handle_snap))
			}
		});

		let mut delta: DVec2;
		let best_snapped = if snapped_near_handle_start.other_snap_better(&snapped_anchor) {
			delta = snapped_anchor.snapped_point_document - transform.transform_point2(self.next_point);
			snapped_anchor
		} else {
			delta = snapped_near_handle_start.snapped_point_document - transform.transform_point2(ref_pos);
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

	/// Calculates the offset from the mouse when swapping handles, and swaps the handles.
	fn swap_handles(
		&mut self,
		layer: Option<LayerNodeIdentifier>,
		document: &DocumentMessageHandler,
		shape_editor: &mut ShapeState,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) {
		// Validate necessary data exists
		let Some(vector) = layer.and_then(|layer| document.network_interface.compute_modified_vector(layer)) else {
			return;
		};

		let Some(viewport) = layer.map(|layer| document.metadata().transform_to_viewport(layer)) else {
			return;
		};

		// Determine if we need to swap to opposite handle
		let should_swap_to_opposite = self.path_closed && matches!(self.handle_type, TargetHandle::PreviewInHandle | TargetHandle::PriorOutHandle(..) | TargetHandle::PriorInHandle(..))
			|| !self.path_closed && matches!(self.handle_type, TargetHandle::FuturePreviewOutHandle);

		// Determine if we need to swap to start handle
		let should_swap_to_start = !self.path_closed && !matches!(self.handle_type, TargetHandle::None | TargetHandle::FuturePreviewOutHandle);

		if should_swap_to_opposite {
			let opposite_type = self.get_opposite_handle_type(self.handle_type, &vector);
			// Update offset
			let Some(handle_pos) = self.target_handle_position(opposite_type, &vector) else {
				self.handle_swapped = false;
				return;
			};
			if (handle_pos - self.next_point).length() < 1e-6 {
				self.handle_swapped = false;
				return;
			}
			self.handle_end_offset = Some(viewport.transform_point2(handle_pos) - input.mouse.position);

			// Update selections if in closed path mode
			if self.path_closed {
				self.cleanup_target_selections(shape_editor, layer, document, responses);
			}
			self.update_handle_type(opposite_type);
			self.add_target_selections(shape_editor, layer);
		} else if should_swap_to_start {
			self.cleanup_target_selections(shape_editor, layer, document, responses);

			// Calculate offset from mouse position to next handle start
			if let Some(layer_id) = layer {
				let transform = document.metadata().transform_to_viewport(layer_id);
				self.handle_start_offset = Some(transform.transform_point2(self.next_handle_start) - input.mouse.position);
			}

			self.update_handle_type(TargetHandle::FuturePreviewOutHandle);
		}

		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::None });
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

	fn move_anchor_and_handles(&mut self, delta: DVec2, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>, vector: &Vector) {
		if self.handle_end.is_none() {
			if let Some(latest_pt) = self.latest_point_mut() {
				latest_pt.pos += delta;
			}
		}

		let Some(end_point) = self.prior_segment_endpoint else { return };

		let modification_type_anchor = VectorModificationType::ApplyPointDelta { point: end_point, delta };
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: modification_type_anchor,
		});

		let reference_handle = if self.path_closed { TargetHandle::PreviewInHandle } else { TargetHandle::FuturePreviewOutHandle };

		// Move the end handle
		let end_handle_type = self.get_opposite_handle_type(reference_handle, vector);
		match end_handle_type {
			TargetHandle::PriorInHandle(..) | TargetHandle::PriorOutHandle(..) => {
				let Some(handle_pos) = self.target_handle_position(end_handle_type, vector) else { return };
				self.update_target_handle_pos(end_handle_type, self.next_point, responses, handle_pos + delta, layer);
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
		let vector = document.network_interface.compute_modified_vector(layer)?;
		let viewport_to_document = document.metadata().document_to_viewport.inverse();
		let mut mouse_pos = mouse;

		// Handles pressing Space to drag anchor and its handles
		if self.modifiers.move_anchor_with_handles {
			let Some(delta) = self.space_anchor_handle_snap(&viewport_to_document, &transform, &snap_data, &mouse, &vector, input) else {
				return Some(PenToolFsmState::DraggingHandle(self.handle_mode));
			};

			if self.moving_start_point() {
				return self.handle_single_point_path_drag(delta, layer, responses);
			}

			self.next_handle_start += delta;
			self.next_point += delta;

			if let Some(handle) = self.handle_end.as_mut() {
				*handle += delta;
				if !self.path_closed {
					responses.add(OverlaysMessage::Draw);
					return Some(PenToolFsmState::DraggingHandle(self.handle_mode));
				};
			}

			self.move_anchor_and_handles(delta, layer, responses, &vector);

			responses.add(OverlaysMessage::Draw);
			return Some(PenToolFsmState::DraggingHandle(self.handle_mode));
		}

		match self.handle_type {
			TargetHandle::FuturePreviewOutHandle => {
				let offset = self.handle_start_offset.unwrap_or(DVec2::ZERO);
				mouse_pos += offset;
				self.next_handle_start = self.compute_snapped_angle(snap_data.clone(), transform, colinear, mouse_pos, Some(self.next_point), false);
			}
			_ => {
				mouse_pos += self.handle_end_offset.unwrap_or(DVec2::ZERO);
				let mouse_pos = self.compute_snapped_angle(snap_data.clone(), transform, colinear, mouse_pos, Some(self.next_point), false);
				self.update_target_handle_pos(self.handle_type, self.next_point, responses, mouse_pos, layer);
			}
		}

		let mouse_pos = viewport_to_document.transform_point2(mouse_pos);
		let anchor = transform.transform_point2(self.next_point);
		let distance = (mouse_pos - anchor).length();

		if self.switch_to_free_on_ctrl_release && !self.modifiers.lock_angle {
			self.switch_to_free_on_ctrl_release = false;
			self.handle_mode = HandleMode::Free;
		}

		if distance > 20. && self.handle_mode == HandleMode::Free && self.modifiers.lock_angle && !self.angle_locked {
			self.angle_locked = true
		}

		match self.handle_mode {
			HandleMode::ColinearLocked | HandleMode::ColinearEquidistant => {
				self.g1_continuous = true;
				self.apply_colinear_constraint(responses, layer, self.next_point, &vector);
				self.adjust_handle_length(responses, layer, &vector);
			}
			HandleMode::Free => {
				self.g1_continuous = false;
			}
		}

		if distance < 20. && self.handle_mode == HandleMode::Free && self.modifiers.lock_angle && !self.angle_locked {
			let Some(endpoint) = self.prior_segment_endpoint else {
				return Some(PenToolFsmState::DraggingHandle(self.handle_mode));
			};
			self.set_lock_angle(&vector, endpoint, self.prior_segment);
			self.switch_to_free_on_ctrl_release = true;
			let last_segment = self.prior_segment;
			if let Some(latest) = self.latest_point_mut() {
				latest.in_segment = last_segment;
			}
		}

		responses.add(OverlaysMessage::Draw);

		Some(PenToolFsmState::DraggingHandle(self.handle_mode))
	}

	/// Makes the opposite handle equidistant or locks its length.
	fn adjust_handle_length(&mut self, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, vector: &Vector) {
		let opposite_handle_type = self.get_opposite_handle_type(self.handle_type, vector);
		match self.handle_mode {
			HandleMode::ColinearEquidistant => {
				if self.modifiers.break_handle {
					// Store handle for later restoration only when Alt is first pressed
					if !self.alt_pressed {
						self.previous_handle_end_pos = self.target_handle_position(opposite_handle_type, vector);
						self.alt_pressed = true;
					}

					// Set handle to opposite position of the other handle
					let Some(new_position) = self.target_handle_position(self.handle_type, vector).map(|handle| self.next_point * 2. - handle) else {
						return;
					};
					self.update_target_handle_pos(opposite_handle_type, self.next_point, responses, new_position, layer);
				} else if self.alt_pressed {
					// Restore the previous handle position when Alt is released
					if let Some(previous_handle) = self.previous_handle_end_pos {
						self.update_target_handle_pos(opposite_handle_type, self.next_point, responses, previous_handle, layer);
					}
					self.alt_pressed = false;
					self.previous_handle_end_pos = None;
				}
			}
			HandleMode::ColinearLocked => {
				if !self.modifiers.break_handle {
					let Some(new_position) = self.target_handle_position(self.handle_type, vector).map(|handle| self.next_point * 2. - handle) else {
						return;
					};
					self.update_target_handle_pos(opposite_handle_type, self.next_point, responses, new_position, layer);
				}
			}
			HandleMode::Free => {}
		}
	}

	fn apply_colinear_constraint(&mut self, responses: &mut VecDeque<Message>, layer: LayerNodeIdentifier, anchor_pos: DVec2, vector: &Vector) {
		let Some(handle) = self.target_handle_position(self.handle_type, vector) else { return };

		if (anchor_pos - handle).length() < 1e-6 && self.modifiers.lock_angle {
			return;
		}

		let Some(direction) = (anchor_pos - handle).try_normalize() else { return };

		let opposite_handle = self.get_opposite_handle_type(self.handle_type, vector);

		let Some(handle_offset) = self.target_handle_position(opposite_handle, vector).map(|handle| (handle - anchor_pos).length()) else {
			return;
		};

		let new_handle_position = anchor_pos + handle_offset * direction;

		self.update_target_handle_pos(opposite_handle, self.next_point, responses, new_handle_position, layer);
	}

	fn place_anchor(&mut self, snap_data: SnapData, transform: DAffine2, mouse: DVec2, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) -> Option<PenToolFsmState> {
		let document = snap_data.document;

		let relative = if self.path_closed { None } else { self.latest_point().map(|point| point.pos) };
		self.next_point = self.compute_snapped_angle(snap_data, transform, false, mouse, relative, true);

		let selected_nodes = document.network_interface.selected_nodes();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		let layer = selected_layers.next().filter(|_| selected_layers.next().is_none()).or(self.current_layer)?;
		let vector = document.network_interface.compute_modified_vector(layer)?;
		let transform = document.metadata().document_to_viewport * transform;
		for point in vector.extendable_points(preferences.vector_meshes) {
			let Some(pos) = vector.point_domain.position_from_id(point) else { continue };
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

	#[allow(clippy::too_many_arguments)]
	fn create_initial_point(
		&mut self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
		tool_options: &PenOptions,
		append: bool,
		preferences: &PreferencesMessageHandler,
		shape_editor: &mut ShapeState,
	) {
		let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
		let snapped = self.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
		let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);
		self.handle_type = TargetHandle::FuturePreviewOutHandle;

		let selected_nodes = document.network_interface.selected_nodes();
		self.handle_end = None;

		let tolerance = crate::consts::SNAP_POINT_TOLERANCE;
		let extension_choice = should_extend(document, viewport, tolerance, selected_nodes.selected_layers(document.metadata()), preferences);
		if let Some((layer, point, position)) = extension_choice {
			self.current_layer = Some(layer);
			self.extend_existing_path(document, layer, point, position);
			return;
		} else if preferences.vector_meshes {
			if let Some(closest_segment) = shape_editor.upper_closest_segment(&document.network_interface, viewport, tolerance) {
				let (point, segments) = closest_segment.adjusted_insert(responses);
				let layer = closest_segment.layer();
				let position = closest_segment.closest_point_document();

				// Setting any one of the new segments created as the previous segment
				self.prior_segment_endpoint = Some(point);
				self.prior_segment_layer = Some(layer);
				self.prior_segments = Some(segments.to_vec());

				self.extend_existing_path(document, layer, point, position);
				return;
			}
		}

		if append {
			if let Some((layer, point, _)) = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences) {
				let vector = document.network_interface.compute_modified_vector(layer).unwrap();
				let segment = vector.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);

				if self.modifiers.lock_angle {
					self.set_lock_angle(&vector, point, segment);
					self.switch_to_free_on_ctrl_release = true;
				}
			}
			let mut selected_layers_except_artboards = selected_nodes.selected_layers_except_artboards(&document.network_interface);
			let existing_layer = selected_layers_except_artboards.next().filter(|_| selected_layers_except_artboards.next().is_none());
			if let Some(layer) = existing_layer {
				// Add point to existing layer
				responses.add(PenToolMessage::AddPointLayerPosition { layer, viewport });
				return;
			}
		}

		if let Some((layer, point, _position)) = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences) {
			let vector = document.network_interface.compute_modified_vector(layer).unwrap();
			let segment = vector.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);
			self.handle_mode = HandleMode::Free;
			if self.modifiers.lock_angle {
				self.set_lock_angle(&vector, point, segment);
				self.switch_to_free_on_ctrl_release = true;
			}
		}

		// New path layer
		let node_type = resolve_document_node_type("Path").expect("Path node does not exist");
		let nodes = vec![(NodeId(0), node_type.default_node_template())];

		let parent = document.new_layer_bounding_artboard(input);
		let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
		self.current_layer = Some(layer);
		tool_options.fill.apply_fill(layer, responses);
		tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);
		self.prior_segment = None;
		self.prior_segments = None;
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });

		// It is necessary to defer this until the transform of the layer can be accurately computed (quite hacky)
		responses.add(DeferMessage::AfterGraphRun {
			messages: vec![PenToolMessage::AddPointLayerPosition { layer, viewport }.into()],
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Perform extension of an existing path
	fn extend_existing_path(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier, point: PointId, position: DVec2) {
		let vector = document.network_interface.compute_modified_vector(layer);
		let (handle_start, in_segment) = if let Some(vector) = &vector {
			vector
				.segment_iter()
				.find_map(|(segment_id, bezier, start, end)| {
					let is_end = point == end;
					let is_start = point == start;
					if !is_end && !is_start {
						return None;
					}

					let points = pathseg_points(bezier);
					let handle = match (points.p1, points.p2) {
						(Some(p1), Some(_)) if is_start => p1,
						(Some(_), Some(p2)) if !is_start => p2,
						(Some(p1), None) | (None, Some(p1)) => p1,
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

		let in_segment = if self.modifiers.lock_angle { self.prior_segment } else { in_segment };

		self.add_point(LastPoint {
			id: point,
			pos: position,
			in_segment,
			handle_start,
		});

		self.next_point = position;
		self.next_handle_start = handle_start;
		let vector = document.network_interface.compute_modified_vector(layer).unwrap();
		let segment = vector.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);
		self.handle_mode = HandleMode::Free;

		if self.modifiers.lock_angle {
			self.set_lock_angle(&vector, point, segment);
			self.switch_to_free_on_ctrl_release = true;
		}
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
		self.prior_segment = None;
		self.prior_segment_endpoint = None;
		self.prior_segment_layer = None;
		self.prior_segments = None;

		if let Some((layer, point, _position)) = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences) {
			self.prior_segment_endpoint = Some(point);
			self.prior_segment_layer = Some(layer);
			let vector = document.network_interface.compute_modified_vector(layer).unwrap();
			let segment = vector.all_connected(point).collect::<Vec<_>>().first().map(|s| s.segment);
			self.prior_segment = segment;
			layer_manipulators.insert(point);
			for (&id, &position) in vector.point_domain.ids().iter().zip(vector.point_domain.positions()) {
				if id == point {
					continue;
				}
				unselected.push(SnapCandidatePoint::handle(transform.transform_point2(position)))
			}
			manipulators.insert(layer, layer_manipulators);
			self.snap_cache = SnapCache { manipulators, unselected }
		}
	}

	fn set_lock_angle(&mut self, vector: &Vector, anchor: PointId, segment: Option<SegmentId>) {
		let anchor_position = vector.point_domain.position_from_id(anchor);

		let Some((anchor_position, segment)) = anchor_position.zip(segment) else {
			self.handle_mode = HandleMode::Free;
			return;
		};

		match (self.handle_type, self.path_closed) {
			(TargetHandle::FuturePreviewOutHandle, _) | (TargetHandle::PreviewInHandle, true) => {
				if let Some(required_handle) = calculate_segment_angle(anchor, segment, vector, true) {
					self.angle = required_handle;
					self.handle_mode = HandleMode::ColinearEquidistant;
				}
			}
			(TargetHandle::PriorInHandle(..) | TargetHandle::PriorOutHandle(..), true) => {
				self.angle = -(self.handle_end.unwrap() - anchor_position).angle_to(DVec2::X);
				self.handle_mode = HandleMode::ColinearEquidistant;
			}
			_ => {
				self.angle = -(self.next_handle_start - anchor_position).angle_to(DVec2::X);
				self.handle_mode = HandleMode::ColinearEquidistant;
			}
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

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document,
			global_tool_data,
			input,
			shape_editor,
			preferences,
			..
		} = tool_action_data;
		let selected_nodes = document.network_interface.selected_nodes();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		let layer = selected_layers.next().filter(|_| selected_layers.next().is_none()).or(tool_data.current_layer);

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

				let latest_pos = latest.pos;
				let latest_handle_start = latest.handle_start;

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

				let vector = document.network_interface.compute_modified_vector(layer).unwrap();
				tool_data.previous_handle_start_pos = latest.handle_start;
				let opposite_handle = tool_data.check_grs_end_handle(&vector);
				tool_data.previous_handle_end_pos = tool_data.target_handle_position(opposite_handle, &vector);
				let handle1 = latest_handle_start - latest_pos;
				let Some(opposite_handle_pos) = tool_data.target_handle_position(opposite_handle, &vector) else {
					return PenToolFsmState::GRSHandle;
				};
				let handle2 = opposite_handle_pos - latest_pos;
				let pi = std::f64::consts::PI;
				let angle = handle1.angle_to(handle2);
				tool_data.colinear = (angle - pi).abs() < 1e-6 || (angle + pi).abs() < 1e-6;
				PenToolFsmState::GRSHandle
			}
			(PenToolFsmState::GRSHandle, PenToolMessage::FinalPosition { final_position }) => {
				let Some(layer) = layer else { return PenToolFsmState::GRSHandle };
				let vector = document.network_interface.compute_modified_vector(layer);
				let Some(vector) = vector else { return PenToolFsmState::GRSHandle };

				if let Some(latest_pt) = tool_data.latest_point_mut() {
					let layer_space_to_viewport = document.metadata().transform_to_viewport(layer);
					let final_pos = layer_space_to_viewport.inverse().transform_point2(final_position);
					latest_pt.handle_start = final_pos;
				}

				responses.add(OverlaysMessage::Draw);
				let Some(latest) = tool_data.latest_point() else {
					return PenToolFsmState::GRSHandle;
				};
				let opposite_handle = tool_data.check_grs_end_handle(&vector);
				let Some(opposite_handle_pos) = tool_data.target_handle_position(opposite_handle, &vector) else {
					return PenToolFsmState::GRSHandle;
				};

				if tool_data.colinear {
					let Some(direction) = (latest.pos - latest.handle_start).try_normalize() else {
						return PenToolFsmState::GRSHandle;
					};

					if (latest.pos - latest.handle_start).length_squared() < f64::EPSILON {
						return PenToolFsmState::GRSHandle;
					}
					let relative_distance = (opposite_handle_pos - latest.pos).length();
					let relative_position = relative_distance * direction + latest.pos;
					tool_data.update_target_handle_pos(opposite_handle, latest.pos, responses, relative_position, layer);
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
				let vector = document.network_interface.compute_modified_vector(layer).unwrap();
				let opposite_handle = tool_data.check_grs_end_handle(&vector);

				let previous = tool_data.previous_handle_start_pos;
				if let Some(latest) = tool_data.latest_point_mut() {
					latest.handle_start = previous;
				} else {
					return PenToolFsmState::PlacingAnchor;
				}

				responses.add(OverlaysMessage::Draw);
				responses.add(PenToolMessage::PointerMove {
					snap_angle: Key::Control,
					break_handle: Key::Alt,
					lock_angle: Key::Shift,
					colinear: Key::KeyC,
					move_anchor_with_handles: Key::Space,
				});

				let Some((previous_pos, latest)) = tool_data.previous_handle_end_pos.zip(tool_data.latest_point()) else {
					return PenToolFsmState::PlacingAnchor;
				};
				tool_data.update_target_handle_pos(opposite_handle, latest.pos, responses, previous_pos, layer);

				PenToolFsmState::PlacingAnchor
			}
			(_, PenToolMessage::SelectionChanged) => {
				responses.add(OverlaysMessage::Draw);
				self
			}
			(PenToolFsmState::Ready, PenToolMessage::Overlays { context: mut overlay_context }) => {
				match tool_options.pen_overlay_mode {
					PenOverlayMode::AllHandles => {
						path_overlays(document, DrawHandles::All, shape_editor, &mut overlay_context);
					}
					PenOverlayMode::FrontierHandles => {
						path_overlays(document, DrawHandles::None, shape_editor, &mut overlay_context);
					}
				}
				// Check if there is an anchor within threshold
				// If not check if there is a closest segment within threshold, if yes then draw overlay
				let tolerance = crate::consts::SNAP_POINT_TOLERANCE;
				let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
				let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
				let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);

				let close_to_point = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences).is_some();
				if preferences.vector_meshes && !close_to_point {
					if let Some(closest_segment) = shape_editor.upper_closest_segment(&document.network_interface, viewport, tolerance) {
						let pos = closest_segment.closest_point_to_viewport();
						let perp = closest_segment.calculate_perp(document);
						overlay_context.manipulator_anchor(pos, true, None);
						overlay_context.line(pos - perp * SEGMENT_OVERLAY_SIZE, pos + perp * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), None);
					}
				}
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				self
			}
			(_, PenToolMessage::Overlays { context: mut overlay_context }) => {
				let display_anchors = overlay_context.visibility_settings.anchors();
				let display_handles = overlay_context.visibility_settings.handles();

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
					let end = tool_data.next_point;
					let bezier = PathSeg::Cubic(CubicBez::new(dvec2_to_point(start), dvec2_to_point(handle_start), dvec2_to_point(handle_end), dvec2_to_point(end)));
					if (end - start).length_squared() > f64::EPSILON {
						// Draw the curve for the currently-being-placed segment
						overlay_context.outline_bezier(bezier, transform);
					}
				}

				if display_handles {
					// Draw the line between the currently-being-placed anchor and its currently-being-dragged-out outgoing handle (opposite the one currently being dragged out)
					overlay_context.line(next_anchor, next_handle_start, None, None);
				}

				match tool_options.pen_overlay_mode {
					PenOverlayMode::AllHandles => {
						path_overlays(document, DrawHandles::All, shape_editor, &mut overlay_context);
					}
					PenOverlayMode::FrontierHandles => {
						if let Some(layer) = tool_data.current_layer {
							if let Some(latest_segment) = tool_data.prior_segment {
								let selected_anchors_data = HashMap::from([(layer, vec![latest_segment])]);
								path_overlays(document, DrawHandles::SelectedAnchors(selected_anchors_data), shape_editor, &mut overlay_context);
							}
							// If a vector mesh then there can be more than one prior segments
							else if let Some(segments) = tool_data.prior_segments.clone() {
								if preferences.vector_meshes {
									let selected_anchors_data = HashMap::from([(layer, segments)]);
									path_overlays(document, DrawHandles::SelectedAnchors(selected_anchors_data), shape_editor, &mut overlay_context);
								}
							} else {
								path_overlays(document, DrawHandles::None, shape_editor, &mut overlay_context);
							};
						}
					}
				}

				if let (Some(anchor_start), Some(handle_start), Some(handle_end)) = (anchor_start, handle_start, handle_end) {
					if display_handles {
						// Draw the line between the most recently placed anchor and its outgoing handle (which is currently influencing the currently-being-placed segment)
						overlay_context.line(anchor_start, handle_start, None, None);

						// Draw the line between the currently-being-placed anchor and its incoming handle (opposite the one currently being dragged out)
						overlay_context.line(next_anchor, handle_end, None, None);
					}

					if self == PenToolFsmState::PlacingAnchor && anchor_start != handle_start && tool_data.modifiers.lock_angle {
						// Draw the line between the currently-being-placed anchor and last-placed point (lock angle bent overlays)
						overlay_context.dashed_line(anchor_start, next_anchor, None, None, Some(4.), Some(4.), Some(0.5));
					}

					// Draw the line between the currently-being-placed anchor and last-placed point (snap angle bent overlays)
					if self == PenToolFsmState::PlacingAnchor && anchor_start != handle_start && tool_data.modifiers.snap_angle {
						overlay_context.dashed_line(anchor_start, next_anchor, None, None, Some(4.), Some(4.), Some(0.5));
					}

					if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) && valid(next_anchor, handle_end) && display_handles {
						// Draw the handle circle for the currently-being-dragged-out incoming handle (opposite the one currently being dragged out)
						let selected = tool_data.handle_type == TargetHandle::PreviewInHandle;
						if display_handles {
							overlay_context.manipulator_handle(handle_end, selected, None);
							overlay_context.manipulator_handle(handle_end, selected, None);
						}
					}

					if valid(anchor_start, handle_start) && display_handles {
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

				if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) && valid(next_anchor, next_handle_start) && display_handles {
					// Draw the handle circle for the currently-being-dragged-out outgoing handle (the one currently being dragged out, under the user's cursor)
					let selected = tool_data.handle_type == TargetHandle::FuturePreviewOutHandle;
					overlay_context.manipulator_handle(next_handle_start, selected, None);
				}

				if self == PenToolFsmState::DraggingHandle(tool_data.handle_mode) && display_anchors {
					// Draw the anchor square for the most recently placed anchor
					overlay_context.manipulator_anchor(next_anchor, false, None);
				}

				if self == PenToolFsmState::PlacingAnchor && preferences.vector_meshes {
					let tolerance = crate::consts::SNAP_POINT_TOLERANCE;
					let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
					let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
					let viewport = document.metadata().document_to_viewport.transform_point2(snapped.snapped_point_document);
					let close_to_point = closest_point(document, viewport, tolerance, document.metadata().all_layers(), |_| false, preferences).is_some();
					if !close_to_point {
						if let Some(closest_segment) = shape_editor.upper_closest_segment(&document.network_interface, viewport, tolerance) {
							let pos = closest_segment.closest_point_to_viewport();
							let perp = closest_segment.calculate_perp(document);
							overlay_context.manipulator_anchor(pos, true, None);
							overlay_context.line(pos - perp * SEGMENT_OVERLAY_SIZE, pos + perp * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), None);
						}
					}
				}

				// Display a filled overlay of the shape if the new point closes the path
				if let Some(latest_point) = tool_data.latest_point() {
					let handle_start = latest_point.handle_start;
					let handle_end = tool_data.handle_end.unwrap_or(tool_data.next_handle_start);
					let next_point = tool_data.next_point;
					let start = latest_point.id;

					if let Some(layer) = layer
						&& let Some(mut vector) = document.network_interface.compute_modified_vector(layer)
					{
						let closest_point = vector.extendable_points(preferences.vector_meshes).filter(|&id| id != start).find(|&id| {
							vector.point_domain.position_from_id(id).is_some_and(|pos| {
								let dist_sq = transform.transform_point2(pos).distance_squared(transform.transform_point2(next_point));
								dist_sq < crate::consts::SNAP_POINT_TOLERANCE.powi(2)
							})
						});

						// We have the point. Join the 2 vertices and check if any path is closed.
						if let Some(end) = closest_point {
							let segment_id = SegmentId::generate();
							vector.push(segment_id, start, end, (Some(handle_start), Some(handle_end)), StrokeId::ZERO);

							let grouped_segments = vector.auto_join_paths();
							let closed_paths = grouped_segments.iter().filter(|path| path.is_closed() && path.contains(segment_id));

							let subpaths: Vec<_> = closed_paths
								.filter_map(|path| {
									let segments = path.edges.iter().filter_map(|edge| {
										vector
											.segment_domain
											.iter()
											.find(|(id, _, _, _)| id == &edge.id)
											.map(|(_, start, end, bezier)| if start == edge.start { (bezier, start, end) } else { (bezier.reversed(), end, start) })
									});
									vector.subpath_from_segments_ignore_discontinuities(segments)
								})
								.collect();

							let mut fill_color = graphene_std::Color::from_rgb_str(COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap())
								.unwrap()
								.with_alpha(0.05)
								.to_rgba_hex_srgb();
							fill_color.insert(0, '#');
							overlay_context.fill_path(subpaths.iter(), transform, fill_color.as_str());
						}
					}
				}

				// Draw the overlays that visualize current snapping
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

				self
			}
			(_, PenToolMessage::WorkingColorChanged) => {
				responses.add(PenToolMessage::UpdateOptions {
					options: PenOptionsUpdate::WorkingColors(Some(global_tool_data.primary_color), Some(global_tool_data.secondary_color)),
				});
				self
			}
			(PenToolFsmState::Ready, PenToolMessage::DragStart { append_to_selected }) => {
				responses.add(DocumentMessage::StartTransaction);
				tool_data.handle_mode = HandleMode::Free;

				// Get the closest point and the segment it is on
				let append = input.keyboard.key(append_to_selected);

				tool_data.store_clicked_endpoint(document, &transform, input, preferences);
				tool_data.create_initial_point(document, input, responses, tool_options, append, preferences, shape_editor);

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
						tool_data.bend_from_previous_point(SnapData::new(document, input), transform, layer, preferences, shape_editor, responses);
						tool_data.place_anchor(SnapData::new(document, input), transform, input.mouse.position, preferences, responses);
					}
					tool_data.buffering_merged_vector = false;
					PenToolFsmState::DraggingHandle(tool_data.handle_mode)
				} else {
					if tool_data.handle_end.is_some() {
						responses.add(DocumentMessage::StartTransaction);
					}
					// Merge two layers if the point is connected to the end point of another path

					// This might not be the correct solution to artboards being included as the other layer,
					// which occurs due to the `compute_modified_vector` call in `should_extend` using the click targets for a layer instead of vector.
					let layers = LayerNodeIdentifier::ROOT_PARENT
						.descendants(document.metadata())
						.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));
					if let Some((other_layer, _, _)) = should_extend(document, viewport, crate::consts::SNAP_POINT_TOLERANCE, layers, preferences) {
						let selected_nodes = document.network_interface.selected_nodes();
						let mut selected_layers = selected_nodes.selected_layers(document.metadata());
						if let Some(current_layer) = selected_layers
							.next()
							.filter(|current_layer| selected_layers.next().is_none() && *current_layer != other_layer)
							.or(tool_data.current_layer.filter(|layer| *layer != other_layer))
						{
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
						HandleMode::Free => {
							let last_segment = tool_data.prior_segment;
							if let Some(latest) = tool_data.latest_point_mut() {
								latest.in_segment = last_segment;
							}
							HandleMode::ColinearEquidistant
						}
						HandleMode::ColinearEquidistant | HandleMode::ColinearLocked => HandleMode::Free,
					};
					tool_data.toggle_colinear_debounce = true;
				}

				let Some(vector) = layer.and_then(|layer| document.network_interface.compute_modified_vector(layer)) else {
					return self;
				};

				if tool_data.modifiers.move_anchor_with_handles && !tool_data.space_pressed {
					let reference_handle = if tool_data.path_closed {
						TargetHandle::PreviewInHandle
					} else {
						TargetHandle::FuturePreviewOutHandle
					};
					let handle_start = layer.map(|layer| {
						document
							.metadata()
							.transform_to_viewport(layer)
							.transform_point2(tool_data.target_handle_position(reference_handle, &vector).unwrap())
					});
					tool_data.handle_start_offset = handle_start.map(|start| start - input.mouse.position);
					tool_data.space_pressed = true;
				}

				if !tool_data.modifiers.move_anchor_with_handles {
					tool_data.space_pressed = false;
				}

				if !tool_data.modifiers.colinear {
					tool_data.toggle_colinear_debounce = false;
				}

				if !tool_data.modifiers.lock_angle {
					tool_data.angle_locked = false;
				}

				let state = tool_data
					.drag_handle(snap_data, transform, input.mouse.position, responses, layer, input)
					.unwrap_or(PenToolFsmState::Ready);

				if tool_data.handle_swapped {
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::None });
				}

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
				tool_data.switch_to_free_on_ctrl_release = false;
				tool_data.alt_pressed = false;
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
				if !tool_data.handle_swapped {
					tool_data.handle_swapped = true
				}
				tool_data.swap_handles(layer, document, shape_editor, input, responses);
				responses.add(OverlaysMessage::Draw);
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
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				PenToolFsmState::DraggingHandle(mode)
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerOutsideViewport { .. }) => {
				if !input.mouse.mouse_keys.contains(MouseKeys::LEFT) {
					return self;
				}
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
			(PenToolFsmState::DraggingHandle(..), PenToolMessage::Confirm) => {
				// Confirm to end path
				if let Some((vector, layer)) = layer.and_then(|layer| document.network_interface.compute_modified_vector(layer)).zip(layer) {
					let single_point_in_layer = vector.point_domain.ids().len() == 1;
					tool_data.finish_placing_handle(SnapData::new(document, input), transform, preferences, responses);
					let latest_points = tool_data.latest_points.len() == 1;

					if latest_points && single_point_in_layer {
						responses.add(NodeGraphMessage::DeleteNodes {
							node_ids: vec![layer.to_node()],
							delete_children: true,
						});
						responses.add(NodeGraphMessage::RunDocumentGraph);
					} else if (latest_points && tool_data.prior_segment_endpoint.is_none())
						|| (tool_data.prior_segment_endpoint.is_some() && tool_data.prior_segment_layer != Some(layer) && latest_points)
					{
						let vector_modification = VectorModificationType::RemovePoint {
							id: tool_data.latest_point().unwrap().id,
						};
						responses.add(GraphOperationMessage::Vector {
							layer,
							modification_type: vector_modification,
						});
						responses.add(PenToolMessage::Abort);
					} else {
						responses.add(DocumentMessage::EndTransaction);
					}
				}
				tool_data.cleanup(responses);
				tool_data.cleanup_target_selections(shape_editor, layer, document, responses);

				responses.add(OverlaysMessage::Draw);

				PenToolFsmState::Ready
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::Confirm) => {
				responses.add(DocumentMessage::EndTransaction);
				tool_data.cleanup(responses);
				tool_data.cleanup_target_selections(shape_editor, layer, document, responses);

				PenToolFsmState::Ready
			}
			(PenToolFsmState::DraggingHandle(..), PenToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				if tool_data.handle_end.is_none() {
					tool_data.cleanup(responses);
					tool_data.cleanup_target_selections(shape_editor, layer, document, responses);

					PenToolFsmState::Ready
				} else {
					tool_data
						.place_anchor(SnapData::new(document, input), transform, input.mouse.position, preferences, responses)
						.unwrap_or(PenToolFsmState::Ready)
				}
			}
			(PenToolFsmState::PlacingAnchor, PenToolMessage::Abort) => {
				let should_delete_layer = if let Some(vector) = layer.and_then(|layer| document.network_interface.compute_modified_vector(layer)) {
					vector.point_domain.ids().len() == 1
				} else {
					false
				};

				responses.add(DocumentMessage::AbortTransaction);
				tool_data.cleanup(responses);
				tool_data.cleanup_target_selections(shape_editor, layer, document, responses);

				if should_delete_layer {
					responses.add(NodeGraphMessage::DeleteNodes {
						node_ids: vec![layer.unwrap().to_node()],
						delete_children: true,
					});
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
				responses.add(OverlaysMessage::Draw);

				PenToolFsmState::Ready
			}
			(_, PenToolMessage::Abort) => PenToolFsmState::Ready,
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
				HintGroup(vec![
					HintInfo::mouse(MouseMotion::Lmb, ""),
					HintInfo::mouse(MouseMotion::LmbDrag, "Bend Prev. Point").prepend_slash(),
					HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
				]),
			]),
			PenToolFsmState::DraggingHandle(mode) => {
				let mut dragging_hint_data = HintData(Vec::new());
				dragging_hint_data.0.push(HintGroup(vec![
					HintInfo::mouse(MouseMotion::Rmb, ""),
					HintInfo::keys([Key::Escape], "Cancel Segment").prepend_slash(),
					HintInfo::keys([Key::Enter], "End Path"),
				]));

				let mut toggle_group = match mode {
					HandleMode::Free => {
						vec![HintInfo::keys([Key::KeyC], "Make Handles Colinear")]
					}
					HandleMode::ColinearLocked | HandleMode::ColinearEquidistant => {
						vec![HintInfo::keys([Key::KeyC], "Break Colinear Handles")]
					}
				};
				toggle_group.push(HintInfo::keys([Key::Tab], "Swap Dragged Handle"));

				let mut common_hints = vec![HintInfo::keys([Key::Shift], "15° Increments"), HintInfo::keys([Key::Control], "Lock Angle")];
				let mut hold_group = match mode {
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
				hold_group.push(HintInfo::keys([Key::Space], "Drag Anchor"));

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
