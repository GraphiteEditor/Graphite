use super::select_tool::extend_lasso;
use super::tool_prelude::*;
use crate::consts::{
	COLOR_OVERLAY_BLUE, COLOR_OVERLAY_GRAY, COLOR_OVERLAY_GREEN, COLOR_OVERLAY_RED, DEFAULT_STROKE_WIDTH, DOUBLE_CLICK_MILLISECONDS, DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD, DRAG_THRESHOLD,
	DRILL_THROUGH_THRESHOLD, HANDLE_ROTATE_SNAP_ANGLE, SEGMENT_INSERTION_DISTANCE, SEGMENT_OVERLAY_SIZE, SELECTION_THRESHOLD, SELECTION_TOLERANCE,
};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::{path_overlays, selected_segments};
use crate::messages::portfolio::document::overlays::utility_types::{DrawHandles, OverlayContext};
use crate::messages::portfolio::document::utility_types::clipboards::Clipboard;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::transformation::Axis;
use crate::messages::preferences::SelectionMode;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::pivot::{PivotGizmo, PivotGizmoType, PivotToolSource, pin_pivot_widget, pivot_gizmo_type_widget, pivot_reference_point_widget};
use crate::messages::tool::common_functionality::shape_editor::{
	ClosestSegment, ManipulatorAngle, OpposingHandleLengths, SelectedLayerState, SelectedPointsInfo, SelectionChange, SelectionShape, SelectionShapeType, ShapeState,
};
use crate::messages::tool::common_functionality::snapping::{SnapCache, SnapCandidatePoint, SnapConstraint, SnapData, SnapManager};
use crate::messages::tool::common_functionality::utility_functions::{calculate_segment_angle, find_two_param_best_approximate, make_path_editable_is_allowed};
use graphene_std::Color;
use graphene_std::renderer::Quad;
use graphene_std::subpath::pathseg_points;
use graphene_std::transform::ReferencePoint;
use graphene_std::uuid::NodeId;
use graphene_std::vector::algorithms::util::pathseg_tangent;
use graphene_std::vector::click_target::ClickTargetType;
use graphene_std::vector::misc::{HandleId, ManipulatorPointId, dvec2_to_point, point_to_dvec2};
use graphene_std::vector::{HandleExt, NoHashBuilder, PointId, SegmentId, Vector, VectorModificationType};
use kurbo::{DEFAULT_ACCURACY, ParamCurve, ParamCurveNearest, PathSeg, Rect};
use std::vec;

#[derive(Default, ExtractField)]
pub struct PathTool {
	fsm_state: PathToolFsmState,
	tool_data: PathToolData,
	options: PathToolOptions,
}

#[derive(Default)]
pub struct PathToolOptions {
	path_overlay_mode: PathOverlayMode,
	path_editing_mode: PathEditingMode,
}

#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PathToolMessage {
	// Standard messages
	Abort,
	SelectionChanged,
	Overlays {
		context: OverlayContext,
	},

	// Tool-specific messages
	BreakPath,
	DeselectAllSelected,
	Delete,
	DeleteAndBreakPath,
	DragStop {
		extend_selection: Key,
		shrink_selection: Key,
	},
	Enter {
		extend_selection: Key,
		shrink_selection: Key,
	},
	Escape,
	ClosePath,
	DoubleClick {
		extend_selection: Key,
		shrink_selection: Key,
	},
	GRS {
		// Should be `Key::KeyG` (Grab), `Key::KeyR` (Rotate), or `Key::KeyS` (Scale)
		key: Key,
	},
	ManipulatorMakeHandlesFree,
	ManipulatorMakeHandlesColinear,
	MouseDown {
		extend_selection: Key,
		lasso_select: Key,
		handle_drag_from_anchor: Key,
		drag_restore_handle: Key,
		segment_editing_modifier: Key,
	},
	NudgeSelectedPoints {
		delta_x: f64,
		delta_y: f64,
	},
	PointerMove {
		equidistant: Key,
		toggle_colinear: Key,
		move_anchor_with_handles: Key,
		snap_angle: Key,
		lock_angle: Key,
		delete_segment: Key,
		break_colinear_molding: Key,
		segment_editing_modifier: Key,
	},
	PointerOutsideViewport {
		equidistant: Key,
		toggle_colinear: Key,
		move_anchor_with_handles: Key,
		snap_angle: Key,
		lock_angle: Key,
		delete_segment: Key,
		break_colinear_molding: Key,
		segment_editing_modifier: Key,
	},
	RightClick,
	SelectAll,
	SelectedPointUpdated,
	SelectedPointXChanged {
		new_x: f64,
	},
	SelectedPointYChanged {
		new_y: f64,
	},
	SetPivot {
		position: ReferencePoint,
	},
	SwapSelectedHandles,
	UpdateOptions {
		options: PathOptionsUpdate,
	},
	UpdateSelectedPointsStatus {
		overlay_context: OverlayContext,
	},
	StartSlidingPoint,
	Copy {
		clipboard: Clipboard,
	},
	Cut {
		clipboard: Clipboard,
	},
	Paste {
		data: String,
	},
	DeleteSelected,
	Duplicate,
	TogglePointEditing,
	ToggleSegmentEditing,
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PathOverlayMode {
	AllHandles = 0,
	#[default]
	SelectedPointHandles = 1,
	FrontierHandles = 2,
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct PathEditingMode {
	point_editing_mode: bool,
	segment_editing_mode: bool,
}

impl Default for PathEditingMode {
	fn default() -> Self {
		Self {
			point_editing_mode: true,
			segment_editing_mode: false,
		}
	}
}

#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum PathOptionsUpdate {
	OverlayModeType(PathOverlayMode),
	PointEditingMode { enabled: bool },
	SegmentEditingMode { enabled: bool },
	PivotGizmoType(PivotGizmoType),
	TogglePivotGizmoType(bool),
	TogglePivotPinned,
}

impl ToolMetadata for PathTool {
	fn icon_name(&self) -> String {
		"VectorPathTool".into()
	}
	fn tooltip(&self) -> String {
		"Path Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Path
	}
}

impl LayoutHolder for PathTool {
	fn layout(&self) -> Layout {
		let coordinates = self.tool_data.selection_status.as_one().as_ref().map(|point| point.coordinates);
		let (x, y) = coordinates.map(|point| (Some(point.x), Some(point.y))).unwrap_or((None, None));

		let selection_status = &self.tool_data.selection_status;
		let manipulator_angle = selection_status.angle();

		let x_location = NumberInput::new(x)
			.unit(" px")
			.label("X")
			.min_width(120)
			.disabled(x.is_none())
			.min(-((1_u64 << f64::MANTISSA_DIGITS) as f64))
			.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				if let Some(new_x) = number_input.value.or(x) {
					PathToolMessage::SelectedPointXChanged { new_x }.into()
				} else {
					Message::NoOp
				}
			})
			.widget_holder();

		let y_location = NumberInput::new(y)
			.unit(" px")
			.label("Y")
			.min_width(120)
			.disabled(y.is_none())
			.min(-((1_u64 << f64::MANTISSA_DIGITS) as f64))
			.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				if let Some(new_y) = number_input.value.or(y) {
					PathToolMessage::SelectedPointYChanged { new_y }.into()
				} else {
					Message::NoOp
				}
			})
			.widget_holder();

		let related_seperator = Separator::new(SeparatorType::Related).widget_holder();
		let unrelated_seperator = Separator::new(SeparatorType::Unrelated).widget_holder();

		let colinear_handles_tooltip = "Keep both handles unbent, each 180° apart, when moving either";
		let colinear_handles_state = manipulator_angle.and_then(|angle| match angle {
			ManipulatorAngle::Colinear => Some(true),
			ManipulatorAngle::Free => Some(false),
			ManipulatorAngle::Mixed => None,
		})
		// TODO: Remove `unwrap_or_default` once checkboxes are capable of displaying a mixed state
		.unwrap_or_default();
		let checkbox_id = CheckboxId::new();
		let colinear_handle_checkbox = CheckboxInput::new(colinear_handles_state)
			.disabled(!self.tool_data.can_toggle_colinearity)
			.on_update(|&CheckboxInput { checked, .. }| {
				if checked {
					PathToolMessage::ManipulatorMakeHandlesColinear.into()
				} else {
					PathToolMessage::ManipulatorMakeHandlesFree.into()
				}
			})
			.tooltip(colinear_handles_tooltip)
			.for_label(checkbox_id)
			.widget_holder();
		let colinear_handles_label = TextLabel::new("Colinear Handles")
			.disabled(!self.tool_data.can_toggle_colinearity)
			.tooltip(colinear_handles_tooltip)
			.for_checkbox(checkbox_id)
			.widget_holder();

		let point_editing_mode = CheckboxInput::new(self.options.path_editing_mode.point_editing_mode)
			// TODO(Keavon): Replace with a real icon
			.icon("Dot")
			.tooltip("Point Editing Mode\n\nShift + click to select both modes.")
			.on_update(|_| PathToolMessage::TogglePointEditing.into())
			.widget_holder();
		let segment_editing_mode = CheckboxInput::new(self.options.path_editing_mode.segment_editing_mode)
			// TODO(Keavon): Replace with a real icon
			.icon("Remove")
			.tooltip("Segment Editing Mode\n\nShift + click to select both modes.")
			.on_update(|_| PathToolMessage::ToggleSegmentEditing.into())
			.widget_holder();

		let path_overlay_mode_widget = RadioInput::new(vec![
			RadioEntryData::new("all")
				.icon("HandleVisibilityAll")
				.tooltip("Show all handles regardless of selection")
				.on_update(move |_| {
					PathToolMessage::UpdateOptions {
						options: PathOptionsUpdate::OverlayModeType(PathOverlayMode::AllHandles),
					}
					.into()
				}),
			RadioEntryData::new("selected")
				.icon("HandleVisibilitySelected")
				.tooltip("Show only handles of the segments connected to selected points")
				.on_update(move |_| {
					PathToolMessage::UpdateOptions {
						options: PathOptionsUpdate::OverlayModeType(PathOverlayMode::SelectedPointHandles),
					}
					.into()
				}),
			RadioEntryData::new("frontier")
				.icon("HandleVisibilityFrontier")
				.tooltip("Show only handles at the frontiers of the segments connected to selected points")
				.on_update(move |_| {
					PathToolMessage::UpdateOptions {
						options: PathOptionsUpdate::OverlayModeType(PathOverlayMode::FrontierHandles),
					}
					.into()
				}),
		])
		.selected_index(Some(self.options.path_overlay_mode as u32))
		.widget_holder();

		// Works only if a single layer is selected and its type is Vector
		let path_node_button = TextButton::new("Make Path Editable")
			.icon(Some("NodeShape".into()))
			.tooltip("Make Path Editable")
			.on_update(|_| NodeGraphMessage::AddPathNode.into())
			.disabled(!self.tool_data.make_path_editable_is_allowed)
			.widget_holder();

		let [_checkbox, _dropdown] = {
			let pivot_gizmo_type_widget = pivot_gizmo_type_widget(self.tool_data.pivot_gizmo.state, PivotToolSource::Path);
			[pivot_gizmo_type_widget[0].clone(), pivot_gizmo_type_widget[2].clone()]
		};

		let has_something = !self.tool_data.saved_points_before_anchor_convert_smooth_sharp.is_empty();
		let _pivot_reference = pivot_reference_point_widget(
			has_something || !self.tool_data.pivot_gizmo.state.is_pivot(),
			self.tool_data.pivot_gizmo.pivot.to_pivot_position(),
			PivotToolSource::Path,
		);

		let _pin_pivot = pin_pivot_widget(self.tool_data.pivot_gizmo.pin_active(), false, PivotToolSource::Path);

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				x_location,
				related_seperator.clone(),
				y_location,
				unrelated_seperator.clone(),
				colinear_handle_checkbox,
				related_seperator.clone(),
				colinear_handles_label,
				unrelated_seperator.clone(),
				point_editing_mode,
				related_seperator.clone(),
				segment_editing_mode,
				unrelated_seperator.clone(),
				path_overlay_mode_widget,
				unrelated_seperator.clone(),
				path_node_button,
				// checkbox.clone(),
				// related_seperator.clone(),
				// dropdown.clone(),
				// unrelated_seperator,
				// pivot_reference,
				// related_seperator.clone(),
				// pin_pivot,
			],
		}]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for PathTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let updating_point = message == ToolMessage::Path(PathToolMessage::SelectedPointUpdated);

		match message {
			ToolMessage::Path(PathToolMessage::UpdateOptions { options }) => match options {
				PathOptionsUpdate::OverlayModeType(overlay_mode_type) => {
					self.options.path_overlay_mode = overlay_mode_type;
					responses.add(OverlaysMessage::Draw);
				}
				PathOptionsUpdate::PointEditingMode { enabled } => {
					self.options.path_editing_mode.point_editing_mode = enabled;
					responses.add(OverlaysMessage::Draw);
				}
				PathOptionsUpdate::SegmentEditingMode { enabled } => {
					self.options.path_editing_mode.segment_editing_mode = enabled;
					responses.add(OverlaysMessage::Draw);
				}
				PathOptionsUpdate::PivotGizmoType(gizmo_type) => {
					if !self.tool_data.pivot_gizmo.state.disabled {
						self.tool_data.pivot_gizmo.state.gizmo_type = gizmo_type;
						responses.add(ToolMessage::UpdateHints);
						let pivot_gizmo = self.tool_data.pivot_gizmo();
						responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });
						responses.add(NodeGraphMessage::RunDocumentGraph);
						self.send_layout(responses, LayoutTarget::ToolOptions);
					}
				}
				PathOptionsUpdate::TogglePivotGizmoType(state) => {
					self.tool_data.pivot_gizmo.state.disabled = !state;
					responses.add(ToolMessage::UpdateHints);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					self.send_layout(responses, LayoutTarget::ToolOptions);
				}

				PathOptionsUpdate::TogglePivotPinned => {
					self.tool_data.pivot_gizmo.pivot.pinned = !self.tool_data.pivot_gizmo.pivot.pinned;
					responses.add(ToolMessage::UpdateHints);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					self.send_layout(responses, LayoutTarget::ToolOptions);
				}
			},
			ToolMessage::Path(PathToolMessage::SwapSelectedHandles) => {
				if context.shape_editor.handle_with_pair_selected(&context.document.network_interface) {
					context.shape_editor.alternate_selected_handles(&context.document.network_interface);
					responses.add(PathToolMessage::SelectedPointUpdated);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::None });
					responses.add(OverlaysMessage::Draw);
				}
			}
			_ => {
				self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
			}
		}

		if updating_point {
			self.send_layout(responses, LayoutTarget::ToolOptions);
		}
	}

	// Different actions depending on state may be wanted:
	fn actions(&self) -> ActionList {
		match self.fsm_state {
			PathToolFsmState::Ready => actions!(PathToolMessageDiscriminant;
				DoubleClick,
				MouseDown,
				Delete,
				NudgeSelectedPoints,
				Enter,
				SelectAll,
				DeselectAllSelected,
				BreakPath,
				DeleteAndBreakPath,
				ClosePath,
				PointerMove,
				StartSlidingPoint,
				Copy,
				Cut,
				DeleteSelected,
				Paste,
				Duplicate,
				TogglePointEditing,
				ToggleSegmentEditing
			),
			PathToolFsmState::Dragging(_) => actions!(PathToolMessageDiscriminant;
				Escape,
				RightClick,
				DoubleClick,
				DragStop,
				PointerMove,
				Delete,
				BreakPath,
				DeleteAndBreakPath,
				SwapSelectedHandles,
				StartSlidingPoint,
				Copy,
				Cut,
				DeleteSelected,
				Paste,
				Duplicate,
				TogglePointEditing,
				ToggleSegmentEditing
			),
			PathToolFsmState::Drawing { .. } => actions!(PathToolMessageDiscriminant;
				DoubleClick,
				DragStop,
				PointerMove,
				Delete,
				Enter,
				BreakPath,
				DeleteAndBreakPath,
				Escape,
				RightClick,
				StartSlidingPoint,
				TogglePointEditing,
				ToggleSegmentEditing
			),
			PathToolFsmState::SlidingPoint => actions!(PathToolMessageDiscriminant;
				PointerMove,
				DragStop,
				Escape,
				RightClick
			),
		}
	}
}

impl ToolTransition for PathTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(PathToolMessage::Abort.into()),
			selection_changed: Some(PathToolMessage::SelectionChanged.into()),
			overlay_provider: Some(|context| PathToolMessage::Overlays { context }.into()),
			..Default::default()
		}
	}
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DraggingState {
	point_select_state: PointSelectState,
	colinear: ManipulatorAngle,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum PointSelectState {
	HandleWithPair,
	#[default]
	HandleNoPair,
	Anchor,
}

#[derive(Clone, Copy)]
pub struct SlidingSegmentData {
	segment_id: SegmentId,
	bezier: PathSeg,
	start: PointId,
}

#[derive(Clone, Copy)]
pub struct SlidingPointInfo {
	anchor: PointId,
	layer: LayerNodeIdentifier,
	connected_segments: [SlidingSegmentData; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PathToolFsmState {
	#[default]
	Ready,
	Dragging(DraggingState),
	Drawing {
		selection_shape: SelectionShapeType,
	},
	SlidingPoint,
}

#[derive(Default)]
struct PathToolData {
	snap_manager: SnapManager,
	lasso_polygon: Vec<DVec2>,
	selection_mode: Option<SelectionMode>,
	drag_start_pos: DVec2,
	previous_mouse_position: DVec2,
	toggle_colinear_debounce: bool,
	opposing_handle_lengths: Option<OpposingHandleLengths>,
	/// Describes information about the selected point(s), if any, across one or multiple shapes and manipulator point types (anchor or handle).
	/// The available information varies depending on whether `None`, `One`, or `Multiple` points are currently selected.
	/// NOTE: It must be updated using `update_selection_status` to ensure `can_toggle_colinearity` stays synchronized with the current selection.
	selection_status: SelectionStatus,
	/// `true` if we can change the current selection to colinear or not.
	can_toggle_colinearity: bool,
	segment: Option<ClosestSegment>,
	snap_cache: SnapCache,
	double_click_handled: bool,
	delete_segment_pressed: bool,
	segment_editing_modifier: bool,
	multiple_toggle_pressed: bool,
	auto_panning: AutoPanning,
	saved_points_before_anchor_select_toggle: HashMap<LayerNodeIdentifier, Vec<ManipulatorPointId>>,
	select_anchor_toggled: bool,
	saved_selection_before_handle_drag: HashMap<LayerNodeIdentifier, (HashSet<ManipulatorPointId>, HashSet<SegmentId>)>,
	handle_drag_toggle: bool,
	saved_points_before_anchor_convert_smooth_sharp: HashMap<LayerNodeIdentifier, Vec<ManipulatorPointId>>,
	last_click_time: u64,
	dragging_state: DraggingState,
	angle: f64,
	pivot_gizmo: PivotGizmo,
	ordered_points: Vec<ManipulatorPointId>,
	opposite_handle_position: Option<DVec2>,
	last_clicked_point_was_selected: bool,
	last_clicked_segment_was_selected: bool,
	snapping_axis: Option<Axis>,
	alt_clicked_on_anchor: bool,
	alt_dragging_from_anchor: bool,
	angle_locked: bool,
	temporary_colinear_handles: bool,
	molding_info: Option<(DVec2, DVec2)>,
	molding_segment: bool,
	temporary_adjacent_handles_while_molding: Option<[Option<HandleId>; 2]>,
	frontier_handles_info: Option<HashMap<LayerNodeIdentifier, HashMap<SegmentId, Vec<PointId>>>>,
	adjacent_anchor_offset: Option<DVec2>,
	sliding_point_info: Option<SlidingPointInfo>,
	started_drawing_from_inside: bool,
	first_selected_with_single_click: bool,
	stored_selection: Option<HashMap<LayerNodeIdentifier, SelectedLayerState>>,
	last_drill_through_click_position: Option<DVec2>,
	drill_through_cycle_index: usize,
	drill_through_cycle_count: usize,
	hovered_layers: Vec<LayerNodeIdentifier>,
	ghost_outline: Vec<(Vec<ClickTargetType>, LayerNodeIdentifier)>,
	make_path_editable_is_allowed: bool,
}

impl PathToolData {
	fn save_points_before_anchor_toggle(&mut self, points: HashMap<LayerNodeIdentifier, Vec<ManipulatorPointId>>) -> PathToolFsmState {
		self.saved_points_before_anchor_select_toggle = points;
		PathToolFsmState::Dragging(self.dragging_state)
	}

	pub fn selection_quad(&self, metadata: &DocumentMetadata) -> Quad {
		let bbox = self.selection_box(metadata);
		Quad::from_box(bbox)
	}

	pub fn calculate_selection_mode_from_direction(&mut self, metadata: &DocumentMetadata) -> SelectionMode {
		let bbox = self.selection_box(metadata);
		let above_threshold = bbox[1].distance_squared(bbox[0]) > DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD.powi(2);

		if self.selection_mode.is_none() && above_threshold {
			let mode = if bbox[1].x < bbox[0].x {
				SelectionMode::Touched
			} else {
				// This also covers the case where they're equal: the area is zero, so we use `Enclosed` to ensure the selection ends up empty, as nothing will be enclosed by an empty area
				SelectionMode::Enclosed
			};
			self.selection_mode = Some(mode);
		}

		self.selection_mode.unwrap_or(SelectionMode::Touched)
	}

	pub fn selection_box(&self, metadata: &DocumentMetadata) -> [DVec2; 2] {
		// Convert previous mouse position to viewport space first
		let document_to_viewport = metadata.document_to_viewport;
		let previous_mouse = document_to_viewport.transform_point2(self.previous_mouse_position);
		if previous_mouse == self.drag_start_pos {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[self.drag_start_pos - tolerance, self.drag_start_pos + tolerance]
		} else {
			[self.drag_start_pos, previous_mouse]
		}
	}

	fn update_selection_status(&mut self, shape_editor: &mut ShapeState, document: &DocumentMessageHandler) {
		let selection_status = get_selection_status(&document.network_interface, shape_editor);

		self.can_toggle_colinearity = match &selection_status {
			SelectionStatus::None => false,
			SelectionStatus::One(single_selected_point) => {
				let vector = document.network_interface.compute_modified_vector(single_selected_point.layer).unwrap();
				if single_selected_point.id.get_handle_pair(&vector).is_some() {
					let anchor = single_selected_point.id.get_anchor(&vector).expect("Cannot find connected anchor");
					vector.all_connected(anchor).count() <= 2
				} else {
					false
				}
			}
			SelectionStatus::Multiple(_) => true,
		};
		self.selection_status = selection_status;
	}

	fn remove_saved_points(&mut self) {
		self.saved_points_before_anchor_select_toggle.clear();
	}

	fn reset_drill_through_cycle(&mut self) {
		self.last_drill_through_click_position = None;
		self.drill_through_cycle_index = 0;
	}

	fn next_drill_through_cycle(&mut self, position: DVec2) -> usize {
		if self.last_drill_through_click_position.is_none_or(|last_pos| last_pos.distance(position) > DRILL_THROUGH_THRESHOLD) {
			// New position, reset cycle
			self.drill_through_cycle_index = 0;
		} else {
			// Same position, advance cycle
			self.drill_through_cycle_index = (self.drill_through_cycle_index + 1) % self.drill_through_cycle_count.max(1);
		}
		self.last_drill_through_click_position = Some(position);
		self.drill_through_cycle_index
	}

	fn peek_drill_through_index(&self) -> usize {
		if self.drill_through_cycle_count == 0 {
			0
		} else {
			(self.drill_through_cycle_index + 1) % self.drill_through_cycle_count.max(1)
		}
	}

	fn has_drill_through_mouse_moved(&self, position: DVec2) -> bool {
		self.last_drill_through_click_position.is_none_or(|last_pos| last_pos.distance(position) > DRILL_THROUGH_THRESHOLD)
	}

	fn set_ghost_outline(&mut self, shape_editor: &ShapeState, document: &DocumentMessageHandler) {
		self.ghost_outline.clear();
		for &layer in shape_editor.selected_shape_state.keys() {
			// We probably need to collect here
			let outline: Vec<ClickTargetType> = document.metadata().layer_with_free_points_outline(layer).cloned().collect();

			self.ghost_outline.push((outline, layer));
		}
	}

	// TODO: This function is for basic point select mode. We definitely need to make a new one for the segment select mode.
	#[allow(clippy::too_many_arguments)]
	fn mouse_down(
		&mut self,
		shape_editor: &mut ShapeState,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
		extend_selection: bool,
		lasso_select: bool,
		handle_drag_from_anchor: bool,
		drag_zero_handle: bool,
		segment_editing_modifier: bool,
		path_overlay_mode: PathOverlayMode,
		segment_editing_mode: bool,
		point_editing_mode: bool,
	) -> PathToolFsmState {
		self.double_click_handled = false;
		self.opposing_handle_lengths = None;

		self.drag_start_pos = input.mouse.position;

		if input.time - self.last_click_time > DOUBLE_CLICK_MILLISECONDS {
			self.saved_points_before_anchor_convert_smooth_sharp.clear();
			self.stored_selection = None;
		}

		self.last_click_time = input.time;

		let mut old_selection = HashMap::new();

		for (layer, state) in &shape_editor.selected_shape_state {
			let selected_points = state.selected_points().collect::<HashSet<_>>();
			let selected_segments = state.selected_segments().collect::<HashSet<_>>();
			old_selection.insert(*layer, (selected_points, selected_segments));
		}

		// Check if the point is already selected; if not, select the first point within the threshold (in pixels)
		// Don't select the points which are not shown currently in PathOverlayMode
		if let Some((already_selected, mut selection_info)) = shape_editor.get_point_selection_state(
			&document.network_interface,
			input.mouse.position,
			SELECTION_THRESHOLD,
			path_overlay_mode,
			self.frontier_handles_info.as_ref(),
			point_editing_mode,
		) {
			responses.add(DocumentMessage::StartTransaction);

			self.set_ghost_outline(shape_editor, document);

			self.last_clicked_point_was_selected = already_selected;

			// If the point is already selected and shift (`extend_selection`) is used, keep the selection unchanged.
			// Otherwise, select the first point within the threshold.
			if !(already_selected && extend_selection) {
				if let Some(updated_selection_info) = shape_editor.change_point_selection(
					&document.network_interface,
					input.mouse.position,
					SELECTION_THRESHOLD,
					extend_selection,
					path_overlay_mode,
					self.frontier_handles_info.as_ref(),
				) {
					selection_info = updated_selection_info;
				}
			}

			if let Some(selected_points) = selection_info {
				self.drag_start_pos = input.mouse.position;

				// If selected points contain only handles and there was some selection before, then it is stored and becomes restored upon release
				let mut dragging_only_handles = true;
				for point in &selected_points.points {
					if matches!(point.point_id, ManipulatorPointId::Anchor(_)) {
						dragging_only_handles = false;
						break;
					}
				}
				if dragging_only_handles && !self.handle_drag_toggle && !old_selection.is_empty() {
					self.saved_selection_before_handle_drag = old_selection;
				}

				if handle_drag_from_anchor {
					if let Some((layer, point)) = shape_editor.find_nearest_point_indices(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD) {
						// Check that selected point is an anchor
						if let (Some(point_id), Some(vector)) = (point.as_anchor(), document.network_interface.compute_modified_vector(layer)) {
							let handles = vector.all_connected(point_id).collect::<Vec<_>>();
							self.alt_clicked_on_anchor = true;
							for handle in &handles {
								let modification_type = handle.set_relative_position(DVec2::ZERO);
								responses.add(GraphOperationMessage::Vector { layer, modification_type });
								for &handles in &vector.colinear_manipulators {
									if handles.contains(handle) {
										let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
										responses.add(GraphOperationMessage::Vector { layer, modification_type });
									}
								}
							}

							let manipulator_point_id = handles[0].to_manipulator_point();
							shape_editor.deselect_all_points();
							shape_editor.select_point_by_layer_and_id(manipulator_point_id, layer);
							responses.add(PathToolMessage::SelectedPointUpdated);
						}
					}
				}

				if let Some((Some(point), Some(vector), layer)) = shape_editor
					.find_nearest_point_indices(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD)
					.map(|(layer, point)| (point.as_anchor(), document.network_interface.compute_modified_vector(layer), layer))
				{
					let handles = vector
						.all_connected(point)
						.filter(|handle| handle.length(&vector) < 1e-6)
						.map(|handle| handle.to_manipulator_point())
						.collect::<Vec<_>>();
					let endpoint = vector.extendable_points(false).any(|anchor| point == anchor);

					if drag_zero_handle && (handles.len() == 1 && !endpoint) {
						shape_editor.deselect_all_points();
						shape_editor.select_points_by_layer_and_id(&HashMap::from([(layer, handles)]));
						shape_editor.convert_selected_manipulators_to_colinear_handles(responses, document);
					}
				}

				self.start_dragging_point(selected_points, input, document, shape_editor);
				responses.add(OverlaysMessage::Draw);
			}
			PathToolFsmState::Dragging(self.dragging_state)
		}
		// We didn't find a point nearby, so we will see if there is a segment to select or insert a point on
		else if let Some(segment) = shape_editor.upper_closest_segment(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD) {
			responses.add(DocumentMessage::StartTransaction);

			self.set_ghost_outline(shape_editor, document);

			if segment_editing_mode && !segment_editing_modifier {
				let layer = segment.layer();
				let segment_id = segment.segment();
				let already_selected = shape_editor.selected_shape_state.get(&layer).is_some_and(|state| state.is_segment_selected(segment_id));
				self.last_clicked_segment_was_selected = already_selected;

				if !(already_selected && extend_selection) {
					let retain_existing_selection = extend_selection || already_selected;
					if !retain_existing_selection {
						shape_editor.deselect_all_segments();
						shape_editor.deselect_all_points();
					}

					// Add to selected segments
					if let Some(selected_shape_state) = shape_editor.selected_shape_state.get_mut(&layer) {
						selected_shape_state.select_segment(segment_id);
					}

					// TODO: If the segment connected to one of the endpoints is also selected then select that point
				}

				self.drag_start_pos = input.mouse.position;
				let viewport_to_document = document.metadata().document_to_viewport.inverse();
				self.previous_mouse_position = viewport_to_document.transform_point2(input.mouse.position);

				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Dragging(self.dragging_state)
			} else {
				let points = pathseg_points(segment.pathseg());

				let [pos1, pos2] = match (points.p1, points.p2) {
					(Some(p1), Some(p2)) => [p1, p2],
					(Some(p1), None) | (None, Some(p1)) => [p1, points.p3],
					(None, None) => [points.p0 + (points.p3 - points.p0) / 3., points.p3 + (points.p0 - points.p3) / 3.],
				};
				self.molding_info = Some((pos1, pos2));
				PathToolFsmState::Dragging(self.dragging_state)
			}
		}
		// If no other layers are selected and this is a single-click, then also select the layer (exception)
		else if let Some(layer) = document.click(input) {
			if shape_editor.selected_shape_state.is_empty() {
				self.first_selected_with_single_click = true;
				// This ensures we don't need to double click a second time to get the drill through to work
				self.last_drill_through_click_position = Some(input.mouse.position);
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
			}

			self.started_drawing_from_inside = true;

			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);

			let selection_shape = if lasso_select { SelectionShapeType::Lasso } else { SelectionShapeType::Box };
			PathToolFsmState::Drawing { selection_shape }
		}
		// Start drawing
		else {
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);

			let selection_shape = if lasso_select { SelectionShapeType::Lasso } else { SelectionShapeType::Box };
			PathToolFsmState::Drawing { selection_shape }
		}
	}

	fn start_dragging_point(&mut self, selected_points: SelectedPointsInfo, input: &InputPreprocessorMessageHandler, document: &DocumentMessageHandler, shape_editor: &mut ShapeState) {
		let mut manipulators = HashMap::with_hasher(NoHashBuilder);
		let mut unselected = Vec::new();
		for (&layer, state) in &shape_editor.selected_shape_state {
			let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
			let transform = document.metadata().transform_to_document_if_feeds(layer, &document.network_interface);

			let mut layer_manipulators = HashSet::with_hasher(NoHashBuilder);
			for point in state.selected_points() {
				let Some(anchor) = point.get_anchor(&vector) else { continue };
				layer_manipulators.insert(anchor);
				let Some([handle1, handle2]) = point.get_handle_pair(&vector) else { continue };
				let Some(handle) = point.as_handle() else { continue };
				// Check which handle is selected and which is opposite
				let opposite = if handle == handle1 { handle2 } else { handle1 };

				self.opposite_handle_position = if self.opposite_handle_position.is_none() {
					opposite.to_manipulator_point().get_position(&vector)
				} else {
					self.opposite_handle_position
				};
			}
			for (&id, &position) in vector.point_domain.ids().iter().zip(vector.point_domain.positions()) {
				if layer_manipulators.contains(&id) {
					continue;
				}
				unselected.push(SnapCandidatePoint::handle(transform.transform_point2(position)))
			}
			if !layer_manipulators.is_empty() {
				manipulators.insert(layer, layer_manipulators);
			}
		}
		self.snap_cache = SnapCache { manipulators, unselected };

		let viewport_to_document = document.metadata().document_to_viewport.inverse();
		self.previous_mouse_position = viewport_to_document.transform_point2(input.mouse.position - selected_points.offset);
	}

	fn update_colinear(&mut self, equidistant: bool, toggle_colinear: bool, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> bool {
		// Check handle colinear state
		let is_colinear = self
			.selection_status
			.angle()
			.map(|angle| match angle {
				ManipulatorAngle::Colinear => true,
				ManipulatorAngle::Free | ManipulatorAngle::Mixed => false,
			})
			.unwrap_or(false);

		// Check if the toggle_colinear key has just been pressed
		if toggle_colinear && !self.toggle_colinear_debounce {
			self.opposing_handle_lengths = None;
			if is_colinear {
				shape_editor.disable_colinear_handles_state_on_selected(&document.network_interface, responses);
			} else {
				shape_editor.convert_selected_manipulators_to_colinear_handles(responses, document);
			}
			self.toggle_colinear_debounce = true;
			return true;
		}
		self.toggle_colinear_debounce = toggle_colinear;

		if equidistant && self.opposing_handle_lengths.is_none() {
			if !is_colinear {
				// Try to get selected handle info
				let Some((_, _, selected_handle_id)) = self.try_get_selected_handle_and_anchor(shape_editor, document) else {
					self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(document));
					return false;
				};

				let Some((layer, _)) = shape_editor.selected_shape_state.iter().next() else {
					self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(document));
					return false;
				};

				let Some(vector) = document.network_interface.compute_modified_vector(*layer) else {
					self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(document));
					return false;
				};

				// Check if handle has a pair (to ignore handles of edges of open paths)
				if let Some(handle_pair) = selected_handle_id.get_handle_pair(&vector) {
					let opposite_handle_length = handle_pair.iter().filter(|&&h| h.to_manipulator_point() != selected_handle_id).find_map(|&h| {
						let opp_handle_pos = h.to_manipulator_point().get_position(&vector)?;
						let opp_anchor_id = h.to_manipulator_point().get_anchor(&vector)?;
						let opp_anchor_pos = vector.point_domain.position_from_id(opp_anchor_id)?;
						Some((opp_handle_pos - opp_anchor_pos).length())
					});

					// Make handles colinear if opposite handle is zero length
					if opposite_handle_length == Some(0.) {
						shape_editor.convert_selected_manipulators_to_colinear_handles(responses, document);
						return true;
					}
				}
			}
			self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(document));
		}
		false
	}

	/// Attempts to get a single selected handle. Also retrieves the position of the anchor it is connected to. Used for the purpose of snapping the angle.
	fn try_get_selected_handle_and_anchor(&self, shape_editor: &ShapeState, document: &DocumentMessageHandler) -> Option<(DVec2, DVec2, ManipulatorPointId)> {
		// Only count selections of a single layer
		let (layer, selection) = shape_editor.selected_shape_state.iter().next()?;

		// Do not allow selections of multiple points to count
		if selection.selected_points_count() != 1 {
			return None;
		}

		// Only count selected handles
		let selected_handle = selection.selected_points().next()?.as_handle()?;
		let handle_id = selected_handle.to_manipulator_point();

		let layer_to_document = document.metadata().transform_to_document_if_feeds(*layer, &document.network_interface);
		let vector = document.network_interface.compute_modified_vector(*layer)?;

		let handle_position_local = selected_handle.to_manipulator_point().get_position(&vector)?;
		let anchor_id = selected_handle.to_manipulator_point().get_anchor(&vector)?;
		let anchor_position_local = vector.point_domain.position_from_id(anchor_id)?;

		let handle_position_document = layer_to_document.transform_point2(handle_position_local);
		let anchor_position_document = layer_to_document.transform_point2(anchor_position_local);

		Some((handle_position_document, anchor_position_document, handle_id))
	}

	#[allow(clippy::too_many_arguments)]
	fn calculate_handle_angle(
		&mut self,
		shape_editor: &mut ShapeState,
		document: &DocumentMessageHandler,
		responses: &mut VecDeque<Message>,
		relative_vector: DVec2,
		handle_vector: DVec2,
		handle_id: ManipulatorPointId,
		lock_angle: bool,
		snap_angle: bool,
		tangent_to_neighboring_tangents: bool,
	) -> f64 {
		let current_angle = -handle_vector.angle_to(DVec2::X);

		if let Some((vector, layer)) = shape_editor
			.selected_shape_state
			.iter()
			.next()
			.and_then(|(layer, _)| document.network_interface.compute_modified_vector(*layer).map(|vector| (vector, layer)))
		{
			let adjacent_anchor = check_handle_over_adjacent_anchor(handle_id, &vector);
			let mut required_angle = None;

			// If the handle is dragged over one of its adjacent anchors while holding down the Ctrl key, compute the angle based on the tangent formed with the neighboring anchor points.
			if adjacent_anchor.is_some() && lock_angle && !self.angle_locked {
				let anchor = handle_id.get_anchor(&vector);
				let (angle, anchor_position) = calculate_adjacent_anchor_tangent(handle_id, anchor, adjacent_anchor, &vector);

				let layer_to_document = document.metadata().transform_to_document_if_feeds(*layer, &document.network_interface);

				self.adjacent_anchor_offset = handle_id
					.get_anchor_position(&vector)
					.and_then(|handle_anchor| anchor_position.map(|adjacent_anchor| layer_to_document.transform_point2(adjacent_anchor) - layer_to_document.transform_point2(handle_anchor)));

				required_angle = angle;
			}

			// If the handle is dragged near its adjacent anchors while holding down the Ctrl key, compute the angle using the tangent direction of neighboring segments.
			if relative_vector.length() < 25. && lock_angle && !self.angle_locked {
				required_angle = calculate_lock_angle(self, shape_editor, responses, document, &vector, handle_id, tangent_to_neighboring_tangents);
			}

			// Finalize and apply angle locking if a valid target angle was determined.
			if let Some(angle) = required_angle {
				self.angle = angle;
				self.angle_locked = true;
				return angle;
			}
		}

		if lock_angle && !self.angle_locked {
			self.angle_locked = true;
			self.angle = -relative_vector.angle_to(DVec2::X);
			return -relative_vector.angle_to(DVec2::X);
		}

		// When the angle is locked we use the old angle
		if self.angle_locked {
			return self.angle;
		}

		// Round the angle to the closest increment
		let mut handle_angle = current_angle;
		if snap_angle && !lock_angle {
			let snap_resolution = HANDLE_ROTATE_SNAP_ANGLE.to_radians();
			handle_angle = (handle_angle / snap_resolution).round() * snap_resolution;
		}

		self.angle = handle_angle;

		handle_angle
	}

	#[allow(clippy::too_many_arguments)]
	fn apply_snapping(
		&mut self,
		handle_direction: DVec2,
		new_handle_position: DVec2,
		anchor_position: DVec2,
		using_angle_constraints: bool,
		handle_position: DVec2,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
	) -> DVec2 {
		let snap_data = SnapData::new(document, input);
		let snap_point = SnapCandidatePoint::handle_neighbors(new_handle_position, [anchor_position]);

		let snap_result = match using_angle_constraints {
			true => {
				let snap_constraint = SnapConstraint::Line {
					origin: anchor_position,
					direction: handle_direction.normalize_or_zero(),
				};

				self.snap_manager.constrained_snap(&snap_data, &snap_point, snap_constraint, Default::default())
			}
			false => self.snap_manager.free_snap(&snap_data, &snap_point, Default::default()),
		};

		self.snap_manager.update_indicator(snap_result.clone());

		document.metadata().document_to_viewport.transform_vector2(snap_result.snapped_point_document - handle_position)
	}

	fn start_snap_along_axis(&mut self, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// Find the negative delta to take the point to the drag start position
		let current_mouse = input.mouse.position;
		let drag_start = self.drag_start_pos;
		let opposite_delta = drag_start - current_mouse;

		shape_editor.move_selected_points_and_segments(None, document, opposite_delta, false, true, false, None, false, responses);

		// Calculate the projected delta and shift the points along that delta
		let delta = current_mouse - drag_start;
		let axis = if delta.x.abs() >= delta.y.abs() { Axis::X } else { Axis::Y };
		self.snapping_axis = Some(axis);
		let projected_delta = match axis {
			Axis::X => DVec2::new(delta.x, 0.),
			Axis::Y => DVec2::new(0., delta.y),
			_ => DVec2::new(delta.x, 0.),
		};

		shape_editor.move_selected_points_and_segments(None, document, projected_delta, false, true, false, None, false, responses);
	}

	fn stop_snap_along_axis(&mut self, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// Calculate the negative delta of the selection and move it back to the drag start
		let current_mouse = input.mouse.position;
		let drag_start = self.drag_start_pos;

		let opposite_delta = drag_start - current_mouse;
		let Some(axis) = self.snapping_axis else { return };
		let opposite_projected_delta = match axis {
			Axis::X => DVec2::new(opposite_delta.x, 0.),
			Axis::Y => DVec2::new(0., opposite_delta.y),
			_ => DVec2::new(opposite_delta.x, 0.),
		};

		shape_editor.move_selected_points_and_segments(None, document, opposite_projected_delta, false, true, false, None, false, responses);

		// Calculate what actually would have been the original delta for the point, and apply that
		let delta = current_mouse - drag_start;

		shape_editor.move_selected_points_and_segments(None, document, delta, false, true, false, None, false, responses);

		self.snapping_axis = None;
	}

	fn get_normalized_tangent(&mut self, point: PointId, segment: SegmentId, vector: &Vector) -> Option<DVec2> {
		let other_point = vector.other_point(segment, point)?;
		let position = ManipulatorPointId::Anchor(point).get_position(vector)?;

		let mut handles = vector.all_connected(other_point);
		let other_handle = handles.find(|handle| handle.segment == segment)?;

		let target_position = if other_handle.length(vector) == 0. {
			ManipulatorPointId::Anchor(other_point).get_position(vector)?
		} else {
			other_handle.to_manipulator_point().get_position(vector)?
		};

		let tangent_vector = target_position - position;
		tangent_vector.try_normalize()
	}

	fn update_closest_segment(&mut self, shape_editor: &mut ShapeState, position: DVec2, document: &DocumentMessageHandler, path_overlay_mode: PathOverlayMode, point_editing_mode: bool) {
		// Check if there is no point nearby
		// If the point mode is deactivated then don't override closest segment even if there is a closer point
		if shape_editor
			.find_nearest_visible_point_indices(&document.network_interface, position, SELECTION_THRESHOLD, path_overlay_mode, self.frontier_handles_info.as_ref())
			.is_some()
			&& point_editing_mode
		{
			self.segment = None;
		}
		// If already hovering on a segment, then recalculate its closest point
		else if let Some(closest_segment) = &mut self.segment {
			closest_segment.update_closest_point(document.metadata(), &document.network_interface, position);

			let layer = closest_segment.layer();
			let segment_id = closest_segment.segment();

			if closest_segment.too_far(position, SEGMENT_INSERTION_DISTANCE) {
				self.segment = None;
			}

			// Check if that segment exists or it has been removed
			if let Some(vector_data) = document.network_interface.compute_modified_vector(layer)
				&& !(vector_data.segment_domain.ids().iter().any(|segment| *segment == segment_id))
			{
				self.segment = None;
			}
		}
		// If not, check that if there is some closest segment or not
		else if let Some(closest_segment) = shape_editor.upper_closest_segment(&document.network_interface, position, SEGMENT_INSERTION_DISTANCE) {
			self.segment = Some(closest_segment);
		}
	}

	fn start_sliding_point(&mut self, shape_editor: &mut ShapeState, document: &DocumentMessageHandler) -> bool {
		let single_anchor_selected = shape_editor.selected_points().count() == 1 && shape_editor.selected_points().any(|point| matches!(point, ManipulatorPointId::Anchor(_)));

		if single_anchor_selected {
			let Some(anchor) = shape_editor.selected_points().next() else { return false };
			let Some(layer) = document.network_interface.selected_nodes().selected_layers(document.metadata()).next() else {
				return false;
			};
			let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return false };

			let Some(point_id) = anchor.as_anchor() else { return false };

			let mut connected_segments = [None, None];
			for (segment, bezier, start, end) in vector.segment_iter() {
				if start == point_id || end == point_id {
					match (connected_segments[0], connected_segments[1]) {
						(None, None) => connected_segments[0] = Some(SlidingSegmentData { segment_id: segment, bezier, start }),
						(Some(_), None) => connected_segments[1] = Some(SlidingSegmentData { segment_id: segment, bezier, start }),
						_ => {
							warn!("more than two segments connected to the anchor point");
							return false;
						}
					}
				}
			}
			let connected_segments = if let [Some(seg1), Some(seg2)] = connected_segments {
				[seg1, seg2]
			} else {
				warn!("expected exactly two connected segments");
				return false;
			};

			self.sliding_point_info = Some(SlidingPointInfo {
				anchor: point_id,
				layer,
				connected_segments,
			});
			return true;
		}
		false
	}

	fn slide_point(&mut self, target_position: DVec2, responses: &mut VecDeque<Message>, network_interface: &NodeNetworkInterface, shape_editor: &ShapeState) {
		let Some(sliding_point_info) = self.sliding_point_info else { return };
		let anchor = sliding_point_info.anchor;
		let layer = sliding_point_info.layer;

		let Some(vector) = network_interface.compute_modified_vector(layer) else { return };
		let transform = network_interface.document_metadata().transform_to_viewport_if_feeds(layer, network_interface);
		let layer_pos = transform.inverse().transform_point2(target_position);

		let segments = sliding_point_info.connected_segments;

		let t1 = segments[0].bezier.nearest(dvec2_to_point(layer_pos), DEFAULT_ACCURACY).t;
		let position1 = point_to_dvec2(segments[0].bezier.eval(t1));

		let t2 = segments[1].bezier.nearest(dvec2_to_point(layer_pos), DEFAULT_ACCURACY).t;
		let position2 = point_to_dvec2(segments[1].bezier.eval(t2));

		let (closer_segment, further_segment, t_value, new_position) = if position2.distance(layer_pos) < position1.distance(layer_pos) {
			(segments[1], segments[0], t2, position2)
		} else {
			(segments[0], segments[1], t1, position1)
		};

		// Move the anchor to the new position
		let Some(current_position) = ManipulatorPointId::Anchor(anchor).get_position(&vector) else {
			return;
		};
		let delta = new_position - current_position;

		shape_editor.move_anchor(anchor, &vector, delta, layer, None, responses);

		// Make a split at the t_value
		let first = closer_segment.bezier.subsegment(0_f64..t_value);
		let second = closer_segment.bezier.subsegment(t_value..1.);

		let closer_segment_other_point = if anchor == closer_segment.start {
			closer_segment.bezier.end()
		} else {
			closer_segment.bezier.start()
		};

		let (split_segment, other_segment) = if first.start() == closer_segment_other_point { (first, second) } else { (second, first) };
		let split_segment_points = pathseg_points(split_segment);

		// Primary handle maps to primary handle and secondary maps to secondary
		let closer_primary_handle = HandleId::primary(closer_segment.segment_id);
		let Some(handle_position) = split_segment_points.p1 else { return };
		let relative_position1 = handle_position - split_segment_points.p0;
		let modification_type = closer_primary_handle.set_relative_position(relative_position1);
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		let closer_secondary_handle = HandleId::end(closer_segment.segment_id);
		let Some(handle_position) = split_segment_points.p2 else { return };
		let relative_position2 = handle_position - split_segment_points.p3;
		let modification_type = closer_secondary_handle.set_relative_position(relative_position2);
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		let end_handle_direction = if anchor == closer_segment.start { -relative_position1 } else { -relative_position2 };

		let further_segment_points = pathseg_points(further_segment.bezier);

		let (further_other_point, start_handle, end_handle, start_handle_pos) = if anchor == further_segment.start {
			(
				further_segment_points.p3,
				HandleId::end(further_segment.segment_id),
				HandleId::primary(further_segment.segment_id),
				further_segment_points.p2,
			)
		} else {
			(
				further_segment_points.p0,
				HandleId::primary(further_segment.segment_id),
				HandleId::end(further_segment.segment_id),
				further_segment_points.p1,
			)
		};
		let Some(start_handle_position) = start_handle_pos else { return };
		let start_handle_direction = start_handle_position - further_other_point;

		// Get normalized direction vectors, if cubic handle is zero then we consider corresponding tangent
		let d1 = start_handle_direction.try_normalize().unwrap_or({
			if anchor == further_segment.start {
				-pathseg_tangent(further_segment.bezier, 1.)
			} else {
				pathseg_tangent(further_segment.bezier, 0.)
			}
		});

		let d2 = end_handle_direction.try_normalize().unwrap_or_default();

		let min_len1 = start_handle_direction.length() * 0.4;
		let min_len2 = end_handle_direction.length() * 0.4;

		let (relative_pos1, relative_pos2) = find_two_param_best_approximate(further_other_point, new_position, d1, d2, min_len1, min_len2, further_segment.bezier, other_segment);

		// Now set those handles to these handle lengths keeping the directions d1, d2
		let modification_type = start_handle.set_relative_position(relative_pos1);
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		let modification_type = end_handle.set_relative_position(relative_pos2);
		responses.add(GraphOperationMessage::Vector { layer, modification_type });
	}

	#[allow(clippy::too_many_arguments)]
	fn drag(
		&mut self,
		equidistant: bool,
		lock_angle: bool,
		snap_angle: bool,
		snap_axis: bool,
		shape_editor: &mut ShapeState,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) {
		// First check if selection is not just a single handle point
		let selected_points = shape_editor.selected_points();
		let single_handle_selected = selected_points.count() == 1
			&& shape_editor
				.selected_points()
				.any(|point| matches!(point, ManipulatorPointId::EndHandle(_) | ManipulatorPointId::PrimaryHandle(_)));

		// This is where it starts snapping along axis
		if snap_axis && self.snapping_axis.is_none() && !single_handle_selected {
			self.start_snap_along_axis(shape_editor, document, input, responses);
		} else if !snap_axis && self.snapping_axis.is_some() {
			self.stop_snap_along_axis(shape_editor, document, input, responses);
		}

		let document_to_viewport = document.metadata().document_to_viewport;
		let previous_mouse = document_to_viewport.transform_point2(self.previous_mouse_position);
		let current_mouse = input.mouse.position;
		let raw_delta = document_to_viewport.inverse().transform_vector2(current_mouse - previous_mouse);

		let snapped_delta = if let Some((handle_position, anchor_position, handle_id)) = self.try_get_selected_handle_and_anchor(shape_editor, document) {
			let cursor_position = handle_position + raw_delta;

			let handle_angle = self.calculate_handle_angle(
				shape_editor,
				document,
				responses,
				handle_position - anchor_position,
				cursor_position - anchor_position,
				handle_id,
				lock_angle,
				snap_angle,
				equidistant,
			);

			let adjacent_anchor_offset = self.adjacent_anchor_offset.unwrap_or(DVec2::ZERO);
			let constrained_direction = DVec2::new(handle_angle.cos(), handle_angle.sin());
			let projected_length = (cursor_position - anchor_position - adjacent_anchor_offset).dot(constrained_direction);
			let constrained_target = anchor_position + adjacent_anchor_offset + constrained_direction * projected_length;
			let constrained_delta = constrained_target - handle_position;

			self.apply_snapping(
				constrained_direction,
				handle_position + constrained_delta,
				anchor_position + adjacent_anchor_offset,
				lock_angle || snap_angle,
				handle_position,
				document,
				input,
			)
		} else {
			shape_editor.snap(&mut self.snap_manager, &self.snap_cache, document, input, previous_mouse)
		};

		let handle_lengths = if equidistant { None } else { self.opposing_handle_lengths.take() };
		let opposite = if lock_angle { None } else { self.opposite_handle_position };
		let unsnapped_delta = current_mouse - previous_mouse;
		let mut was_alt_dragging = false;

		if self.snapping_axis.is_none() {
			if self.alt_clicked_on_anchor && !self.alt_dragging_from_anchor && self.drag_start_pos.distance(input.mouse.position) > DRAG_THRESHOLD {
				// Checking which direction the dragging begins
				self.alt_dragging_from_anchor = true;
				let Some(layer) = document.network_interface.selected_nodes().selected_layers(document.metadata()).next() else {
					return;
				};
				let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return };
				let Some(point_id) = shape_editor.selected_points().next().unwrap().get_anchor(&vector) else {
					return;
				};

				if vector.connected_count(point_id) == 2 {
					let connected_segments: Vec<HandleId> = vector.all_connected(point_id).collect();
					let segment1 = connected_segments[0];
					let Some(tangent1) = self.get_normalized_tangent(point_id, segment1.segment, &vector) else {
						return;
					};
					let segment2 = connected_segments[1];
					let Some(tangent2) = self.get_normalized_tangent(point_id, segment2.segment, &vector) else {
						return;
					};

					let delta = input.mouse.position - self.drag_start_pos;
					let handle = if delta.dot(tangent1) >= delta.dot(tangent2) {
						segment1.to_manipulator_point()
					} else {
						segment2.to_manipulator_point()
					};

					// Now change the selection to this handle
					shape_editor.deselect_all_points();
					shape_editor.select_point_by_layer_and_id(handle, layer);

					responses.add(PathToolMessage::SelectionChanged);
				}
			}

			if self.alt_dragging_from_anchor && !equidistant && self.alt_clicked_on_anchor {
				was_alt_dragging = true;
				self.alt_dragging_from_anchor = false;
				self.alt_clicked_on_anchor = false;
			}

			let mut skip_opposite = false;
			if self.temporary_colinear_handles && !lock_angle {
				shape_editor.disable_colinear_handles_state_on_selected(&document.network_interface, responses);
				self.temporary_colinear_handles = false;
				skip_opposite = true;
			}
			shape_editor.move_selected_points_and_segments(handle_lengths, document, snapped_delta, equidistant, true, was_alt_dragging, opposite, skip_opposite, responses);
			self.previous_mouse_position += document_to_viewport.inverse().transform_vector2(snapped_delta);
		} else {
			let Some(axis) = self.snapping_axis else { return };
			let projected_delta = match axis {
				Axis::X => DVec2::new(unsnapped_delta.x, 0.),
				Axis::Y => DVec2::new(0., unsnapped_delta.y),
				_ => DVec2::new(unsnapped_delta.x, 0.),
			};
			shape_editor.move_selected_points_and_segments(handle_lengths, document, projected_delta, equidistant, true, false, opposite, false, responses);
			self.previous_mouse_position += document_to_viewport.inverse().transform_vector2(unsnapped_delta);
		}

		// Constantly checking and changing the snapping axis based on current mouse position
		if snap_axis && self.snapping_axis.is_some() {
			let Some(current_axis) = self.snapping_axis else { return };
			let total_delta = self.drag_start_pos - input.mouse.position;

			if (total_delta.x.abs() > total_delta.y.abs() && current_axis == Axis::Y) || (total_delta.y.abs() > total_delta.x.abs() && current_axis == Axis::X) {
				self.stop_snap_along_axis(shape_editor, document, input, responses);
				self.start_snap_along_axis(shape_editor, document, input, responses);
			}
		}
	}

	fn pivot_gizmo(&self) -> PivotGizmo {
		self.pivot_gizmo.clone()
	}

	fn sync_history(&mut self, points: &[ManipulatorPointId]) {
		self.ordered_points.retain(|layer| points.contains(layer));
		self.ordered_points.extend(points.iter().find(|&layer| !self.ordered_points.contains(layer)));
		self.pivot_gizmo.point = self.ordered_points.last().copied()
	}
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;
	type ToolOptions = PathToolOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext { document, input, shape_editor, .. } = tool_action_data;

		update_dynamic_hints(self, responses, shape_editor, document, tool_data, tool_options, input.mouse.position);

		let ToolMessage::Path(event) = event else { return self };

		// TODO(mTvare6): Remove once gizmos are implemented for path_tool
		tool_data.pivot_gizmo.state.disabled = true;

		match (self, event) {
			(_, PathToolMessage::SelectionChanged) => {
				// Set the newly targeted layers to visible
				let target_layers = document.network_interface.selected_nodes().selected_layers(document.metadata()).collect();

				shape_editor.set_selected_layers(target_layers);

				responses.add(OverlaysMessage::Draw);
				self
			}
			(_, PathToolMessage::UpdateSelectedPointsStatus { overlay_context }) => {
				let display_anchors = overlay_context.visibility_settings.anchors();
				let display_handles = overlay_context.visibility_settings.handles();

				shape_editor.update_selected_anchors_status(display_anchors);
				shape_editor.update_selected_handles_status(display_handles);

				let new_points = shape_editor.selected_points().copied().collect::<Vec<_>>();
				tool_data.sync_history(&new_points);

				self
			}
			(_, PathToolMessage::TogglePointEditing) => {
				// Clicked on the point edit mode button
				let point_edit = tool_options.path_editing_mode.point_editing_mode;
				let segment_edit = tool_options.path_editing_mode.segment_editing_mode;
				let multiple_toggle = tool_data.multiple_toggle_pressed;

				if point_edit && !segment_edit {
					return self;
				}

				match (multiple_toggle, point_edit) {
					(true, true) => {
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::PointEditingMode { enabled: false },
						});
					}
					(true, false) => {
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::PointEditingMode { enabled: true },
						});
					}
					(_, _) => {
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::PointEditingMode { enabled: true },
						});
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::SegmentEditingMode { enabled: false },
						});

						// Select all of the end points of selected segments
						let selected_layers = shape_editor.selected_layers().cloned().collect::<Vec<_>>();

						for layer in selected_layers {
							let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };

							let selected_state = shape_editor.selected_shape_state.entry(layer).or_default();

							for (segment, _, start, end) in vector.segment_bezier_iter() {
								if selected_state.is_segment_selected(segment) {
									selected_state.select_point(ManipulatorPointId::Anchor(start));
									selected_state.select_point(ManipulatorPointId::Anchor(end));
								}
							}
						}

						// Deselect all of the segments
						shape_editor.deselect_all_segments();
					}
				}

				self
			}
			(_, PathToolMessage::ToggleSegmentEditing) => {
				// Clicked on the point edit mode button
				let segment_edit = tool_options.path_editing_mode.segment_editing_mode;
				let point_edit = tool_options.path_editing_mode.point_editing_mode;

				let multiple_toggle = tool_data.multiple_toggle_pressed;

				if segment_edit && !point_edit {
					return self;
				}

				match (multiple_toggle, segment_edit) {
					(true, true) => {
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::SegmentEditingMode { enabled: false },
						});
					}
					(true, false) => {
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::SegmentEditingMode { enabled: true },
						});
					}
					(_, _) => {
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::PointEditingMode { enabled: false },
						});
						responses.add(PathToolMessage::UpdateOptions {
							options: PathOptionsUpdate::SegmentEditingMode { enabled: true },
						});

						// Select all the segments which have both of the ends selected
						let selected_layers = shape_editor.selected_layers().cloned().collect::<Vec<_>>();

						for layer in selected_layers {
							let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };

							let selected_state = shape_editor.selected_shape_state.entry(layer).or_default();

							for (segment, _, start, end) in vector.segment_bezier_iter() {
								let first_selected = selected_state.is_point_selected(ManipulatorPointId::Anchor(start));
								let second_selected = selected_state.is_point_selected(ManipulatorPointId::Anchor(end));
								if first_selected && second_selected {
									selected_state.select_segment(segment);
								}
							}
						}
					}
				}

				self
			}
			(_, PathToolMessage::Overlays { context: mut overlay_context }) => {
				// Set this to show ghost line only if drag actually happened
				if matches!(self, Self::Dragging(_)) && tool_data.drag_start_pos.distance(input.mouse.position) > DRAG_THRESHOLD {
					for (outline, layer) in &tool_data.ghost_outline {
						let transform = document.metadata().transform_to_viewport(*layer);
						overlay_context.outline(outline.iter(), transform, Some(COLOR_OVERLAY_GRAY));
					}
				}

				// TODO: find the segment ids of which the selected points are a part of

				match tool_options.path_overlay_mode {
					PathOverlayMode::AllHandles => {
						path_overlays(document, DrawHandles::All, shape_editor, &mut overlay_context);
						tool_data.frontier_handles_info = None;
					}
					PathOverlayMode::SelectedPointHandles => {
						let selected_segments = selected_segments(&document.network_interface, shape_editor);

						path_overlays(document, DrawHandles::SelectedAnchors(selected_segments), shape_editor, &mut overlay_context);
						tool_data.frontier_handles_info = None;
					}
					PathOverlayMode::FrontierHandles => {
						let selected_segments = selected_segments(&document.network_interface, shape_editor);

						// Match the behavior of `PathOverlayMode::SelectedPointHandles` when only one point is selected
						if shape_editor.selected_points().count() == 1 {
							path_overlays(document, DrawHandles::SelectedAnchors(selected_segments), shape_editor, &mut overlay_context);
						} else {
							let mut segment_endpoints_by_layer = HashMap::new();

							for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
								let mut segment_endpoints: HashMap<SegmentId, Vec<PointId>> = HashMap::new();

								let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
								let Some(state) = shape_editor.selected_shape_state.get_mut(&layer) else { continue };

								let selected_points = state.selected_points();
								let selected_anchors = selected_points
									.filter_map(|point_id| if let ManipulatorPointId::Anchor(p) = point_id { Some(p) } else { None })
									.collect::<Vec<_>>();

								let Some(focused_segments) = selected_segments.get(&layer) else { continue };

								// The points which are part of only one segment will be rendered
								let mut selected_segments_by_point: HashMap<PointId, Vec<SegmentId>> = HashMap::new();

								for (segment_id, _bezier, start, end) in vector.segment_bezier_iter() {
									if focused_segments.contains(&segment_id) {
										selected_segments_by_point.entry(start).or_default().push(segment_id);
										selected_segments_by_point.entry(end).or_default().push(segment_id);
									}
								}

								for (point, attached_segments) in selected_segments_by_point {
									if attached_segments.len() == 1 {
										segment_endpoints.entry(attached_segments[0]).or_default().push(point);
									}
									// Handle the edge case where a point, although not explicitly selected, is shared by two segments.
									else if !selected_anchors.contains(&point) {
										segment_endpoints.entry(attached_segments[0]).or_default().push(point);
										segment_endpoints.entry(attached_segments[1]).or_default().push(point);
									}
								}

								segment_endpoints_by_layer.insert(layer, segment_endpoints);
							}

							// Caching segment endpoints for use in point selection logic
							tool_data.frontier_handles_info = Some(segment_endpoints_by_layer.clone());

							// Now frontier anchors can be sent for rendering overlays
							path_overlays(document, DrawHandles::FrontierHandles(segment_endpoints_by_layer), shape_editor, &mut overlay_context);
						}
					}
				}

				match self {
					Self::Ready => {
						tool_data.update_closest_segment(
							shape_editor,
							input.mouse.position,
							document,
							tool_options.path_overlay_mode,
							tool_options.path_editing_mode.point_editing_mode,
						);

						// If there exists an underlying anchor, we show a hover overlay
						(|| {
							if !tool_options.path_editing_mode.point_editing_mode {
								return;
							}

							let nearest_visible_point_indices = shape_editor.find_nearest_visible_point_indices(
								&document.network_interface,
								input.mouse.position,
								SELECTION_THRESHOLD,
								tool_options.path_overlay_mode,
								tool_data.frontier_handles_info.as_ref(),
							);

							let Some((layer, manipulator_point_id)) = nearest_visible_point_indices else { return };
							let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return };
							let Some(position) = manipulator_point_id.get_position(&vector) else {
								error!("No position for hovered point");
								return;
							};

							let transform = document.metadata().transform_to_viewport(layer);
							let position = transform.transform_point2(position);
							let selected = shape_editor.selected_shape_state.entry(layer).or_default().is_point_selected(manipulator_point_id);

							match manipulator_point_id {
								ManipulatorPointId::Anchor(_) => overlay_context.hover_manipulator_anchor(position, selected),
								_ => overlay_context.hover_manipulator_handle(position, selected),
							}
						})();

						if let Some(closest_segment) = &tool_data.segment {
							if tool_options.path_editing_mode.segment_editing_mode && !tool_data.segment_editing_modifier {
								let transform = document.metadata().transform_to_viewport_if_feeds(closest_segment.layer(), &document.network_interface);

								overlay_context.outline_overlay_bezier(closest_segment.pathseg(), transform);

								// Draw the anchors again
								let display_anchors = overlay_context.visibility_settings.anchors();
								if display_anchors {
									let start_pos = transform.transform_point2(point_to_dvec2(closest_segment.pathseg().start()));
									let end_pos = transform.transform_point2(point_to_dvec2(closest_segment.pathseg().end()));
									let start_id = closest_segment.points()[0];
									let end_id = closest_segment.points()[1];
									if let Some(shape_state) = shape_editor.selected_shape_state.get_mut(&closest_segment.layer()) {
										overlay_context.manipulator_anchor(start_pos, shape_state.is_point_selected(ManipulatorPointId::Anchor(start_id)), None);
										overlay_context.manipulator_anchor(end_pos, shape_state.is_point_selected(ManipulatorPointId::Anchor(end_id)), None);
									}
								}
							} else {
								// We want this overlay also when in segment_editing_mode
								let perp = closest_segment.calculate_perp(document);
								let point = closest_segment.closest_point(document.metadata(), &document.network_interface);

								// Draw an X on the segment
								if tool_data.delete_segment_pressed {
									let angle = 45_f64.to_radians();
									let tilted_line = DVec2::from_angle(angle).rotate(perp);
									let tilted_perp = tilted_line.perp();

									overlay_context.line(point - tilted_line * SEGMENT_OVERLAY_SIZE, point + tilted_line * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), None);
									overlay_context.line(point - tilted_perp * SEGMENT_OVERLAY_SIZE, point + tilted_perp * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), None);
								}
								// Draw a line on the segment
								else {
									overlay_context.line(point - perp * SEGMENT_OVERLAY_SIZE, point + perp * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), None);
								}
							}
						}

						// Show outlines for hovered layers with appropriate highlighting
						let currently_selected_layer = document.network_interface.selected_nodes().selected_layers(document.metadata()).next();
						let next_selected_index = tool_data.peek_drill_through_index();
						let mouse_has_moved = tool_data.has_drill_through_mouse_moved(input.mouse.position);

						for (index, &hovered_layer) in tool_data.hovered_layers.iter().enumerate() {
							// Skip already highlighted selected layer
							if Some(hovered_layer) == currently_selected_layer {
								continue;
							}

							let layer_to_viewport = document.metadata().transform_to_viewport(hovered_layer);
							let outline = document.metadata().layer_with_free_points_outline(hovered_layer);

							// Determine highlight color based on drill-through state
							let color = match (index, mouse_has_moved) {
								// If the layer is the next selected one and mouse has not moved, highlight it blue
								(i, false) if i == next_selected_index => COLOR_OVERLAY_BLUE,
								// If the layer is the first hovered one and mouse has moved, highlight it blue
								(0, true) => COLOR_OVERLAY_BLUE,
								// Otherwise, use gray
								_ => COLOR_OVERLAY_GRAY,
							};

							// TODO: Make this draw underneath all other overlays
							overlay_context.outline(outline, layer_to_viewport, Some(color));
						}
					}
					Self::Drawing { selection_shape } => {
						let mut fill_color = graphene_std::Color::from_rgb_str(COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap())
							.unwrap()
							.with_alpha(0.05)
							.to_rgba_hex_srgb();
						fill_color.insert(0, '#');
						let fill_color = Some(fill_color.as_str());

						let selection_mode = match tool_action_data.preferences.get_selection_mode() {
							SelectionMode::Directional => tool_data.calculate_selection_mode_from_direction(document.metadata()),
							selection_mode => selection_mode,
						};

						let quad = tool_data.selection_quad(document.metadata());

						let select_segments = tool_options.path_editing_mode.segment_editing_mode;
						let select_points = tool_options.path_editing_mode.point_editing_mode;
						let (points_inside, segments_inside) = match selection_shape {
							SelectionShapeType::Box => {
								let previous_mouse = document.metadata().document_to_viewport.transform_point2(tool_data.previous_mouse_position);
								let bbox = Rect::new(tool_data.drag_start_pos.x, tool_data.drag_start_pos.y, previous_mouse.x, previous_mouse.y).abs();
								shape_editor.get_inside_points_and_segments(
									&document.network_interface,
									SelectionShape::Box(bbox),
									tool_options.path_overlay_mode,
									tool_data.frontier_handles_info.as_ref(),
									select_segments,
									select_points,
									selection_mode,
								)
							}
							SelectionShapeType::Lasso => shape_editor.get_inside_points_and_segments(
								&document.network_interface,
								SelectionShape::Lasso(&tool_data.lasso_polygon),
								tool_options.path_overlay_mode,
								tool_data.frontier_handles_info.as_ref(),
								select_segments,
								select_points,
								selection_mode,
							),
						};

						for (layer, points) in points_inside {
							let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };

							for point in points {
								let Some(position) = point.get_position(&vector) else { continue };

								let transform = document.metadata().transform_to_viewport(layer);
								let position = transform.transform_point2(position);

								let selected = shape_editor.selected_shape_state.entry(layer).or_default().is_point_selected(point);

								match point {
									ManipulatorPointId::Anchor(_) => overlay_context.hover_manipulator_anchor(position, selected),
									_ => overlay_context.hover_manipulator_handle(position, selected),
								}
							}
						}

						for (layer, segments) in segments_inside {
							let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };

							let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);

							for (segment, bezier, _, _) in vector.segment_iter() {
								if segments.contains(&segment) {
									overlay_context.outline_overlay_bezier(bezier, transform);
								}
							}
						}

						match (selection_shape, selection_mode, tool_data.started_drawing_from_inside) {
							// Don't draw box if it is from inside a shape and selection just began
							(SelectionShapeType::Box, SelectionMode::Enclosed, false) => overlay_context.dashed_quad(quad, None, fill_color, Some(4.), Some(4.), Some(0.5)),
							(SelectionShapeType::Lasso, SelectionMode::Enclosed, _) => overlay_context.dashed_polygon(&tool_data.lasso_polygon, None, fill_color, Some(4.), Some(4.), Some(0.5)),
							(SelectionShapeType::Box, _, false) => overlay_context.quad(quad, None, fill_color),
							(SelectionShapeType::Lasso, _, _) => overlay_context.polygon(&tool_data.lasso_polygon, None, fill_color),
							(SelectionShapeType::Box, _, _) => {}
						}
					}
					Self::Dragging(_) => {
						tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

						// Draw the snapping axis lines
						if tool_data.snapping_axis.is_some() {
							let Some(axis) = tool_data.snapping_axis else { return self };
							let origin = tool_data.drag_start_pos;
							let viewport_diagonal = input.viewport_bounds.size().length();

							let faded = |color: &str| {
								let mut color = graphene_std::Color::from_rgb_str(color.strip_prefix('#').unwrap()).unwrap().with_alpha(0.25).to_rgba_hex_srgb();
								color.insert(0, '#');
								color
							};
							match axis {
								Axis::Y => {
									overlay_context.line(origin - DVec2::Y * viewport_diagonal, origin + DVec2::Y * viewport_diagonal, Some(COLOR_OVERLAY_GREEN), None);
									overlay_context.line(origin - DVec2::X * viewport_diagonal, origin + DVec2::X * viewport_diagonal, Some(&faded(COLOR_OVERLAY_RED)), None);
								}
								Axis::X | Axis::Both => {
									overlay_context.line(origin - DVec2::X * viewport_diagonal, origin + DVec2::X * viewport_diagonal, Some(COLOR_OVERLAY_RED), None);
									overlay_context.line(origin - DVec2::Y * viewport_diagonal, origin + DVec2::Y * viewport_diagonal, Some(&faded(COLOR_OVERLAY_GREEN)), None);
								}
							}
						}
					}
					Self::SlidingPoint => {}
				}

				responses.add(PathToolMessage::SelectedPointUpdated);
				responses.add(PathToolMessage::UpdateSelectedPointsStatus { overlay_context });
				self
			}

			// Mouse down
			(
				_,
				PathToolMessage::MouseDown {
					extend_selection,
					lasso_select,
					handle_drag_from_anchor,
					drag_restore_handle,
					segment_editing_modifier,
				},
			) => {
				let extend_selection = input.keyboard.get(extend_selection as usize);
				let lasso_select = input.keyboard.get(lasso_select as usize);
				let handle_drag_from_anchor = input.keyboard.get(handle_drag_from_anchor as usize);
				let drag_zero_handle = input.keyboard.get(drag_restore_handle as usize);
				let segment_editing_modifier = input.keyboard.get(segment_editing_modifier as usize);

				tool_data.selection_mode = None;
				tool_data.lasso_polygon.clear();

				tool_data.mouse_down(
					shape_editor,
					document,
					input,
					responses,
					extend_selection,
					lasso_select,
					handle_drag_from_anchor,
					drag_zero_handle,
					segment_editing_modifier,
					tool_options.path_overlay_mode,
					tool_options.path_editing_mode.segment_editing_mode,
					tool_options.path_editing_mode.point_editing_mode,
				)
			}
			(
				PathToolFsmState::Drawing { selection_shape },
				PathToolMessage::PointerMove {
					equidistant,
					toggle_colinear,
					move_anchor_with_handles,
					snap_angle,
					lock_angle,
					delete_segment,
					break_colinear_molding,
					segment_editing_modifier,
				},
			) => {
				tool_data.previous_mouse_position = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);

				tool_data.started_drawing_from_inside = false;
				tool_data.stored_selection = None;

				if selection_shape == SelectionShapeType::Lasso {
					extend_lasso(&mut tool_data.lasso_polygon, input.mouse.position);
				}

				responses.add(OverlaysMessage::Draw);

				// Auto-panning
				let messages = [
					PathToolMessage::PointerOutsideViewport {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
						snap_angle,
						lock_angle,
						delete_segment,
						break_colinear_molding,
						segment_editing_modifier,
					}
					.into(),
					PathToolMessage::PointerMove {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
						snap_angle,
						lock_angle,
						delete_segment,
						break_colinear_molding,
						segment_editing_modifier,
					}
					.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				PathToolFsmState::Drawing { selection_shape }
			}
			(
				PathToolFsmState::Dragging(_),
				PathToolMessage::PointerMove {
					equidistant,
					toggle_colinear,
					move_anchor_with_handles,
					snap_angle,
					lock_angle,
					delete_segment,
					break_colinear_molding,
					segment_editing_modifier,
				},
			) => {
				let selected_only_handles = !shape_editor.selected_points().any(|point| matches!(point, ManipulatorPointId::Anchor(_)));
				tool_data.stored_selection = None;

				if !tool_data.saved_selection_before_handle_drag.is_empty() && (tool_data.drag_start_pos.distance(input.mouse.position) > DRAG_THRESHOLD) && (selected_only_handles) {
					tool_data.handle_drag_toggle = true;
				}

				if tool_data.drag_start_pos.distance(input.mouse.position) > DRAG_THRESHOLD {
					tool_data.molding_segment = true;
				}

				let break_molding = input.keyboard.get(break_colinear_molding as usize);

				// Logic for molding segment
				if let Some(segment) = &mut tool_data.segment {
					if let Some(molding_segment_handles) = tool_data.molding_info {
						tool_data.temporary_adjacent_handles_while_molding = segment.mold_handle_positions(
							document,
							responses,
							molding_segment_handles,
							input.mouse.position,
							break_molding,
							tool_data.temporary_adjacent_handles_while_molding,
						);

						return PathToolFsmState::Dragging(tool_data.dragging_state);
					}
				}

				let anchor_and_handle_toggled = input.keyboard.get(move_anchor_with_handles as usize);
				let initial_press = anchor_and_handle_toggled && !tool_data.select_anchor_toggled;
				let released_from_toggle = tool_data.select_anchor_toggled && !anchor_and_handle_toggled;

				if initial_press {
					responses.add(PathToolMessage::SelectedPointUpdated);
					tool_data.select_anchor_toggled = true;

					let mut points_to_save = HashMap::new();
					for (layer, state) in &shape_editor.selected_shape_state {
						points_to_save.insert(*layer, state.selected_points().collect::<Vec<_>>());
					}
					tool_data.save_points_before_anchor_toggle(points_to_save);

					shape_editor.select_anchor_and_connected_handles(&document.network_interface);
				} else if released_from_toggle {
					responses.add(PathToolMessage::SelectedPointUpdated);
					tool_data.select_anchor_toggled = false;
					shape_editor.deselect_all_points();
					shape_editor.select_points_by_layer_and_id(&tool_data.saved_points_before_anchor_select_toggle);
					tool_data.remove_saved_points();
				}

				let toggle_colinear_state = input.keyboard.get(toggle_colinear as usize);
				let equidistant_state = input.keyboard.get(equidistant as usize);
				let lock_angle_state = input.keyboard.get(lock_angle as usize);
				let snap_angle_state = input.keyboard.get(snap_angle as usize);

				if !lock_angle_state {
					tool_data.angle_locked = false;
					tool_data.adjacent_anchor_offset = None;
				}

				if !tool_data.update_colinear(equidistant_state, toggle_colinear_state, tool_action_data.shape_editor, tool_action_data.document, responses) {
					tool_data.drag(
						equidistant_state,
						lock_angle_state,
						snap_angle_state,
						snap_angle_state,
						tool_action_data.shape_editor,
						tool_action_data.document,
						input,
						responses,
					);
				}

				// Auto-panning
				let messages = [
					PathToolMessage::PointerOutsideViewport {
						toggle_colinear,
						equidistant,
						move_anchor_with_handles,
						snap_angle,
						lock_angle,
						delete_segment,
						break_colinear_molding,
						segment_editing_modifier,
					}
					.into(),
					PathToolMessage::PointerMove {
						toggle_colinear,
						equidistant,
						move_anchor_with_handles,
						snap_angle,
						lock_angle,
						delete_segment,
						break_colinear_molding,
						segment_editing_modifier,
					}
					.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				PathToolFsmState::Dragging(tool_data.dragging_state)
			}
			(PathToolFsmState::SlidingPoint, PathToolMessage::PointerMove { .. }) => {
				tool_data.slide_point(input.mouse.position, responses, &document.network_interface, shape_editor);
				PathToolFsmState::SlidingPoint
			}
			(
				PathToolFsmState::Ready,
				PathToolMessage::PointerMove {
					delete_segment,
					segment_editing_modifier,
					snap_angle,
					..
				},
			) => {
				tool_data.delete_segment_pressed = input.keyboard.get(delete_segment as usize);
				tool_data.segment_editing_modifier = input.keyboard.get(segment_editing_modifier as usize);
				tool_data.multiple_toggle_pressed = input.keyboard.get(snap_angle as usize);
				tool_data.saved_points_before_anchor_convert_smooth_sharp.clear();
				tool_data.adjacent_anchor_offset = None;
				tool_data.stored_selection = None;

				if tool_data.has_drill_through_mouse_moved(input.mouse.position) {
					tool_data.reset_drill_through_cycle();
				}

				// When moving the cursor around we want to update the hovered layers
				let new_hovered_layers: Vec<LayerNodeIdentifier> = document
					.click_list_no_parents(input)
					.filter(|&layer| {
						// Filter out artboards and parent holders, and already selected layers
						!document.network_interface.is_artboard(&layer.to_node(), &[])
					})
					.collect();

				if tool_data.hovered_layers != new_hovered_layers {
					tool_data.hovered_layers = new_hovered_layers;
				}

				responses.add(OverlaysMessage::Draw);

				self
			}
			(PathToolFsmState::Drawing { selection_shape: selection_type }, PathToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(offset) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_start_pos += offset;
				}

				PathToolFsmState::Drawing { selection_shape: selection_type }
			}
			(PathToolFsmState::Dragging(dragging_state), PathToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(offset) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_start_pos += offset;
				}

				PathToolFsmState::Dragging(dragging_state)
			}
			(
				state,
				PathToolMessage::PointerOutsideViewport {
					equidistant,
					toggle_colinear,
					move_anchor_with_handles,
					snap_angle,
					lock_angle,
					delete_segment,
					break_colinear_molding,
					segment_editing_modifier,
				},
			) => {
				// Auto-panning
				let messages = [
					PathToolMessage::PointerOutsideViewport {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
						snap_angle,
						lock_angle,
						delete_segment,
						break_colinear_molding,
						segment_editing_modifier,
					}
					.into(),
					PathToolMessage::PointerMove {
						equidistant,
						toggle_colinear,
						move_anchor_with_handles,
						snap_angle,
						lock_angle,
						delete_segment,
						break_colinear_molding,
						segment_editing_modifier,
					}
					.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(PathToolFsmState::Drawing { selection_shape }, PathToolMessage::Enter { extend_selection, shrink_selection }) => {
				let extend_selection = input.keyboard.get(extend_selection as usize);
				let shrink_selection = input.keyboard.get(shrink_selection as usize);

				let selection_change = if shrink_selection {
					SelectionChange::Shrink
				} else if extend_selection {
					SelectionChange::Extend
				} else {
					SelectionChange::Clear
				};

				let document_to_viewport = document.metadata().document_to_viewport;
				let previous_mouse = document_to_viewport.transform_point2(tool_data.previous_mouse_position);
				if tool_data.drag_start_pos == previous_mouse {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				} else {
					let selection_mode = match tool_action_data.preferences.get_selection_mode() {
						SelectionMode::Directional => tool_data.calculate_selection_mode_from_direction(document.metadata()),
						selection_mode => selection_mode,
					};

					match selection_shape {
						SelectionShapeType::Box => {
							let bbox = Rect::new(tool_data.drag_start_pos.x, tool_data.drag_start_pos.y, previous_mouse.x, previous_mouse.y).abs();

							shape_editor.select_all_in_shape(
								&document.network_interface,
								SelectionShape::Box(bbox),
								selection_change,
								tool_options.path_overlay_mode,
								tool_data.frontier_handles_info.as_ref(),
								tool_options.path_editing_mode.segment_editing_mode,
								tool_options.path_editing_mode.point_editing_mode,
								selection_mode,
							);
						}
						SelectionShapeType::Lasso => shape_editor.select_all_in_shape(
							&document.network_interface,
							SelectionShape::Lasso(&tool_data.lasso_polygon),
							selection_change,
							tool_options.path_overlay_mode,
							tool_data.frontier_handles_info.as_ref(),
							tool_options.path_editing_mode.segment_editing_mode,
							tool_options.path_editing_mode.point_editing_mode,
							selection_mode,
						),
					}
				}

				responses.add(OverlaysMessage::Draw);

				PathToolFsmState::Ready
			}
			(PathToolFsmState::Dragging { .. }, PathToolMessage::Escape | PathToolMessage::RightClick) => {
				if tool_data.handle_drag_toggle && tool_data.drag_start_pos.distance(input.mouse.position) > DRAG_THRESHOLD {
					shape_editor.deselect_all_points();
					shape_editor.deselect_all_segments();

					for (layer, (selected_points, selected_segments)) in &tool_data.saved_selection_before_handle_drag {
						let Some(state) = shape_editor.selected_shape_state.get_mut(layer) else { continue };
						selected_points.iter().for_each(|point| state.select_point(*point));
						selected_segments.iter().for_each(|segment| state.select_segment(*segment));
					}

					tool_data.saved_selection_before_handle_drag.clear();
					tool_data.handle_drag_toggle = false;
				}
				tool_data.molding_info = None;
				tool_data.molding_segment = false;
				tool_data.temporary_adjacent_handles_while_molding = None;
				tool_data.angle_locked = false;
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}
			(PathToolFsmState::Drawing { .. }, PathToolMessage::Escape | PathToolMessage::RightClick) => {
				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}
			(PathToolFsmState::SlidingPoint, PathToolMessage::Escape | PathToolMessage::RightClick) => {
				tool_data.sliding_point_info = None;

				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);

				PathToolFsmState::Ready
			}
			// Mouse up
			(PathToolFsmState::Drawing { selection_shape }, PathToolMessage::DragStop { extend_selection, shrink_selection }) => {
				let extend_selection = input.keyboard.get(extend_selection as usize);
				let shrink_selection = input.keyboard.get(shrink_selection as usize);

				let select_kind = if shrink_selection {
					SelectionChange::Shrink
				} else if extend_selection {
					SelectionChange::Extend
				} else {
					SelectionChange::Clear
				};

				let document_to_viewport = document.metadata().document_to_viewport;
				let previous_mouse = document_to_viewport.transform_point2(tool_data.previous_mouse_position);

				let selection_mode = match tool_action_data.preferences.get_selection_mode() {
					SelectionMode::Directional => tool_data.calculate_selection_mode_from_direction(document.metadata()),
					selection_mode => selection_mode,
				};
				tool_data.started_drawing_from_inside = false;

				if tool_data.drag_start_pos.distance(previous_mouse) < 1e-8 {
					// Clicked inside or outside the shape then deselect all of the points/segments
					if document.click(input).is_some() && tool_data.stored_selection.is_none() {
						tool_data.stored_selection = Some(shape_editor.selected_shape_state.clone());
					}

					shape_editor.deselect_all_points();
					shape_editor.deselect_all_segments();
				} else {
					match selection_shape {
						SelectionShapeType::Box => {
							let bbox = Rect::new(tool_data.drag_start_pos.x, tool_data.drag_start_pos.y, previous_mouse.x, previous_mouse.y).abs();

							shape_editor.select_all_in_shape(
								&document.network_interface,
								SelectionShape::Box(bbox),
								select_kind,
								tool_options.path_overlay_mode,
								tool_data.frontier_handles_info.as_ref(),
								tool_options.path_editing_mode.segment_editing_mode,
								tool_options.path_editing_mode.point_editing_mode,
								selection_mode,
							);
						}
						SelectionShapeType::Lasso => shape_editor.select_all_in_shape(
							&document.network_interface,
							SelectionShape::Lasso(&tool_data.lasso_polygon),
							select_kind,
							tool_options.path_overlay_mode,
							tool_data.frontier_handles_info.as_ref(),
							tool_options.path_editing_mode.segment_editing_mode,
							tool_options.path_editing_mode.point_editing_mode,
							selection_mode,
						),
					}
				}
				responses.add(OverlaysMessage::Draw);
				responses.add(PathToolMessage::SelectedPointUpdated);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DragStop { extend_selection, .. }) => {
				tool_data.ghost_outline.clear();
				let extend_selection = input.keyboard.get(extend_selection as usize);
				let drag_occurred = tool_data.drag_start_pos.distance(input.mouse.position) > DRAG_THRESHOLD;
				let mut segment_dissolved = false;
				let mut point_inserted = false;

				let nearest_point = shape_editor.find_nearest_visible_point_indices(
					&document.network_interface,
					input.mouse.position,
					SELECTION_THRESHOLD,
					tool_options.path_overlay_mode,
					tool_data.frontier_handles_info.as_ref(),
				);

				let nearest_segment = tool_data.segment.clone();

				if let Some(segment) = &mut tool_data.segment {
					let segment_mode = tool_options.path_editing_mode.segment_editing_mode;
					let point_mode = tool_options.path_editing_mode.point_editing_mode;
					// If segment mode and the insertion modifier is pressed or it is in point editing mode

					if !drag_occurred && !tool_data.molding_segment && ((point_mode && !segment_mode) || (segment_mode && tool_data.segment_editing_modifier)) {
						if tool_data.delete_segment_pressed {
							if let Some(vector) = document.network_interface.compute_modified_vector(segment.layer()) {
								shape_editor.dissolve_segment(responses, segment.layer(), &vector, segment.segment(), segment.points());
								segment_dissolved = true;
							}
						} else {
							let is_segment_selected = shape_editor
								.selected_shape_state
								.get(&segment.layer())
								.is_some_and(|state| state.is_segment_selected(segment.segment()));

							segment.adjusted_insert_and_select(shape_editor, responses, extend_selection, point_mode, is_segment_selected);
							point_inserted = true;
						}
					}

					tool_data.segment = None;
					tool_data.molding_info = None;
					tool_data.molding_segment = false;
					tool_data.temporary_adjacent_handles_while_molding = None;

					if segment_dissolved || point_inserted {
						responses.add(DocumentMessage::EndTransaction);
						return PathToolFsmState::Ready;
					}
				}

				let segment_mode = tool_options.path_editing_mode.segment_editing_mode;
				let point_mode = tool_options.path_editing_mode.point_editing_mode;

				if let (Some((layer, nearest_point)), true) = (nearest_point, point_mode) {
					let clicked_selected = shape_editor.selected_points().any(|&point| nearest_point == point);
					if !drag_occurred && extend_selection {
						if clicked_selected && tool_data.last_clicked_point_was_selected {
							shape_editor.selected_shape_state.entry(layer).or_default().deselect_point(nearest_point);
						} else {
							shape_editor.selected_shape_state.entry(layer).or_default().select_point(nearest_point);
						}
						responses.add(OverlaysMessage::Draw);
					}
					if !drag_occurred && !extend_selection && clicked_selected {
						if tool_data.saved_points_before_anchor_convert_smooth_sharp.is_empty() {
							let mut saved_points = HashMap::new();
							for (layer, state) in &shape_editor.selected_shape_state {
								saved_points.insert(*layer, state.selected_points().collect::<Vec<_>>());
							}
							tool_data.saved_points_before_anchor_convert_smooth_sharp = saved_points;
						}

						shape_editor.deselect_all_points();
						shape_editor.deselect_all_segments();

						shape_editor.selected_shape_state.entry(layer).or_default().select_point(nearest_point);

						responses.add(OverlaysMessage::Draw);
					}
				}
				// Segment editing mode
				else if let Some(nearest_segment) = nearest_segment {
					if segment_mode {
						let clicked_selected = shape_editor.selected_segments().any(|&segment| segment == nearest_segment.segment());
						if !drag_occurred && extend_selection {
							if clicked_selected && tool_data.last_clicked_segment_was_selected {
								shape_editor
									.selected_shape_state
									.entry(nearest_segment.layer())
									.or_default()
									.deselect_segment(nearest_segment.segment());

								// If in segment editing mode only, and upon deselecting a segment, we deselect both of its anchors
								if segment_mode && !point_mode {
									nearest_segment.points().iter().for_each(|point_id| {
										shape_editor
											.selected_shape_state
											.entry(nearest_segment.layer())
											.or_default()
											.deselect_point(ManipulatorPointId::Anchor(*point_id));
									});
								}
							} else {
								shape_editor.selected_shape_state.entry(nearest_segment.layer()).or_default().select_segment(nearest_segment.segment());
							}

							responses.add(OverlaysMessage::Draw);
						}
						if !drag_occurred && !extend_selection && clicked_selected {
							shape_editor.deselect_all_segments();
							shape_editor.deselect_all_points();
							shape_editor.selected_shape_state.entry(nearest_segment.layer()).or_default().select_segment(nearest_segment.segment());

							responses.add(OverlaysMessage::Draw);
						}
					}

					// If only in segment select mode, we also select all of the endpoints of selected segments
					let point_mode = tool_options.path_editing_mode.point_editing_mode;
					if !point_mode {
						let [start, end] = nearest_segment.points();
						shape_editor
							.selected_shape_state
							.entry(nearest_segment.layer())
							.or_default()
							.select_point(ManipulatorPointId::Anchor(start));
						shape_editor
							.selected_shape_state
							.entry(nearest_segment.layer())
							.or_default()
							.select_point(ManipulatorPointId::Anchor(end));
					}
				}
				// Deselect all points if the user clicks the filled region of the shape
				else if tool_data.drag_start_pos.distance(input.mouse.position) <= DRAG_THRESHOLD {
					shape_editor.deselect_all_points();
					shape_editor.deselect_all_segments();
				}

				if tool_data.temporary_colinear_handles {
					tool_data.temporary_colinear_handles = false;
				}

				if tool_data.handle_drag_toggle && drag_occurred {
					shape_editor.deselect_all_points();
					shape_editor.deselect_all_segments();

					for (layer, (selected_points, selected_segments)) in &tool_data.saved_selection_before_handle_drag {
						let Some(state) = shape_editor.selected_shape_state.get_mut(layer) else { continue };
						selected_points.iter().for_each(|point| state.select_point(*point));
						selected_segments.iter().for_each(|segment| state.select_segment(*segment));
					}

					tool_data.saved_selection_before_handle_drag.clear();
					tool_data.handle_drag_toggle = false;
				}

				tool_data.alt_dragging_from_anchor = false;
				tool_data.alt_clicked_on_anchor = false;
				tool_data.angle_locked = false;

				if tool_data.select_anchor_toggled {
					shape_editor.deselect_all_points();
					shape_editor.select_points_by_layer_and_id(&tool_data.saved_points_before_anchor_select_toggle);
					tool_data.remove_saved_points();
					tool_data.select_anchor_toggled = false;
				}

				tool_data.snapping_axis = None;
				tool_data.sliding_point_info = None;

				if drag_occurred || extend_selection {
					responses.add(DocumentMessage::EndTransaction);
				}
				responses.add(PathToolMessage::SelectedPointUpdated);
				tool_data.snap_manager.cleanup(responses);
				tool_data.opposite_handle_position = None;

				PathToolFsmState::Ready
			}

			// Delete key
			(_, PathToolMessage::Delete) => {
				// Delete the selected points and clean up overlays
				let point_mode = tool_options.path_editing_mode.point_editing_mode;
				let segment_mode = tool_options.path_editing_mode.segment_editing_mode;
				let only_segment_mode = segment_mode && !point_mode;

				let transaction_started = shape_editor.delete_selected_segments(document, responses, true);
				if only_segment_mode {
					shape_editor.delete_hanging_selected_anchors(document, responses, !transaction_started);
				} else {
					shape_editor.delete_selected_points(document, responses, !transaction_started);
				}
				responses.add(PathToolMessage::SelectionChanged);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::BreakPath) => {
				shape_editor.break_path_at_selected_point(document, responses);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DeleteAndBreakPath) => {
				shape_editor.delete_point_and_break_path(document, responses);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::ClosePath) => {
				responses.add(DocumentMessage::AddTransaction);
				shape_editor.close_selected_path(document, responses, tool_action_data.preferences.vector_meshes);
				responses.add(DocumentMessage::EndTransaction);

				responses.add(OverlaysMessage::Draw);

				self
			}
			(_, PathToolMessage::StartSlidingPoint) => {
				responses.add(DocumentMessage::StartTransaction);
				if tool_data.start_sliding_point(shape_editor, document) {
					PathToolFsmState::SlidingPoint
				} else {
					PathToolFsmState::Ready
				}
			}
			(_, PathToolMessage::Copy { clipboard }) => {
				// TODO: Add support for selected segments

				let mut buffer = Vec::new();

				for (&layer, layer_selection_state) in &shape_editor.selected_shape_state {
					if layer_selection_state.is_empty() {
						continue;
					}

					let Some(old_vector) = document.network_interface.compute_modified_vector(layer) else { continue };

					// Also get the transform node that is applied on the layer if it exists
					let transform = document.metadata().transform_to_document(layer);

					let mut new_vector = Vector::default();

					let mut selected_points_by_segment = HashSet::new();
					old_vector
						.segment_bezier_iter()
						.filter(|(segment, _, _, _)| layer_selection_state.is_segment_selected(*segment))
						.for_each(|(_, _, start, end)| {
							selected_points_by_segment.insert(start);
							selected_points_by_segment.insert(end);
						});

					// Add all the selected points
					for (point, position) in old_vector.point_domain.iter() {
						if layer_selection_state.is_point_selected(ManipulatorPointId::Anchor(point)) || selected_points_by_segment.contains(&point) {
							new_vector.point_domain.push(point, position);
						}
					}

					let find_index = |id: PointId| new_vector.point_domain.iter().enumerate().find(|(_, (point_id, _))| *point_id == id).map(|(index, _)| index);

					// Add segments which have selected ends
					for ((segment_id, bezier, start, end), stroke) in old_vector.segment_bezier_iter().zip(old_vector.segment_domain.stroke().iter()) {
						let both_ends_selected = layer_selection_state.is_point_selected(ManipulatorPointId::Anchor(start)) && layer_selection_state.is_point_selected(ManipulatorPointId::Anchor(end));

						let segment_selected = layer_selection_state.is_segment_selected(segment_id);

						if both_ends_selected || segment_selected {
							let Some((start_index, end_index)) = find_index(start).zip(find_index(end)) else {
								error!("Point does not exist in point domain");
								return PathToolFsmState::Ready;
							};
							new_vector.segment_domain.push(segment_id, start_index, end_index, bezier.handles, *stroke);
						}
					}

					for handles in old_vector.colinear_manipulators {
						if new_vector.segment_domain.ids().contains(&handles[0].segment) && new_vector.segment_domain.ids().contains(&handles[1].segment) {
							new_vector.colinear_manipulators.push(handles);
						}
					}

					buffer.push((layer, new_vector, transform));
				}

				if clipboard == Clipboard::Device {
					let mut copy_text = String::from("graphite/vector: ");
					copy_text += &serde_json::to_string(&buffer).expect("Could not serialize paste");

					responses.add(FrontendMessage::TriggerTextCopy { copy_text });
				}
				// TODO: Add implementation for internal clipboard

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::Cut { clipboard }) => {
				responses.add(PathToolMessage::Copy { clipboard });
				// Delete the selected points/segments
				responses.add(PathToolMessage::DeleteSelected);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::Paste { data }) => {
				// Deserialize the data
				if let Ok(data) = serde_json::from_str::<Vec<(LayerNodeIdentifier, Vector, DAffine2)>>(&data) {
					shape_editor.deselect_all_points();
					responses.add(DocumentMessage::AddTransaction);
					let mut new_layers = Vec::new();
					for (layer, new_vector, transform) in data {
						// If layer is not selected then create a new selected layer
						let layer = if shape_editor.selected_shape_state.contains_key(&layer) {
							layer
						} else {
							let Some(node_type) = resolve_document_node_type("Path") else {
								error!("Could not resolve node type for Path");
								continue;
							};
							let nodes = vec![(NodeId(0), node_type.default_node_template())];

							let parent = document.new_layer_parent(false);

							let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);

							let fill_color = Color::WHITE;
							let stroke_color = Color::BLACK;

							let fill = graphene_std::vector::style::Fill::solid(fill_color.to_gamma_srgb());
							responses.add(GraphOperationMessage::FillSet { layer, fill });

							let stroke = graphene_std::vector::style::Stroke::new(Some(stroke_color.to_gamma_srgb()), DEFAULT_STROKE_WIDTH);
							responses.add(GraphOperationMessage::StrokeSet { layer, stroke });

							new_layers.push(layer);

							responses.add(GraphOperationMessage::TransformSet {
								layer,
								transform,
								transform_in: TransformIn::Local,
								skip_rerender: false,
							});

							layer
						};

						// Create new point ids and add those into the existing vector content
						let mut points_map = HashMap::new();
						for (point, position) in new_vector.point_domain.iter() {
							let new_point_id = PointId::generate();
							points_map.insert(point, new_point_id);

							let modification_type = VectorModificationType::InsertPoint { id: new_point_id, position };

							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Create new segment ids and add the segments into the existing vector content
						let mut segments_map = HashMap::new();
						for (segment_id, bezier, start, end) in new_vector.segment_iter() {
							let new_segment_id = SegmentId::generate();

							segments_map.insert(segment_id, new_segment_id);

							let points = pathseg_points(bezier);
							let handles = [points.p1.map(|handle| handle - points.p0), points.p2.map(|handle| handle - points.p3)];

							let points = [points_map[&start], points_map[&end]];
							let modification_type = VectorModificationType::InsertSegment { id: new_segment_id, points, handles };

							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Set G1 continuity
						for handles in new_vector.colinear_manipulators {
							let to_new_handle = |handle: HandleId| -> HandleId {
								HandleId {
									ty: handle.ty,
									segment: segments_map[&handle.segment],
								}
							};
							let new_handles = [to_new_handle(handles[0]), to_new_handle(handles[1])];
							let modification_type = VectorModificationType::SetG1Continuous { handles: new_handles, enabled: true };

							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						shape_editor.selected_shape_state.entry(layer).or_insert(Default::default());

						// Set selection to newly inserted points
						let Some(state) = shape_editor.selected_shape_state.get_mut(&layer) else {
							error!("No state for layer: {layer:?}");
							continue;
						};

						// If point editing mode is enabled, select all the pasted points
						if tool_options.path_editing_mode.point_editing_mode {
							points_map.values().for_each(|point| state.select_point(ManipulatorPointId::Anchor(*point)));
						}
						// If segment editing mode is enabled, select all the pasted segments
						if tool_options.path_editing_mode.segment_editing_mode {
							segments_map.values().for_each(|segment| state.select_segment(*segment));
						}
					}

					// If there are new layers created, we need to center them in the viewport
					if !new_layers.is_empty() {
						responses.add(Message::Defer(DeferMessage::AfterGraphRun {
							messages: vec![PortfolioMessage::CenterPastedLayers { layers: new_layers }.into()],
						}));
					}
				}

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DeleteSelected) => {
				// Delete the selected points and segments
				let deleted_some_point = shape_editor.delete_point_and_break_path(document, responses);
				shape_editor.delete_selected_segments(document, responses, !deleted_some_point);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::Duplicate) => {
				responses.add(DocumentMessage::AddTransaction);

				// Copy the existing selected geometry and paste it in the existing layers
				for (layer, layer_selection_state) in shape_editor.selected_shape_state.clone() {
					if layer_selection_state.is_empty() {
						continue;
					}
					let Some(old_vector) = document.network_interface.compute_modified_vector(layer) else {
						continue;
					};

					// Add all the selected points
					let mut selected_points_by_segment = HashSet::new();
					old_vector
						.segment_bezier_iter()
						.filter(|(segment, _, _, _)| layer_selection_state.is_segment_selected(*segment))
						.for_each(|(_, _, start, end)| {
							selected_points_by_segment.insert(start);
							selected_points_by_segment.insert(end);
						});

					let mut points_map = HashMap::new();
					for (point, position) in old_vector.point_domain.iter() {
						// TODO: Either the point is selected or it is an endpoint of a selected segment

						if layer_selection_state.is_point_selected(ManipulatorPointId::Anchor(point)) || selected_points_by_segment.contains(&point) {
							// Insert the same point with a new id
							let new_id = PointId::generate();
							points_map.insert(point, new_id);

							let modification_type = VectorModificationType::InsertPoint { id: new_id, position };

							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}

					let mut segments_map = HashMap::new();

					for (segment_id, bezier, start, end) in old_vector.segment_iter() {
						let both_ends_selected = layer_selection_state.is_point_selected(ManipulatorPointId::Anchor(start)) && layer_selection_state.is_point_selected(ManipulatorPointId::Anchor(end));

						let segment_selected = layer_selection_state.is_segment_selected(segment_id);

						if both_ends_selected || segment_selected {
							let new_id = SegmentId::generate();
							segments_map.insert(segment_id, new_id);

							let points = pathseg_points(bezier);
							let handles = [points.p1.map(|handle| handle - points.p0), points.p2.map(|handle| handle - points.p3)];

							let points = [points_map[&start], points_map[&end]];
							let modification_type = VectorModificationType::InsertSegment { id: new_id, points, handles };

							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}

					for handles in old_vector.colinear_manipulators {
						let to_new_handle = |handle: HandleId| -> HandleId {
							HandleId {
								ty: handle.ty,
								segment: segments_map[&handle.segment],
							}
						};

						if segments_map.contains_key(&handles[0].segment) && segments_map.contains_key(&handles[1].segment) {
							let new_handles = [to_new_handle(handles[0]), to_new_handle(handles[1])];
							let modification_type = VectorModificationType::SetG1Continuous { handles: new_handles, enabled: true };

							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}

					shape_editor.deselect_all_points();
					shape_editor.deselect_all_segments();

					// Set selection to newly inserted points and segments
					let Some(state) = shape_editor.selected_shape_state.get_mut(&layer) else {
						error!("No state for layer: {layer:?}");
						continue;
					};
					if tool_options.path_editing_mode.point_editing_mode {
						points_map.values().for_each(|point| state.select_point(ManipulatorPointId::Anchor(*point)));
					}
					if tool_options.path_editing_mode.segment_editing_mode {
						segments_map.values().for_each(|segment| state.select_segment(*segment));
					}
				}

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DoubleClick { extend_selection, shrink_selection }) => {
				// Double-clicked on a point (flip smooth/sharp behavior)
				let nearest_point = shape_editor.find_nearest_point_indices(&document.network_interface, input.mouse.position, SELECTION_THRESHOLD);

				let mut get_drill_through_layer = || -> Option<LayerNodeIdentifier> {
					let drill_through_layers = document.click_list_no_parents(input).collect::<Vec<LayerNodeIdentifier>>();
					if drill_through_layers.is_empty() {
						tool_data.reset_drill_through_cycle();
						None
					} else {
						tool_data.drill_through_cycle_count = drill_through_layers.len();
						let cycle_index = tool_data.next_drill_through_cycle(input.mouse.position);
						let layer = drill_through_layers.get(cycle_index);
						if cycle_index == 0 { drill_through_layers.first().copied() } else { layer.copied() }
					}
				};

				if nearest_point.is_some() {
					// Flip the selected point between smooth and sharp
					if !tool_data.double_click_handled && tool_data.drag_start_pos.distance(input.mouse.position) <= DRAG_THRESHOLD {
						responses.add(DocumentMessage::StartTransaction);

						shape_editor.select_points_by_layer_and_id(&tool_data.saved_points_before_anchor_convert_smooth_sharp);
						shape_editor.flip_smooth_sharp(&document.network_interface, responses);
						tool_data.saved_points_before_anchor_convert_smooth_sharp.clear();

						responses.add(DocumentMessage::EndTransaction);
						responses.add(DeferMessage::AfterGraphRun {
							messages: vec![PathToolMessage::SelectedPointUpdated.into()],
						});
					}

					return PathToolFsmState::Ready;
				}
				// Double-clicked on a filled region
				else if let Some(layer) = &get_drill_through_layer() {
					let extend_selection = input.keyboard.get(extend_selection as usize);
					let shrink_selection = input.keyboard.get(shrink_selection as usize);

					if shape_editor.is_selected_layer(*layer) {
						if extend_selection && !tool_data.first_selected_with_single_click {
							responses.add(NodeGraphMessage::SelectedNodesRemove { nodes: vec![layer.to_node()] });

							if let Some(selection) = &tool_data.stored_selection {
								let mut selection = selection.clone();
								selection.remove(layer);
								shape_editor.selected_shape_state = selection;
								tool_data.stored_selection = None;
							}
						} else if shrink_selection && !tool_data.first_selected_with_single_click {
							// Only deselect all the points of the double clicked layer
							if let Some(selection) = &tool_data.stored_selection {
								let selection = selection.clone();
								shape_editor.selected_shape_state = selection;
								tool_data.stored_selection = None;
							}

							let state = shape_editor.selected_shape_state.get_mut(layer).expect("No state for selected layer");
							state.deselect_all_points_in_layer();
							state.deselect_all_segments_in_layer();
						} else if !tool_data.first_selected_with_single_click {
							// Select according to the selected editing mode
							let point_editing_mode = tool_options.path_editing_mode.point_editing_mode;
							let segment_editing_mode = tool_options.path_editing_mode.segment_editing_mode;
							shape_editor.select_connected(document, *layer, input.mouse.position, point_editing_mode, segment_editing_mode);

							// Select all the other layers back again
							if let Some(selection) = &tool_data.stored_selection {
								let mut selection = selection.clone();
								selection.remove(layer);

								for (layer, state) in selection {
									shape_editor.selected_shape_state.insert(layer, state);
								}
								tool_data.stored_selection = None;
							}
						}

						// If it was the very first click without there being an existing selection,
						// then the single-click behavior and double-click behavior should not collide
						tool_data.first_selected_with_single_click = false;
					} else if extend_selection {
						responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![layer.to_node()] });

						if let Some(selection) = &tool_data.stored_selection {
							shape_editor.selected_shape_state = selection.clone();
							tool_data.stored_selection = None;
						}
					} else {
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] });
					}

					responses.add(OverlaysMessage::Draw);
				}
				// Double clicked on the background
				else {
					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![] });
				}

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::Abort) => {
				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::NudgeSelectedPoints { delta_x, delta_y }) => {
				shape_editor.move_selected_points_and_segments(
					tool_data.opposing_handle_lengths.take(),
					document,
					(delta_x, delta_y).into(),
					true,
					false,
					false,
					tool_data.opposite_handle_position,
					false,
					responses,
				);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectAll) => {
				shape_editor.select_all_anchors_in_selected_layers(document);

				let point_editing_mode = tool_options.path_editing_mode.point_editing_mode;
				let segment_editing_mode = tool_options.path_editing_mode.segment_editing_mode;

				if point_editing_mode {
					shape_editor.select_all_anchors_in_selected_layers(document);
				}
				if segment_editing_mode {
					shape_editor.select_all_segments_in_selected_layers(document);
				}

				responses.add(OverlaysMessage::Draw);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DeselectAllSelected) => {
				shape_editor.deselect_all_points();
				shape_editor.deselect_all_segments();

				responses.add(OverlaysMessage::Draw);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointXChanged { new_x }) => {
				if let Some(&SingleSelectedPoint { coordinates, id, layer, .. }) = tool_data.selection_status.as_one() {
					shape_editor.reposition_control_point(&id, &document.network_interface, DVec2::new(new_x, coordinates.y), layer, responses);
				}
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointYChanged { new_y }) => {
				if let Some(&SingleSelectedPoint { coordinates, id, layer, .. }) = tool_data.selection_status.as_one() {
					shape_editor.reposition_control_point(&id, &document.network_interface, DVec2::new(coordinates.x, new_y), layer, responses);
				}
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SelectedPointUpdated) => {
				let colinear = shape_editor.selected_manipulator_angles(&document.network_interface);
				tool_data.dragging_state = DraggingState {
					point_select_state: shape_editor.get_dragging_state(&document.network_interface),
					colinear,
				};

				tool_data.make_path_editable_is_allowed = make_path_editable_is_allowed(&mut document.network_interface).is_some();
				tool_data.update_selection_status(shape_editor, document);
				self
			}
			(_, PathToolMessage::ManipulatorMakeHandlesColinear) => {
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.convert_selected_manipulators_to_colinear_handles(responses, document);
				responses.add(DocumentMessage::EndTransaction);
				responses.add(PathToolMessage::SelectionChanged);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::ManipulatorMakeHandlesFree) => {
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.disable_colinear_handles_state_on_selected(&document.network_interface, responses);
				responses.add(DocumentMessage::EndTransaction);
				PathToolFsmState::Ready
			}
			(_, PathToolMessage::SetPivot { position }) => {
				responses.add(DocumentMessage::StartTransaction);

				tool_data.pivot_gizmo.pivot.last_non_none_reference_point = position;
				let position: Option<DVec2> = position.into();
				tool_data.pivot_gizmo.pivot.set_normalized_position(position.unwrap());
				let pivot_gizmo = tool_data.pivot_gizmo();
				responses.add(TransformLayerMessage::SetPivotGizmo { pivot_gizmo });
				responses.add(NodeGraphMessage::RunDocumentGraph);

				self
			}
			(_, _) => PathToolFsmState::Ready,
		}
	}

	fn update_hints(&self, _responses: &mut VecDeque<Message>) {
		// Moved logic to update_dynamic_hints
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

#[derive(Debug, PartialEq, Default)]
enum SelectionStatus {
	#[default]
	None,
	One(SingleSelectedPoint),
	Multiple(MultipleSelectedPoints),
}

impl SelectionStatus {
	fn as_one(&self) -> Option<&SingleSelectedPoint> {
		match self {
			SelectionStatus::One(one) => Some(one),
			_ => None,
		}
	}

	fn angle(&self) -> Option<ManipulatorAngle> {
		match self {
			Self::None => None,
			Self::One(one) => Some(one.manipulator_angle),
			Self::Multiple(one) => Some(one.manipulator_angle),
		}
	}
}

#[derive(Debug, PartialEq)]
struct MultipleSelectedPoints {
	manipulator_angle: ManipulatorAngle,
}

#[derive(Debug, PartialEq)]
struct SingleSelectedPoint {
	coordinates: DVec2,
	id: ManipulatorPointId,
	layer: LayerNodeIdentifier,
	manipulator_angle: ManipulatorAngle,
}

/// Sets the cumulative description of the selected points: if `None` are selected, if `One` is selected, or if `Multiple` are selected.
/// Applies to any selected points, whether they are anchors or handles; and whether they are from a single shape or across multiple shapes.
fn get_selection_status(network_interface: &NodeNetworkInterface, shape_state: &mut ShapeState) -> SelectionStatus {
	let mut selection_layers = shape_state.selected_shape_state.iter().map(|(k, v)| (*k, v.selected_points_count()));
	let total_selected_points = selection_layers.clone().map(|(_, v)| v).sum::<usize>();

	// Check to see if only one manipulator group in a single shape is selected
	if total_selected_points == 1 {
		let Some(layer) = selection_layers.find(|(_, v)| *v > 0).map(|(k, _)| k) else {
			return SelectionStatus::None;
		};
		let Some(vector) = network_interface.compute_modified_vector(layer) else {
			return SelectionStatus::None;
		};
		let Some(&point) = shape_state.selected_points().next() else {
			return SelectionStatus::None;
		};
		let Some(local_position) = point.get_position(&vector) else {
			return SelectionStatus::None;
		};

		let coordinates = network_interface
			.document_metadata()
			.transform_to_document_if_feeds(layer, network_interface)
			.transform_point2(local_position);
		let manipulator_angle = if vector.colinear(point) { ManipulatorAngle::Colinear } else { ManipulatorAngle::Free };

		return SelectionStatus::One(SingleSelectedPoint {
			coordinates,
			layer,
			id: point,
			manipulator_angle,
		});
	};

	// Check to see if multiple manipulator groups are selected
	if total_selected_points > 1 {
		return SelectionStatus::Multiple(MultipleSelectedPoints {
			manipulator_angle: shape_state.selected_manipulator_angles(network_interface),
		});
	}

	SelectionStatus::None
}

fn calculate_lock_angle(
	tool_data: &mut PathToolData,
	shape_state: &mut ShapeState,
	responses: &mut VecDeque<Message>,
	document: &DocumentMessageHandler,
	vector: &Vector,
	handle_id: ManipulatorPointId,
	tangent_to_neighboring_tangents: bool,
) -> Option<f64> {
	let anchor = handle_id.get_anchor(vector)?;
	let anchor_position = vector.point_domain.position_from_id(anchor);
	let current_segment = handle_id.get_segment();
	let points_connected = vector.connected_count(anchor);

	let (anchor_position, segment) = anchor_position.zip(current_segment)?;
	if points_connected == 1 {
		calculate_segment_angle(anchor, segment, vector, false)
	} else {
		let opposite_handle = handle_id
			.get_handle_pair(vector)
			.iter()
			.flatten()
			.find(|&h| h.to_manipulator_point() != handle_id)
			.copied()
			.map(|h| h.to_manipulator_point());
		let opposite_handle_position = opposite_handle.and_then(|h| h.get_position(vector)).filter(|pos| (pos - anchor_position).length() > 1e-6);

		if let Some(opposite_pos) = opposite_handle_position {
			if !vector.colinear_manipulators.iter().flatten().map(|h| h.to_manipulator_point()).any(|h| h == handle_id) {
				shape_state.convert_selected_manipulators_to_colinear_handles(responses, document);
				tool_data.temporary_colinear_handles = true;
			}
			Some(-(opposite_pos - anchor_position).angle_to(DVec2::X))
		} else {
			let angle_1 = vector
				.adjacent_segment(&handle_id)
				.and_then(|(_, adjacent_segment)| calculate_segment_angle(anchor, adjacent_segment, vector, false));

			let angle_2 = calculate_segment_angle(anchor, segment, vector, false);

			match (angle_1, angle_2) {
				(Some(angle_1), Some(angle_2)) => {
					let angle = Some((angle_1 + angle_2) / 2.);
					if tangent_to_neighboring_tangents {
						angle.map(|angle| angle + std::f64::consts::FRAC_PI_2)
					} else {
						angle
					}
				}
				(Some(angle_1), None) => Some(angle_1),
				(None, Some(angle_2)) => Some(angle_2),
				(None, None) => None,
			}
		}
	}
}

fn check_handle_over_adjacent_anchor(handle_id: ManipulatorPointId, vector: &Vector) -> Option<PointId> {
	let (anchor, handle_position) = handle_id.get_anchor(vector).zip(handle_id.get_position(vector))?;

	let check_if_close = |point_id: &PointId| {
		let Some(anchor_position) = vector.point_domain.position_from_id(*point_id) else {
			return false;
		};
		(anchor_position - handle_position).length() < 10.
	};

	vector.connected_points(anchor).find(check_if_close)
}
fn calculate_adjacent_anchor_tangent(currently_dragged_handle: ManipulatorPointId, anchor: Option<PointId>, adjacent_anchor: Option<PointId>, vector: &Vector) -> (Option<f64>, Option<DVec2>) {
	// Early return if no anchor or no adjacent anchors

	let Some((dragged_handle_anchor, adjacent_anchor)) = anchor.zip(adjacent_anchor) else {
		return (None, None);
	};
	let adjacent_anchor_position = vector.point_domain.position_from_id(adjacent_anchor);

	let handles: Vec<_> = vector.all_connected(adjacent_anchor).filter(|handle| handle.length(vector) > 1e-6).collect();

	match handles.len() {
		0 => {
			// Find non-shared segments
			let non_shared_segment: Vec<_> = vector
				.segment_bezier_iter()
				.filter_map(|(segment_id, _, start, end)| {
					let touches_adjacent = start == adjacent_anchor || end == adjacent_anchor;
					let shares_with_dragged = start == dragged_handle_anchor || end == dragged_handle_anchor;

					if touches_adjacent && !shares_with_dragged { Some(segment_id) } else { None }
				})
				.collect();

			match non_shared_segment.first() {
				Some(&segment) => {
					let angle = calculate_segment_angle(adjacent_anchor, segment, vector, true);
					(angle, adjacent_anchor_position)
				}
				None => (None, None),
			}
		}

		1 => {
			let segment = handles[0].segment;
			let angle = calculate_segment_angle(adjacent_anchor, segment, vector, true);
			(angle, adjacent_anchor_position)
		}

		2 => {
			// Use the angle formed by the handle of the shared segment relative to its associated anchor point.
			let Some(shared_segment_handle) = handles
				.iter()
				.find(|handle| handle.opposite().to_manipulator_point() == currently_dragged_handle)
				.map(|handle| handle.to_manipulator_point())
			else {
				return (None, None);
			};

			let angle = shared_segment_handle
				.get_position(vector)
				.zip(adjacent_anchor_position)
				.map(|(handle, anchor)| -(handle - anchor).angle_to(DVec2::X));

			(angle, adjacent_anchor_position)
		}

		_ => (None, None),
	}
}

fn update_dynamic_hints(
	state: PathToolFsmState,
	responses: &mut VecDeque<Message>,
	shape_editor: &mut ShapeState,
	document: &DocumentMessageHandler,
	tool_data: &PathToolData,
	tool_options: &PathToolOptions,
	position: DVec2,
) {
	// Condinting based on currently selected segment if it has any one g1 continuous handle

	let hint_data = match state {
		PathToolFsmState::Ready => {
			// Show point sliding hints only when there is an anchor with colinear handles selected
			let single_anchor_selected = shape_editor.selected_points().count() == 1 && shape_editor.selected_points().any(|point| matches!(point, ManipulatorPointId::Anchor(_)));
			let at_least_one_anchor_selected = shape_editor.selected_points().any(|point| matches!(point, ManipulatorPointId::Anchor(_)));
			let at_least_one_point_selected = shape_editor.selected_points().count() >= 1;

			let mut single_colinear_anchor_selected = false;
			if single_anchor_selected {
				if let (Some(anchor), Some(layer)) = (
					shape_editor.selected_points().next(),
					document.network_interface.selected_nodes().selected_layers(document.metadata()).next(),
				) {
					if let Some(vector) = document.network_interface.compute_modified_vector(layer) {
						single_colinear_anchor_selected = vector.colinear(*anchor)
					}
				}
			}

			let drag_selected_hints = vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")];
			let mut delete_selected_hints = vec![HintInfo::keys([Key::Delete], "Delete Selected")];

			if at_least_one_anchor_selected {
				delete_selected_hints.push(HintInfo::keys([Key::Accel], "No Dissolve").prepend_plus());
				delete_selected_hints.push(HintInfo::keys([Key::Shift], "Cut Anchor").prepend_plus());
			}

			let segment_edit = tool_options.path_editing_mode.segment_editing_mode;
			let point_edit = tool_options.path_editing_mode.point_editing_mode;

			let hovering_segment = tool_data.segment.is_some();
			let hovering_point = shape_editor
				.find_nearest_visible_point_indices(
					&document.network_interface,
					position,
					SELECTION_THRESHOLD,
					tool_options.path_overlay_mode,
					tool_data.frontier_handles_info.as_ref(),
				)
				.is_some();

			let mut hint_data = if hovering_segment {
				if segment_edit {
					// Hovering a segment in segment editing mode
					vec![
						HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Segment"), HintInfo::keys([Key::Shift], "Extend").prepend_plus()]),
						HintGroup(vec![HintInfo::keys_and_mouse([Key::Control], MouseMotion::Lmb, "Insert Point on Segment")]),
						HintGroup(vec![HintInfo::keys_and_mouse([Key::Control], MouseMotion::LmbDrag, "Mold Segment")]),
					]
				} else {
					// Hovering a segment in point editing mode
					vec![
						HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Insert Point on Segment")]),
						HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Mold Segment")]),
						HintGroup(vec![HintInfo::keys_and_mouse([Key::Alt], MouseMotion::Lmb, "Delete Segment")]),
					]
				}
			} else if hovering_point {
				if point_edit {
					// Hovering over a point in point editing mode
					vec![HintGroup(vec![
						HintInfo::mouse(MouseMotion::Lmb, "Select Point"),
						HintInfo::keys([Key::Shift], "Extend").prepend_plus(),
					])]
				} else {
					// Hovering over a point in segment selection mode (will select a nearby segment)
					vec![HintGroup(vec![
						HintInfo::mouse(MouseMotion::Lmb, "Select Segment"),
						HintInfo::keys([Key::Shift], "Extend").prepend_plus(),
					])]
				}
			} else {
				vec![HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
					HintInfo::keys([Key::Control], "Lasso").prepend_plus(),
				])]
			};

			if at_least_one_anchor_selected {
				// TODO: Dynamically show either "Smooth" or "Sharp" based on the current state
				hint_data.push(HintGroup(vec![
					HintInfo::mouse(MouseMotion::LmbDouble, "Convert Anchor Point"),
					HintInfo::keys_and_mouse([Key::Alt], MouseMotion::Lmb, "To Sharp"),
					HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "To Smooth"),
				]));
			}

			if at_least_one_point_selected {
				let mut groups = vec![
					HintGroup(drag_selected_hints),
					HintGroup(vec![HintInfo::multi_keys([[Key::KeyG], [Key::KeyR], [Key::KeyS]], "Grab/Rotate/Scale Selected")]),
				];

				if single_colinear_anchor_selected {
					groups.push(HintGroup(vec![HintInfo::multi_keys([[Key::KeyG], [Key::KeyG]], "Slide")]));
				}

				groups.push(HintGroup(vec![HintInfo::arrow_keys("Nudge Selected"), HintInfo::keys([Key::Shift], "10x").prepend_plus()]));
				groups.push(HintGroup(delete_selected_hints));

				hint_data.append(&mut groups);
			}

			HintData(hint_data)
		}
		PathToolFsmState::Dragging(dragging_state) => {
			let colinear = dragging_state.colinear;
			let mut dragging_hint_data = HintData(Vec::new());
			dragging_hint_data
				.0
				.push(HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]));

			let drag_anchor = HintInfo::keys([Key::Space], "Drag Anchor");
			let toggle_group = match dragging_state.point_select_state {
				PointSelectState::HandleNoPair | PointSelectState::HandleWithPair => {
					let mut hints = vec![HintInfo::keys([Key::Tab], "Swap Dragged Handle")];
					hints.push(HintInfo::keys(
						[Key::KeyC],
						if colinear == ManipulatorAngle::Colinear {
							"Break Colinear Handles"
						} else {
							"Make Handles Colinear"
						},
					));
					hints
				}
				PointSelectState::Anchor => Vec::new(),
			};
			let hold_group = match dragging_state.point_select_state {
				PointSelectState::HandleNoPair => {
					let mut hints = vec![];
					if colinear != ManipulatorAngle::Free {
						hints.push(HintInfo::keys([Key::Alt], "Equidistant Handles"));
					}
					hints.push(HintInfo::keys([Key::Shift], "15° Increments"));
					hints.push(HintInfo::keys([Key::Control], "Lock Angle"));
					hints.push(drag_anchor);
					hints
				}
				PointSelectState::HandleWithPair => {
					let mut hints = vec![];
					if colinear != ManipulatorAngle::Free {
						hints.push(HintInfo::keys([Key::Alt], "Equidistant Handles"));
					}
					hints.push(HintInfo::keys([Key::Shift], "15° Increments"));
					hints.push(HintInfo::keys([Key::Control], "Lock Angle"));
					hints.push(drag_anchor);
					hints
				}
				PointSelectState::Anchor => Vec::new(),
			};

			if !toggle_group.is_empty() {
				dragging_hint_data.0.push(HintGroup(toggle_group));
			}

			if !hold_group.is_empty() {
				dragging_hint_data.0.push(HintGroup(hold_group));
			}

			if tool_data.molding_segment {
				let mut has_colinear_anchors = false;

				if let Some(segment) = &tool_data.segment {
					let handle1 = HandleId::primary(segment.segment());
					let handle2 = HandleId::end(segment.segment());

					if let Some(vector) = document.network_interface.compute_modified_vector(segment.layer()) {
						let other_handle1 = vector.other_colinear_handle(handle1);
						let other_handle2 = vector.other_colinear_handle(handle2);
						if other_handle1.is_some() || other_handle2.is_some() {
							has_colinear_anchors = true;
						}
					};
				}

				let handles_stored = if let Some(other_handles) = tool_data.temporary_adjacent_handles_while_molding {
					other_handles[0].is_some() || other_handles[1].is_some()
				} else {
					false
				};

				let molding_disable_possible = has_colinear_anchors || handles_stored;

				let mut molding_hints = vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])];

				if molding_disable_possible {
					molding_hints.push(HintGroup(vec![HintInfo::keys([Key::Alt], "Break Colinear Handles")]));
				}

				HintData(molding_hints)
			} else {
				dragging_hint_data
			}
		}
		PathToolFsmState::Drawing { .. } => HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
			HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
				HintInfo::keys([Key::Shift], "Extend").prepend_plus(),
				HintInfo::keys([Key::Alt], "Subtract").prepend_plus(),
			]),
		]),
		PathToolFsmState::SlidingPoint => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
	};
	responses.add(FrontendMessage::UpdateInputHints { hint_data });
}
